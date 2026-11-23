//! The game side of the **Mob Crusher** (`ModuleAgricultureHusbandry.MOB_CRUSHER`).
//!
//! Upstream's `MobCrusherTile` is an `IndustrialAreaWorkingTile` over Titanium's
//! `ProgressBarComponent`: a bar that counts **down** from 100, a 10,000 FE buffer, a
//! **32,000 mB essence** tank (`Action.DRAIN`) and an **18-slot output container** (6 × 3,
//! output-only), with a working area of the block **behind** the machine expanded two blocks up.
//! At zero the attempt hurts the first eligible Mob for 50 FE; the arithmetic is [`crate::crusher`].
//!
//! # The kill is a player kill
//!
//! The jar rolls the victim's loot table with a **fake player**'s `LootContext` and reads the
//! experience with `entity.getExperienceReward(level, fakePlayer)`. The pin has the same fake
//! player (`Host::fake_player`, upstream's `IFFakePlayer`) and a player-credited hurt
//! (`EntityRef::hurt` with the fake as its `direct` and `causing` source), so the port lets the
//! engine's own death path do both: the loot table rolls with a player kill and vanilla awards
//! the mob's experience. Two hooks take that result, because the machine owns it:
//!
//! - [`mod_api::Hooks::living_drops`] takes the whole drop list and queues it for the machine's
//!   output container (upstream's `ItemHandlerHelper.insertItem(this.output, …)`), cancelling
//!   the world drops;
//! - [`mod_api::Hooks::living_experience_drop`] queues `20 × points` mB for the tank and cancels
//!   the orbs (the machine turns the experience into essence, as upstream's `fillForced` does).
//!
//! Both hooks find their machine through the [`ATTEMPTS`] record written **before** the hurt:
//! the victim's entity id, the machine's position and its dimension. The machine drains the
//! queues in its next tick (the death, and so the hooks, run after a block entity's own tick),
//! records the last kill's points for the readout and prunes an attempt no death answered.
//!
//! # The blacklist
//!
//! `industrialforegoing:mob_crusher_instant_kill_blacklist` (the ender dragon) sends the attempt
//! down `damage()`: `entity.hurt(playerAttack, 75)` instead of the instant kill. The tag is read
//! through [`Host::entity_type_in_tag`]. The wither's spawn invulnerability has no pin read, so
//! `wither_spawning` is always false (deviation, recorded in `notes.md`).

use std::sync::{Mutex, OnceLock};

use mod_api::{
    self, BlockEntityPart, BlockEntityRef, BlockEntityTickEvent, BlockEntityTicksEvent, BlockPart,
    Compound, Direction, EntityRef, FLUIDS, FluidIdentity, Host, HostTransaction, ItemKey,
    ItemPart, LevelHost, MobCategory, ModContainer, ModEnergyBuffer, ModFluidTanks,
    OnBlockPlacedByEvent, Registrar, ServerHost, StateForPlacementEvent, Tag,
};

use crate::crusher::{
    AREA_UP, AttackPath, CAPACITY_FE, Candidate, Crusher, ESSENCE, OUTPUT_SLOTS, Step, TANK_MB,
};

/// The machine (`ModuleAgricultureHusbandry.MOB_CRUSHER`).
pub const BLOCK: &str = "industrialforegoing:mob_crusher";
/// Its block entity type, a bare name so the mod's namespace qualifies it.
pub const ENTITY: &str = "mob_crusher";
/// The block entity type's registry id, which `minecraft:block_entity_data` tags its payload with.
pub const ENTITY_ID: &str = "industrialforegoing:mob_crusher";
/// The state property Titanium's `RotationType.FOUR_WAY` sets.
pub const SUBFACING: &str = "subfacing";
/// The jar's `IndustrialTags.EntityTypes.MOB_CRUSHER_INSTANT_KILL_BLACKLIST`.
pub const BLACKLIST: &str = "industrialforegoing:mob_crusher_instant_kill_blacklist";
/// The saved keys: the buffer, the bar and the last kill's experience (the readout's evidence).
const KEY_ENERGY: &str = "energy";
const KEY_PROGRESS: &str = "progress";
const KEY_LAST_XP: &str = "last_xp";
/// The one tank, in declaration order.
/// The side the tank is read through (all six reach it).
const READ_FACE: Direction = Direction::Up;
/// How many ticks an attempt waits for its death before it is forgotten.
const ATTEMPT_TTL: i64 = 5;

/// The block entity type and the item, resolved once in `register`: `block_drops` and the command
/// path run with a read-only host, which cannot name either.
static KIND: OnceLock<mod_api::BlockEntityTypeKey> = OnceLock::new();
static ITEM: OnceLock<ItemKey> = OnceLock::new();

// ---------------------------------------------------------------------------
// The declarations
// ---------------------------------------------------------------------------

/// Declare the machine: its block entity type, its 10,000 FE buffer, its 32,000 mB drain-only
/// essence tank, its 18-slot output container, the Part that ticks it and the Part on its item.
pub(crate) fn declare(r: &mut dyn Registrar) {
    let block = r.block_key(BLOCK);
    let item = r.item_key(BLOCK);
    let _ = ITEM.set(item);
    r.item_part(item, CrusherItem);
    let kind = r.block_entity_type(ENTITY).blocks(&[ENTITY]).build();
    let _ = KIND.set(kind);
    r.block_entity_part(&kind, CrusherPart)
        .expect("the crusher's Part attaches to the type this mod just declared");
    // `new EnergyStorageComponent<>(maxStoredPower, 10, 20)`: unlimited IO up to capacity.
    r.block_energy(
        &block,
        ModEnergyBuffer {
            capacity: CAPACITY_FE,
            max_insert: CAPACITY_FE,
            max_extract: CAPACITY_FE,
            insert_faces: [true; 6],
            extract_faces: [true; 6],
        },
    );
    r.block_fluids(&block, tanks());
    r.block_container(&block, container());
}

/// The one tank: `SidedFluidTankComponent("essence", 32000, …, Action.DRAIN)`. Every face reaches
/// it for reading and draining; no face may fill it (the machine fills it itself).
#[must_use]
pub fn tanks() -> ModFluidTanks {
    let every = vec![0usize];
    let mut tanks = ModFluidTanks::default();
    tanks.tanks = vec![TANK_MB];
    tanks.faces = [
        every.clone(),
        every.clone(),
        every.clone(),
        every.clone(),
        every.clone(),
        every,
    ];
    tanks.outputs = vec![0];
    tanks
}

/// The eighteen slots: `SidedInventoryComponent("output", …, 6 * 3, 1)` with
/// `setInputFilter((stack, slot) -> false)` — output-only, every face reaches them, a hopper may
/// pull from any.
#[must_use]
pub fn container() -> ModContainer {
    let every: Vec<usize> = (0..OUTPUT_SLOTS).collect();
    let mut container = ModContainer {
        slots: OUTPUT_SLOTS,
        ..ModContainer::default()
    };
    container.faces = [
        every.clone(),
        every.clone(),
        every.clone(),
        every.clone(),
        every.clone(),
        every,
    ];
    container.outputs = (0..OUTPUT_SLOTS).collect();
    container
}

// ---------------------------------------------------------------------------
// The attempts and the harvests
// ---------------------------------------------------------------------------

/// A kill the machine has just dealt: the victim, the machine, and when the attempt was made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Attempt {
    entity: u32,
    pub(crate) dimension: String,
    pub(crate) pos: [i32; 3],
    made: i64,
}

/// One kill's loot, waiting for the machine's next tick.
#[derive(Clone, Debug, PartialEq)]
struct Harvest {
    dimension: String,
    pos: [i32; 3],
    stacks: Vec<mod_api::ItemStack>,
}

/// One kill's experience, waiting for the machine's next tick.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Gain {
    dimension: String,
    pos: [i32; 3],
    points: i32,
}

/// The attempts in flight, the loot and the experience they will produce.
static ATTEMPTS: Mutex<Vec<Attempt>> = Mutex::new(Vec::new());
static HARVESTS: Mutex<Vec<Harvest>> = Mutex::new(Vec::new());
static GAINS: Mutex<Vec<Gain>> = Mutex::new(Vec::new());

/// The fake player's UUID for a machine: derived from its dimension and position, so every tick
/// reaches the same connectionless player upstream's `worldFakePlayer` map keeps per tile.
#[must_use]
pub fn fake_uuid(dimension: &str, pos: [i32; 3]) -> u128 {
    let mut hash: u128 = 0x9e37_79b9_7f4a_7c15_6a09_e667_f3bc_c908;
    for byte in dimension.bytes().chain(pos[0].to_le_bytes()) {
        hash ^= u128::from(byte);
        hash = hash.wrapping_mul(0x0000_0000_0100_0000_0000_0000_0000_01b3_u128);
    }
    for value in pos[1..].iter().flat_map(|value| value.to_le_bytes()) {
        hash ^= u128::from(value);
        hash = hash.wrapping_mul(0x0000_0000_0100_0000_0000_0000_0000_01b3_u128);
    }
    // A version-4-shaped UUID: the factory keys its map on the value, and a zero would collide.
    (hash & !(0xf_u128 << 76)) | (0x4_u128 << 76) | (0x2_u128 << 62)
}

/// Record an attempt before the victim is hurt, so the hooks a death fires can find the machine.
fn record_attempt(attempt: Attempt) {
    if let Ok(mut attempts) = ATTEMPTS.lock() {
        attempts.push(attempt);
        // A machine broken mid-death can leave entries behind; only a handful are ever live.
        let extra = attempts.len().saturating_sub(64);
        if extra > 0 {
            attempts.drain(..extra);
        }
    }
}

/// Forget the attempt for `entity` (a hurt the mob survived).
fn forget(entity: u32) {
    if let Ok(mut attempts) = ATTEMPTS.lock() {
        attempts.retain(|attempt| attempt.entity != entity);
    }
}

/// The attempt a dying `entity` belongs to, if any (leaving it in place for the experience hook).
pub(crate) fn attempt_for(entity: u32) -> Option<Attempt> {
    ATTEMPTS
        .lock()
        .ok()?
        .iter()
        .find(|attempt| attempt.entity == entity)
        .cloned()
}

/// Take the attempt a dying `entity` belongs to, if any.
pub(crate) fn take_attempt(entity: u32) -> Option<Attempt> {
    let mut attempts = ATTEMPTS.lock().ok()?;
    let index = attempts
        .iter()
        .position(|attempt| attempt.entity == entity)?;
    Some(attempts.remove(index))
}

/// Queue one kill's loot for the machine at `pos` in `dimension`.
pub(crate) fn queue_loot(dimension: &str, pos: [i32; 3], stacks: Vec<mod_api::ItemStack>) {
    if stacks.is_empty() {
        return;
    }
    if let Ok(mut harvests) = HARVESTS.lock()
        && let Some(existing) = harvests
            .iter_mut()
            .find(|harvest| harvest.dimension == dimension && harvest.pos == pos)
    {
        existing.stacks.extend(stacks);
        return;
    }
    if let Ok(mut harvests) = HARVESTS.lock() {
        harvests.push(Harvest {
            dimension: dimension.to_owned(),
            pos,
            stacks,
        });
    }
}

/// Queue one kill's experience for the machine at `pos` in `dimension`.
pub(crate) fn queue_gain(dimension: &str, pos: [i32; 3], points: i32) {
    if points <= 0 {
        return;
    }
    let gain = Gain {
        dimension: dimension.to_owned(),
        pos,
        points,
    };
    if let Ok(mut gains) = GAINS.lock() {
        gains.push(gain);
    }
}

/// The loot waiting for the machine at `pos`, and the experience points waiting for it.
fn take_harvest(dimension: &str, pos: [i32; 3]) -> (Vec<mod_api::ItemStack>, Vec<Gain>) {
    let stacks = HARVESTS
        .lock()
        .ok()
        .map(|mut harvests| {
            let mut taken = Vec::new();
            let mut index = 0;
            while index < harvests.len() {
                if harvests[index].dimension == dimension && harvests[index].pos == pos {
                    taken.extend(harvests.remove(index).stacks);
                } else {
                    index += 1;
                }
            }
            taken
        })
        .unwrap_or_default();
    let gains = GAINS
        .lock()
        .ok()
        .map(|mut gains| {
            let mut taken = Vec::new();
            let mut index = 0;
            while index < gains.len() {
                if gains[index].dimension == dimension && gains[index].pos == pos {
                    taken.push(gains.remove(index));
                } else {
                    index += 1;
                }
            }
            taken
        })
        .unwrap_or_default();
    (stacks, gains)
}

/// Forget this machine's attempts that no death answered (a mob whose hurt was refused, or a
/// death the engine never credited).
fn prune_attempts(dimension: &str, pos: [i32; 3], now: i64) {
    if let Ok(mut attempts) = ATTEMPTS.lock() {
        attempts.retain(|attempt| {
            !(attempt.dimension == dimension
                && attempt.pos == pos
                && now.saturating_sub(attempt.made) > ATTEMPT_TTL)
        });
    }
}

// ---------------------------------------------------------------------------
// The Part
// ---------------------------------------------------------------------------

/// The crusher: its placement rotation, the augment seat and its tick.
pub(crate) struct CrusherPart;

#[mod_api::horizon_part]
impl BlockPart for CrusherPart {
    /// `RotationType.FOUR_WAY`: a placed crusher faces its placer.
    fn state_for_placement(
        &self,
        level: &dyn LevelHost,
        event: &mut StateForPlacementEvent<'_>,
    ) -> Option<Option<mod_api::BlockState>> {
        let faced = level.state_with_property(
            event.state,
            SUBFACING,
            crate::facing_for_yaw(event.context.yaw),
        )?;
        Some(Some(faced))
    }

    /// `IndustrialWorkingTile.canAcceptAugment`: both a speed and an efficiency addon seat, in the
    /// same two saved slots the other machines use.
    fn use_item_on(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::UseItemOnEvent<'_>,
    ) -> Option<mod_api::InteractionResult> {
        crate::seat_from_click(host, event)
    }
}

#[mod_api::horizon_part]
impl BlockEntityPart for CrusherPart {
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    /// One server tick: take what the last kill left, then fall the bar and, at zero, kill the
    /// first eligible mob behind the machine for 50 FE.
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let dimension = host.dimension().to_owned();
        let entity = &mut *event.entity;
        // A container the save left short comes back short (the chamber's measured case): pad the
        // view so the output slots are always the declared eighteen.
        if entity.items.len() < OUTPUT_SLOTS {
            entity.items.resize(OUTPUT_SLOTS, None);
        }
        let mut crusher = Crusher::from_saved(
            read_int(&entity.nbt, KEY_ENERGY),
            read_progress(&entity.nbt),
            read_tank(&entity.nbt),
        );
        // What the last kill produced: the tank and the output container this machine owns.
        let (stacks, gains) = take_harvest(&dimension, pos);
        for gain in gains {
            crusher.deposit_essence(gain.points);
            write_int(&mut entity.nbt, KEY_LAST_XP, gain.points);
        }
        for stack in stacks {
            crate::dissolution::put(&*host, &mut entity.items, stack);
        }
        let now = host
            .level(host.dimension())
            .and_then(|level| level.game_time(&*host))
            .unwrap_or(0);
        prune_attempts(&dimension, pos, now);
        let augments = crate::read_augments(&entity.nbt);
        let found = victim(host, pos);
        let step = crusher.tick(augments, found.as_ref().map(|found| found.candidate));
        if let (Step::Killed { path }, Some(found)) = (step, found) {
            kill(host, pos, &dimension, now, found, path);
        }
        write_int(&mut entity.nbt, KEY_ENERGY, crusher.energy);
        write_int(&mut entity.nbt, KEY_PROGRESS, crusher.progress);
        write_tank(&mut entity.nbt, crusher.essence_mb);
    }
}

/// A mob the machine may kill: its handle, the numbers the rules read and its health.
struct Found {
    entity: EntityRef,
    candidate: Candidate,
    health: f32,
}

/// The first eligible mob in `RangeManager(BEHIND)`'s box, expanded two blocks up.
fn victim(host: &mut dyn ServerHost, pos: [i32; 3]) -> Option<Found> {
    let level = host.level(host.dimension())?;
    let facing = level.block_property(&*host, pos, SUBFACING)?;
    let [dx, dy, dz] = behind(&facing)?;
    let [x, y, z] = [pos[0] + dx, pos[1] + dy, pos[2] + dz];
    let min = [f64::from(x), f64::from(y), f64::from(z)];
    let max = [
        f64::from(x) + 1.0,
        f64::from(y + 1 + AREA_UP),
        f64::from(z) + 1.0,
    ];
    for entity in level.entities_in(&*host, min, max)? {
        let Some(candidate) = candidate_of(host, entity) else {
            continue;
        };
        if !crate::crusher::eligible(candidate) {
            continue;
        }
        let health = host
            .living(entity)
            .and_then(|living| living.health(&*host))
            .unwrap_or(0.0);
        return Some(Found {
            entity,
            candidate,
            health,
        });
    }
    None
}

/// One entity's numbers for the jar's filter, or `None` when it is not a living mob at all.
fn candidate_of(host: &mut dyn ServerHost, entity: EntityRef) -> Option<Candidate> {
    host.mob(entity)?;
    let living = host.living(entity)?;
    let alive = living.alive(&*host).unwrap_or(false);
    let health = living.health(&*host).unwrap_or(0.0);
    if !alive || health <= 0.0 {
        return None;
    }
    let baby = living.baby(&*host).unwrap_or(false);
    let category = entity.classification(&*host);
    let animal = matches!(
        category,
        Some(MobCategory::Creature | MobCategory::Axolotls)
    );
    let blacklisted = entity
        .kind(&*host)
        .map(|kind| kind.id(&*host))
        .is_some_and(|kind| host.entity_type_in_tag(&kind, BLACKLIST));
    Some(Candidate {
        mob: true,
        alive,
        invulnerable: entity.permanently_invulnerable(&*host).unwrap_or(false),
        baby,
        animal,
        // `WitherBoss.getInvulnerableTicks()` has no pin read (deviation, `notes.md`).
        wither_spawning: false,
        blacklisted,
    })
}

/// The offset of the block behind a machine facing `facing` (`Direction.getOpposite`).
fn behind(facing: &str) -> Option<[i32; 3]> {
    match facing {
        "north" => Some([0, 0, 1]),
        "south" => Some([0, 0, -1]),
        "east" => Some([-1, 0, 0]),
        "west" => Some([1, 0, 0]),
        _ => None,
    }
}

/// Hit the victim as upstream does: the blacklist for 75 damage, anything else for its life.
///
/// The record goes in **before** the hurt so the death's two hooks find the machine; a hurt the
/// mob survives forgets it again.
fn kill(
    host: &mut dyn ServerHost,
    pos: [i32; 3],
    dimension: &str,
    now: i64,
    found: Found,
    path: AttackPath,
) {
    let Some(level) = host.level(host.dimension()) else {
        return;
    };
    let Some(fake) = host.fake_player(level, fake_uuid(dimension, pos), "[IF]") else {
        return;
    };
    let amount = match path {
        AttackPath::Blacklisted => crate::crusher::ATTACK_DAMAGE,
        AttackPath::Instant => found.health + 1.0,
    };
    record_attempt(Attempt {
        entity: found.entity.id(),
        dimension: dimension.to_owned(),
        pos,
        made: now,
    });
    let landed = found
        .entity
        .hurt(
            host,
            "minecraft:player_attack",
            amount,
            Some(fake.entity()),
            Some(fake.entity()),
        )
        .unwrap_or(false);
    if !landed
        || host
            .living(found.entity)
            .and_then(|living| living.alive(&*host))
            .unwrap_or(false)
    {
        forget(found.entity.id());
    }
}

// ---------------------------------------------------------------------------
// The item
// ---------------------------------------------------------------------------

/// The crusher's item: a broken machine's drop carries its buffer, tank, bar and output slots,
/// and placing it clamps the numbers back to the capacities (slice 2's route).
pub(crate) struct CrusherItem;

#[mod_api::horizon_part]
impl ItemPart for CrusherItem {
    fn on_block_placed_by(&self, host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
        let Some(player) = event.placer else {
            return;
        };
        let Some(level) = player.level(&*host) else {
            return;
        };
        let Some(entity) = host.block_entity(level, event.pos) else {
            return;
        };
        let Some(saved) = entity.data(&*host) else {
            return;
        };
        if crate::current_block(host, event.pos).as_deref() != Some(BLOCK) {
            return;
        }
        if let Some(patch) = clamp_patch(&saved) {
            let _ = entity.merge_data(host, patch);
        }
    }
}

// ---------------------------------------------------------------------------
// The saved compound
// ---------------------------------------------------------------------------

fn read_int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(value)) => *value,
        _ => 0,
    }
}

fn write_int(nbt: &mut Compound, key: &str, value: i32) {
    nbt.insert(key.into(), Tag::Int(value));
}

/// The bar from the save: a key the save never wrote is a **full** bar, not an empty one.
fn read_progress(nbt: &Compound) -> Option<i32> {
    match nbt.get(&KEY_PROGRESS.into()) {
        Some(Tag::Int(value)) => Some(*value),
        _ => None,
    }
}

/// The tank's amount from the entity's `fluids`: it only counts while it holds essence, so a
/// `/data merge` that put water there reads as empty.
fn read_tank(nbt: &Compound) -> i32 {
    let Some(Tag::List(entries)) = nbt.get(&"fluids".into()) else {
        return 0;
    };
    for entry in entries {
        let Tag::Compound(entry) = entry else {
            continue;
        };
        match entry.get(&"Fluid".into()) {
            Some(Tag::String(fluid)) if fluid.to_string_lossy() == ESSENCE => {}
            _ => continue,
        }
        let amount = match entry.get(&"Amount".into()) {
            Some(Tag::Int(amount)) => *amount,
            _ => 0,
        };
        return amount.clamp(0, TANK_MB);
    }
    0
}

/// The tank back in the pin's shape.
fn write_tank(nbt: &mut Compound, mb: i32) {
    let mut entry = Compound::new();
    entry.insert(
        "Fluid".into(),
        Tag::String(if mb > 0 { ESSENCE } else { "minecraft:empty" }.into()),
    );
    entry.insert("Amount".into(), Tag::Int(mb.clamp(0, TANK_MB)));
    nbt.insert("fluids".into(), Tag::List(vec![Tag::Compound(entry)]));
}

/// Whether the crusher holds anything worth carrying on its item.
pub(crate) fn has_contents(saved: &Compound) -> bool {
    let progress = read_progress(saved).unwrap_or(crate::crusher::MAX_PROGRESS);
    read_int(saved, KEY_ENERGY) > 0
        || progress < crate::crusher::MAX_PROGRESS
        || read_tank(saved) > 0
        || read_int(saved, KEY_LAST_XP) > 0
        || saved.contains_key(&"Items".into())
        || crate::has_seated_augment(saved)
}

/// The `minecraft:block_entity_data` payload a broken crusher's item carries.
pub(crate) fn carried_patch(saved: &Compound) -> Compound {
    let mut payload = saved.clone();
    payload.insert("id".into(), Tag::String(ENTITY_ID.into()));
    let mut patch = Compound::new();
    patch.insert("minecraft:block_entity_data".into(), Tag::Compound(payload));
    patch
}

/// The corrections a placed crusher needs when its item carried values outside the capacities.
#[must_use]
pub(crate) fn clamp_patch(saved: &Compound) -> Option<Compound> {
    let mut patch = Compound::new();
    let mut changed = false;
    let energy = read_int(saved, KEY_ENERGY);
    if !(0..=CAPACITY_FE).contains(&energy) {
        write_int(&mut patch, KEY_ENERGY, energy.clamp(0, CAPACITY_FE));
        changed = true;
    }
    let progress = read_int(saved, KEY_PROGRESS);
    if !(0..=crate::crusher::MAX_PROGRESS).contains(&progress) {
        write_int(
            &mut patch,
            KEY_PROGRESS,
            progress.clamp(0, crate::crusher::MAX_PROGRESS),
        );
        changed = true;
    }
    if let Some(Tag::List(entries)) = saved.get(&"fluids".into())
        && entries.iter().any(|entry| match entry {
            Tag::Compound(entry) => match entry.get(&"Amount".into()) {
                Some(Tag::Int(amount)) => !(0..=TANK_MB).contains(amount),
                _ => false,
            },
            _ => false,
        })
    {
        write_tank(&mut patch, read_tank(saved));
        changed = true;
    }
    changed.then_some(patch)
}

// ---------------------------------------------------------------------------
// The commands
// ---------------------------------------------------------------------------

/// `/industrialforegoing energy`/`progress` on the crusher: one int of its saved compound.
pub(crate) fn set_int(host: &mut dyn ServerHost, pos: [i32; 3], key: &str, value: i32) -> bool {
    let Some(entity) = crate::our_entity(host, pos, BLOCK) else {
        return false;
    };
    let mut patch = Compound::new();
    write_int(&mut patch, key, value);
    entity.merge_data(host, patch).unwrap_or(false)
}

/// `/industrialforegoing drain`: take the tank out through the declared capability, which is what
/// makes it a handler rather than a private counter.
pub(crate) fn drain(host: &mut dyn ServerHost, pos: [i32; 3], amount: i32) {
    let want = u32::try_from(amount.max(0)).unwrap_or(u32::MAX);
    let answer = host
        .transaction::<String, ()>(|host, tx| {
            let Some(level) = host.level(host.dimension()) else {
                return Ok(format!("Mob Crusher at {pos:?}: no drainable fluid."));
            };
            let Some(tanks) = level.capability(host, &FLUIDS, pos, Some(READ_FACE)) else {
                return Ok(format!("Mob Crusher at {pos:?}: no drainable fluid."));
            };
            let Some(resource) = tanks.resource(host, 0) else {
                return Ok(format!("Mob Crusher at {pos:?}: no drainable fluid."));
            };
            let taken = tanks.extract(host, tx, 0, &resource, want);
            if taken == 0 {
                return Ok(format!("Mob Crusher at {pos:?}: no drainable fluid."));
            }
            let name = match resource.fluid() {
                FluidIdentity::Resolved(fluid) => fluid.id(host),
                FluidIdentity::Unresolved(fluid) => fluid.id(host),
                _ => "an unknown fluid".to_owned(),
            };
            Ok(format!(
                "Mob Crusher at {pos:?}: drained {taken} mB of {name}."
            ))
        })
        .unwrap_or_else(|()| format!("{pos:?} is not a mob crusher."));
    crate::say(host, &answer);
}

/// `/industrialforegoing info` for the crusher: the buffer, the tank as the entity holds it and as
/// the declared capability reports it, the bar, the eligible mobs in the working area, the output
/// slots, the last kill's experience, the facing and the game time.
pub(crate) fn readout(host: &mut dyn ServerHost, pos: [i32; 3]) -> String {
    let Some(entity) = crate::our_entity(host, pos, BLOCK) else {
        return format!("{pos:?} is not a mob crusher.");
    };
    let Some(data) = entity.data(&*host) else {
        return format!("{pos:?} is not a mod block entity.");
    };
    let essence = read_tank(&data);
    let progress = read_int(&data, KEY_PROGRESS);
    let augments = crate::read_augments(&data);
    let (cap_fluid, cap_mb) = capability_tank(host, pos);
    let victims = victim_count(host, pos);
    let output = output_contents(host, entity);
    let facing = entity
        .level(&*host)
        .and_then(|level| level.block_property(host, pos, SUBFACING))
        .unwrap_or_else(|| "unknown".to_owned());
    let time = entity
        .level(&*host)
        .and_then(|level| level.game_time(host))
        .unwrap_or(0);
    format!(
        "Mob Crusher at {pos:?}: {}/{} FE, tank {} {}/{TANK_MB} mB, capability {cap_fluid} {cap_mb} mB, progress {progress}/{}, victims {victims}, output {output}, last xp {}, facing {facing}, augments speed {} efficiency {:.2}, t={time}",
        read_int(&data, KEY_ENERGY),
        CAPACITY_FE,
        if essence > 0 { ESSENCE } else { "empty" },
        essence,
        Crusher::bar_length(augments),
        read_int(&data, KEY_LAST_XP),
        augments.speed,
        augments.efficiency,
    )
}

/// The eighteen output slots as `"2x minecraft:beef, 1x minecraft:leather"`, or `empty`.
fn output_contents(host: &mut dyn ServerHost, entity: BlockEntityRef) -> String {
    let mut parts = Vec::new();
    for slot in 0..OUTPUT_SLOTS as u32 {
        let Ok(Some(stack)) = entity.item_slot(&*host, slot) else {
            continue;
        };
        parts.push(format!("{}x {}", stack.count(), stack.item().id(&*host)));
    }
    if parts.is_empty() {
        "empty".to_owned()
    } else {
        parts.join(", ")
    }
}

/// How many eligible mobs the machine's working area holds right now — the counterfactual's read.
fn victim_count(host: &mut dyn ServerHost, pos: [i32; 3]) -> usize {
    let Some(level) = host.level(host.dimension()) else {
        return 0;
    };
    let Some(facing) = level.block_property(&*host, pos, SUBFACING) else {
        return 0;
    };
    let Some([dx, dy, dz]) = behind(&facing) else {
        return 0;
    };
    let [x, y, z] = [pos[0] + dx, pos[1] + dy, pos[2] + dz];
    let min = [f64::from(x), f64::from(y), f64::from(z)];
    let max = [
        f64::from(x) + 1.0,
        f64::from(y + 1 + AREA_UP),
        f64::from(z) + 1.0,
    ];
    let Some(entities) = level.entities_in(&*host, min, max) else {
        return 0;
    };
    let mut count = 0;
    for entity in entities {
        let alive = host
            .living(entity)
            .and_then(|living| living.alive(&*host))
            .unwrap_or(false);
        if !alive {
            continue;
        }
        if candidate_of(host, entity).is_some_and(crate::crusher::eligible) {
            count += 1;
        }
    }
    count
}

/// The tank as the declared fluid capability reports it.
fn capability_tank(host: &mut dyn ServerHost, pos: [i32; 3]) -> (String, i32) {
    let none = ("none".to_owned(), 0);
    let Some(level) = host.level(host.dimension()) else {
        return none;
    };
    let Some(tanks) = level.capability(host, &FLUIDS, pos, Some(READ_FACE)) else {
        return none;
    };
    let Some(resource) = tanks.resource(host, 0) else {
        return ("empty".to_owned(), 0);
    };
    let name = match resource.fluid() {
        FluidIdentity::Resolved(fluid) => fluid.id(host),
        FluidIdentity::Unresolved(fluid) => fluid.id(host),
        _ => "unknown".to_owned(),
    };
    let amount = i32::try_from(tanks.amount(host, 0)).unwrap_or(i32::MAX);
    (name, amount)
}

/// The block entity type the declaration made, for the augment seat.
pub(crate) fn kind_of(entity: BlockEntityRef, host: &dyn Host) -> bool {
    entity
        .kind(host)
        .is_some_and(|kind| KIND.get() == Some(&kind))
}

/// The item `block_drops` may write to.
pub(crate) fn item() -> Option<ItemKey> {
    ITEM.get().copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_crusher_declares_one_thirty_two_thousand_mb_tank() {
        let tanks = tanks();
        assert_eq!(tanks.tanks, vec![TANK_MB]);
        assert!(tanks.inputs.is_empty(), "Action.DRAIN: no face may fill it");
        assert_eq!(tanks.outputs, vec![0]);
        let every = vec![0usize];
        assert_eq!(tanks.faces.to_vec(), vec![every; 6]);
    }

    #[test]
    fn the_output_container_declares_eighteen_output_only_slots() {
        let container = container();
        assert_eq!(container.slots, 18);
        assert!(
            container.inputs.is_empty(),
            "setInputFilter((stack, slot) -> false)"
        );
        assert_eq!(container.outputs, (0..18).collect::<Vec<_>>());
        assert_eq!(container.faces.len(), 6);
    }

    #[test]
    fn the_tank_round_trips_through_the_saved_compound() {
        let mut nbt = Compound::new();
        assert_eq!(read_tank(&nbt), 0);
        write_tank(&mut nbt, 40);
        assert_eq!(read_tank(&nbt), 40);
        write_tank(&mut nbt, 0);
        assert_eq!(read_tank(&nbt), 0);
    }

    #[test]
    fn a_fluid_in_the_wrong_tank_reads_as_empty() {
        let mut nbt = Compound::new();
        let mut entry = Compound::new();
        entry.insert("Fluid".into(), Tag::String("minecraft:water".into()));
        entry.insert("Amount".into(), Tag::Int(500));
        nbt.insert("fluids".into(), Tag::List(vec![Tag::Compound(entry)]));
        assert_eq!(read_tank(&nbt), 0, "the tank only holds its own fluid");
    }

    #[test]
    fn the_working_area_is_the_block_behind_the_machine() {
        assert_eq!(behind("west"), Some([1, 0, 0]));
        assert_eq!(behind("east"), Some([-1, 0, 0]));
        assert_eq!(behind("north"), Some([0, 0, 1]));
        assert_eq!(behind("south"), Some([0, 0, -1]));
        assert_eq!(behind("up"), None, "FOUR_WAY has no vertical facing");
    }

    #[test]
    fn a_broken_crusher_carries_its_tank_buffer_and_output() {
        let mut nbt = Compound::new();
        assert!(!has_contents(&nbt), "a fresh crusher carries nothing");
        write_tank(&mut nbt, 40);
        assert!(has_contents(&nbt));
        write_int(&mut nbt, KEY_ENERGY, 4_600);
        write_int(&mut nbt, KEY_LAST_XP, 2);
        let patch = carried_patch(&nbt);
        let Some(Tag::Compound(payload)) = patch.get(&"minecraft:block_entity_data".into()) else {
            panic!("no block entity data in the patch");
        };
        assert_eq!(
            payload.get(&"id".into()),
            Some(&Tag::String(ENTITY_ID.into()))
        );
        assert_eq!(read_tank(payload), 40);
        assert_eq!(read_int(payload, KEY_ENERGY), 4_600);
    }

    #[test]
    fn a_placed_crusher_clamps_what_its_item_carried() {
        let mut over = Compound::new();
        write_int(&mut over, KEY_ENERGY, 50_000);
        write_int(&mut over, KEY_PROGRESS, 4_000);
        let mut entry = Compound::new();
        entry.insert("Fluid".into(), Tag::String(ESSENCE.into()));
        entry.insert("Amount".into(), Tag::Int(999_999));
        over.insert("fluids".into(), Tag::List(vec![Tag::Compound(entry)]));
        let patch = clamp_patch(&over).expect("out-of-range values are corrected");
        assert_eq!(read_int(&patch, KEY_ENERGY), CAPACITY_FE);
        assert_eq!(read_int(&patch, KEY_PROGRESS), crate::crusher::MAX_PROGRESS);
        assert_eq!(read_tank(&patch), TANK_MB);
        assert!(
            clamp_patch(&Compound::new()).is_none(),
            "a fresh crusher is left alone"
        );
    }

    #[test]
    fn a_fake_player_is_stable_per_machine_and_differs_between_them() {
        let first = fake_uuid("minecraft:the_end", [250, 101, 0]);
        assert_eq!(first, fake_uuid("minecraft:the_end", [250, 101, 0]));
        assert_ne!(first, fake_uuid("minecraft:the_end", [251, 101, 0]));
        assert_ne!(first, fake_uuid("minecraft:the_nether", [250, 101, 0]));
        assert_ne!(first, 0, "a zero UUID would collide in the factory's map");
    }

    #[test]
    fn an_attempt_is_forgotten_and_a_harvest_is_taken() {
        record_attempt(Attempt {
            entity: 7,
            dimension: "minecraft:the_end".into(),
            pos: [250, 101, 0],
            made: 1,
        });
        assert_eq!(
            attempt_for(7).map(|attempt| attempt.pos),
            Some([250, 101, 0])
        );
        prune_attempts("minecraft:the_end", [250, 101, 0], 1 + ATTEMPT_TTL + 1);
        assert!(
            attempt_for(7).is_none(),
            "an attempt no death answered is pruned"
        );
        queue_gain("minecraft:the_end", [250, 101, 0], 2);
        queue_gain("minecraft:the_end", [9, 9, 9], 3);
        let (_, gains) = take_harvest("minecraft:the_end", [250, 101, 0]);
        assert_eq!(gains.len(), 1, "only this machine's gain is taken");
        assert_eq!(gains[0].points, 2);
        let (_, left) = take_harvest("minecraft:the_end", [9, 9, 9]);
        assert_eq!(
            left.len(),
            1,
            "another machine's gain waits for its own tick"
        );
    }

    #[test]
    fn the_constants_are_the_config_s_own() {
        assert_eq!(crate::crusher::POWER_PER_OPERATION, 50);
        assert_eq!(CAPACITY_FE, 10_000);
        assert_eq!(TANK_MB, 32_000);
        assert_eq!(crate::crusher::ATTACK_DAMAGE, 75.0);
        assert_eq!(crate::crusher::ESSENCE_PER_POINT, 20);
        assert_eq!(OUTPUT_SLOTS, 18);
        assert_eq!(AREA_UP, 2);
    }
}
