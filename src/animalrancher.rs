//! The game side of the **Animal Rancher** (`ModuleAgricultureHusbandry.ANIMAL_RANCHER`).
//!
//! Upstream's `AnimalRancherTile` is an `IndustrialAreaWorkingTile`: a bar that counts **down** from
//! 100, a **10,000 FE** buffer, an **8,000 mB** tank (`SidedFluidTankComponent` with no action set,
//! i.e. Titanium's `BOTH`) and a **15-slot** output container (`5 * 3`, no filters: Titanium's
//! sides default to `FaceMode.ENABLED` with an accept-all predicate). Its working area is
//! `RangeManager(BEHIND).get(0)` — the **single block behind** the machine; unlike the crusher's it
//! is not expanded upward. At zero it spends 400 FE on the first of three arms that answers: a
//! sheep's wool, a squid's dye, a cow's or goat's bucket. The arithmetic is [`crate::rancher`].
//!
//! # What the pin cannot do, and what the port does instead
//!
//! - The jar's shearing is NeoForge's `IShearable`/`Shearable` interface through a fake player's
//!   shears. The pin has no `Shearable` read and no shear call, so the port names the one shearable
//!   whose result this slice ships, the **sheep**, by its type id, reads its `Sheared` flag and
//!   `Color` from `entity_nbt` (`saveWithoutId`), inserts `1 + nextInt(3)` wool of that colour and
//!   writes `Sheared: 1b` back with `entity_merge_nbt` (the `/data merge entity` path). The
//!   mooshroom, snow golem and bogged shears are entity-specific conversions and are recorded as a
//!   gap.
//! - The jar's milking is `mobInteract` with a **fresh bucket in a fake player's hand**, whose milk
//!   bucket is drained into the tank. The pin has no player-item interaction with an arbitrary
//!   entity, so the port fills the tank with the same **1,000 mB** of milk — `industrialforegoing:milk`,
//!   because 26.3's fluid registry has only empty/water/lava (deviation, `notes.md`) — for the two
//!   vanilla animals whose bucket interaction yields it (`minecraft:cow`, `minecraft:goat`, both
//!   grown), and refuses a baby exactly as vanilla's `!isBaby()` does.
//! - The squid's dye arm keeps the jar's gate exactly: a free output slot and four
//!   `level.random.nextBoolean()` draws (`1/16`). The two draws are made once per **attempt**
//!   (the tick whose fall reaches zero), not once per tick, and `/industrialforegoing draws` can pin
//!   them for one attempt.

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

use mod_api::{
    self, BlockEntityPart, BlockEntityRef, BlockEntityTickEvent, BlockEntityTicksEvent, BlockPart,
    Compound, Direction, EntityRef, FLUIDS, FluidIdentity, Host, HostTransaction, ItemKey,
    ItemPart, ItemStack, LevelHost, ModContainer, ModEnergyBuffer, ModFluidTanks,
    OnBlockPlacedByEvent, RandomSource, Registrar, ServerHost, StateForPlacementEvent, Tag,
};

use crate::rancher::{
    Anatomy, CAPACITY_FE, Draws, MAX_PROGRESS, MILK, OUTPUT_SLOTS, Rancher, Step, TANK_MB, Wool,
};

/// The machine (`ModuleAgricultureHusbandry.ANIMAL_RANCHER`).
pub const BLOCK: &str = "industrialforegoing:animal_rancher";
/// Its block entity type, a bare name so the mod's namespace qualifies it.
pub const ENTITY: &str = "animal_rancher";
/// The block entity type's registry id, which `minecraft:block_entity_data` tags its payload with.
pub const ENTITY_ID: &str = "industrialforegoing:animal_rancher";
/// The state property Titanium's `RotationType.FOUR_WAY` sets.
pub const SUBFACING: &str = "subfacing";
/// The saved keys: the buffer and the bar (the tank is the pin's `fluids` list).
const KEY_ENERGY: &str = "energy";
const KEY_PROGRESS: &str = "progress";
/// `Sheep.addAdditionalSaveData`'s two keys.
const KEY_SHEARED: &str = "Sheared";
const KEY_COLOUR: &str = "Color";
/// The side the tank is read through (all six reach it).
const READ_FACE: Direction = Direction::Up;

/// The block entity type and the item, resolved once in `register`: `block_drops` and the command
/// path run with a read-only host, which cannot name either.
static KIND: OnceLock<mod_api::BlockEntityTypeKey> = OnceLock::new();
static ITEM: OnceLock<ItemKey> = OnceLock::new();

// ---------------------------------------------------------------------------
// The declarations
// ---------------------------------------------------------------------------

/// Declare the machine: its block entity type, its 10,000 FE buffer, its 8,000 mB both-ways tank,
/// its 15-slot container, the Part that ticks it and the Part on its item.
pub(crate) fn declare(r: &mut dyn Registrar) {
    let block = r.block_key(BLOCK);
    let item = r.item_key(BLOCK);
    let _ = ITEM.set(item);
    r.item_part(item, RancherItem);
    let kind = r.block_entity_type(ENTITY).blocks(&[ENTITY]).build();
    let _ = KIND.set(kind);
    r.block_entity_part(&kind, RancherPart)
        .expect("the rancher's Part attaches to the type this mod just declared");
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

/// The one tank: `SidedFluidTankComponent("fluid_output", 8000, …, 0)` with **no** `setTankAction`,
/// which is Titanium's default `Action.BOTH` — every face may fill and drain it.
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
    tanks.inputs = vec![0];
    tanks.outputs = vec![0];
    tanks
}

/// The fifteen slots: `SidedInventoryComponent("output", …, 5 * 3, 1)` with no filter, so every
/// face reaches them both ways (Titanium's default).
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
    container.inputs = (0..OUTPUT_SLOTS).collect();
    container.outputs = (0..OUTPUT_SLOTS).collect();
    container
}

// ---------------------------------------------------------------------------
// The harness's pinned draws
// ---------------------------------------------------------------------------

/// A machine by dimension and position: the key of everything this module keeps outside the save.
type Place = ([i32; 3], String);

/// The draws `/industrialforegoing draws` pinned, one per position, spent by the next attempt.
/// Without one the attempt draws from the level's own random stream, as the jar does.
static PINNED_DRAWS: Mutex<BTreeMap<Place, Draws>> = Mutex::new(BTreeMap::new());

/// Pin this position's next attempt's two draws.
pub(crate) fn pin_draws(dimension: &str, pos: [i32; 3], draws: Draws) {
    if let Ok(mut pinned) = PINNED_DRAWS.lock() {
        pinned.insert((pos, dimension.to_owned()), draws);
    }
}

/// The pinned draws for this attempt, once, or `None`.
fn take_pinned(dimension: &str, pos: [i32; 3]) -> Option<Draws> {
    PINNED_DRAWS
        .lock()
        .ok()
        .and_then(|mut pinned| pinned.remove(&(pos, dimension.to_owned())))
}

/// This attempt's two draws: the pin, else `level.random.nextInt(3)` and four `nextBoolean()`s.
fn next_draws(host: &mut dyn ServerHost, dimension: &str, pos: [i32; 3]) -> Draws {
    if let Some(draws) = take_pinned(dimension, pos) {
        return draws;
    }
    let Some(level) = host.level(host.dimension()) else {
        return Draws::default();
    };
    level
        .with_random(host, |random| {
            let wool = random
                .next_int_bound(crate::rancher::WOOL_EXTRA)
                .unwrap_or(0);
            let dye = random.next_boolean()
                && random.next_boolean()
                && random.next_boolean()
                && random.next_boolean();
            Draws { wool, dye }
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// The Part
// ---------------------------------------------------------------------------

/// The rancher: its placement rotation, the augment seat and its tick.
pub(crate) struct RancherPart;

#[mod_api::horizon_part]
impl BlockPart for RancherPart {
    /// `RotationType.FOUR_WAY`: a placed rancher faces its placer, and its working area is the block
    /// behind it.
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
impl BlockEntityPart for RancherPart {
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    /// One server tick: fall the bar and, at zero, run one attempt over the block behind.
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let dimension = host.dimension().to_owned();
        let entity = &mut *event.entity;
        // A container the save left short comes back short (the chamber's measured case): pad the
        // view so the output slots are always the declared fifteen.
        if entity.items.len() < OUTPUT_SLOTS {
            entity.items.resize(OUTPUT_SLOTS, None);
        }
        let (tank_mb, milk) = read_tank(&entity.nbt);
        let mut rancher = Rancher::from_saved(
            read_int(&entity.nbt, KEY_ENERGY),
            read_progress(&entity.nbt),
            tank_mb,
            milk,
        );
        let before_tank = (rancher.tank_mb, rancher.milk);
        let augments = crate::read_augments(&entity.nbt);
        let found = animals_in(host, pos);
        let output_full = entity.items.iter().all(Option::is_some);
        // The draws belong to the **attempt**, not to every tick: the bar reaches zero this tick
        // only when it is within one speed-augmented step, and a pin set by the harness is spent
        // by that attempt alone (`/industrialforegoing draws`).
        let at_work = rancher.progress <= augments.speed.max(1);
        let draws = if at_work {
            next_draws(host, &dimension, pos)
        } else {
            Draws::default()
        };
        let step = rancher.tick(augments, &found.animals, output_full, draws);
        match step {
            Step::Shear { at, colour, count } => {
                let Some(item) = item_key(host, wool(colour)) else {
                    return;
                };
                if let Some(stack) = ItemStack::new(item, u32::try_from(count).unwrap_or(1)) {
                    crate::dissolution::put(&*host, &mut entity.items, stack);
                }
                if let Some(sheep) = found.entities.get(at) {
                    shear(host, *sheep);
                }
            }
            Step::Dye => {
                if let Some(item) = item_key(host, "minecraft:black_dye")
                    && let Some(stack) = ItemStack::new(item, 1)
                {
                    crate::dissolution::put(&*host, &mut entity.items, stack);
                }
            }
            Step::Milk { mb } => {
                rancher.fill_milk(mb);
            }
            Step::Working | Step::Idle => {}
        }
        write_int(&mut entity.nbt, KEY_ENERGY, rancher.energy);
        write_int(&mut entity.nbt, KEY_PROGRESS, rancher.progress);
        // Only a tank this machine filled is rewritten: a fluid a pipe put there keeps its own id
        // (a tank holds one fluid, and this machine's write must not turn a pipe's lava into water).
        if (rancher.tank_mb, rancher.milk) != before_tank {
            write_tank(&mut entity.nbt, rancher.tank_mb, rancher.milk);
        }
    }
}

/// The wool item of a colour.
#[must_use]
pub fn wool(colour: Wool) -> &'static str {
    colour.item()
}

/// An item id resolved through the game's registry.
fn item_key(host: &dyn Host, id: &str) -> Option<ItemKey> {
    host.item_key(&mod_api::Identifier::parse(id).ok()?)
}

/// What stands in the working area this attempt: the arms the rules decide on, and the entities
/// behind them (the shear's sheep is the one at the rule's index).
struct Found {
    animals: Vec<Anatomy>,
    entities: Vec<EntityRef>,
}

/// Every grown mob in the block behind the machine that answers an arm, in the level's order.
fn animals_in(host: &mut dyn ServerHost, pos: [i32; 3]) -> Found {
    let mut found = Found {
        animals: Vec::new(),
        entities: Vec::new(),
    };
    let Some((min, max)) = working_area_of(host, pos) else {
        return found;
    };
    let Some(level) = host.level(host.dimension()) else {
        return found;
    };
    let Some(entities) = level.entities_in(&*host, min, max) else {
        return found;
    };
    for entity in entities {
        let Some(living) = host.living(entity) else {
            continue;
        };
        if !living.alive(&*host).unwrap_or(false) {
            continue;
        }
        let Some(kind) = entity.kind(&*host) else {
            continue;
        };
        let kind = kind.id(&*host);
        let baby = living.baby(&*host).unwrap_or(false);
        // Only the sheep's arm reads the entity's saved state; anything else costs no read.
        let (sheared, colour) = if kind == "minecraft:sheep" {
            sheep_state(host, entity)
        } else {
            (false, 0)
        };
        let anatomy = crate::rancher::anatomy(&kind, sheared, colour, baby);
        if anatomy == Anatomy::Inert {
            continue;
        }
        found.animals.push(anatomy);
        found.entities.push(entity);
    }
    found
}

/// `Sheep`'s saved `Sheared` and `Color` (`addAdditionalSaveData`: a boolean and a dye id).
fn sheep_state(host: &mut dyn ServerHost, entity: EntityRef) -> (bool, i32) {
    let Some(nbt) = entity.nbt(host) else {
        return (false, 0);
    };
    (read_flag(&nbt, KEY_SHEARED), read_int(&nbt, KEY_COLOUR))
}

/// Shear a sheep: `Sheared: 1b` merged into its saved data (vanilla's `addAdditionalSaveData`), which
/// is what makes it bare — `isShearable` is `!isSheared()`, so the next attempt has no wool arm for
/// it. The pin has no `setSheared`, so this is the entity's own data.
fn shear(host: &mut dyn ServerHost, sheep: EntityRef) {
    let mut patch = Compound::new();
    patch.insert(KEY_SHEARED.into(), Tag::Byte(1));
    let _ = sheep.merge_nbt(host, patch);
}

/// `RangeManager(BEHIND).get(0)`: the single block behind the machine, `(min, max)` for
/// `entities_in`.
#[must_use]
pub fn working_area(pos: [i32; 3], facing: &str) -> Option<([f64; 3], [f64; 3])> {
    let [dx, dy, dz] = behind(facing)?;
    let [x, y, z] = [pos[0] + dx, pos[1] + dy, pos[2] + dz];
    Some((
        [f64::from(x), f64::from(y), f64::from(z)],
        [f64::from(x) + 1.0, f64::from(y) + 1.0, f64::from(z) + 1.0],
    ))
}

/// The working area of a placed machine, read from its own `subfacing`.
fn working_area_of(host: &mut dyn ServerHost, pos: [i32; 3]) -> Option<([f64; 3], [f64; 3])> {
    let level = host.level(host.dimension())?;
    let facing = level.block_property(&*host, pos, SUBFACING)?;
    working_area(pos, &facing)
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

// ---------------------------------------------------------------------------
// The machine's item
// ---------------------------------------------------------------------------

/// The rancher's item: a broken machine's drop carries its buffer, tank and bar, and placing it
/// clamps the numbers back to the capacities (slice 2's route). The output slots are a declared
/// container and drop beside the block, so the payload never carries `Items`.
pub(crate) struct RancherItem;

#[mod_api::horizon_part]
impl ItemPart for RancherItem {
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
        Some(Tag::Byte(value)) => i32::from(*value),
        Some(Tag::Short(value)) => i32::from(*value),
        _ => 0,
    }
}

fn write_int(nbt: &mut Compound, key: &str, value: i32) {
    nbt.insert(key.into(), Tag::Int(value));
}

/// A saved boolean: 1.21's `putBoolean` writes a byte, older data an int.
fn read_flag(nbt: &Compound, key: &str) -> bool {
    read_int(nbt, key) != 0
}

/// The bar from the save: a key the save never wrote is a **full** bar, not an empty one.
fn read_progress(nbt: &Compound) -> Option<i32> {
    match nbt.get(&KEY_PROGRESS.into()) {
        Some(Tag::Int(value)) => Some(*value),
        _ => None,
    }
}

/// The tank from the entity's `fluids`: its amount, and whether the fluid is this machine's milk
/// (a pipe may fill the tank with water — Titanium's default action is `BOTH` — and a tank holds
/// one fluid).
fn read_tank(nbt: &Compound) -> (i32, bool) {
    let Some(Tag::List(entries)) = nbt.get(&"fluids".into()) else {
        return (0, true);
    };
    for entry in entries {
        let Tag::Compound(entry) = entry else {
            continue;
        };
        let fluid = match entry.get(&"Fluid".into()) {
            Some(Tag::String(fluid)) => fluid.to_string_lossy(),
            _ => continue,
        };
        let amount = match entry.get(&"Amount".into()) {
            Some(Tag::Int(amount)) => *amount,
            _ => 0,
        };
        if fluid == "minecraft:empty" {
            return (0, true);
        }
        return (amount.clamp(0, TANK_MB), fluid == MILK);
    }
    (0, true)
}

/// The tank back in the pin's shape.
fn write_tank(nbt: &mut Compound, mb: i32, milk: bool) {
    let mb = mb.clamp(0, TANK_MB);
    let fluid = if mb == 0 {
        "minecraft:empty"
    } else if milk {
        MILK
    } else {
        // A foreign fluid the tank already held is kept, not overwritten with milk.
        "minecraft:water"
    };
    let mut entry = Compound::new();
    entry.insert("Fluid".into(), Tag::String(fluid.into()));
    entry.insert("Amount".into(), Tag::Int(mb));
    nbt.insert("fluids".into(), Tag::List(vec![Tag::Compound(entry)]));
}

/// Whether the rancher holds anything worth carrying on its item.
pub(crate) fn has_contents(saved: &Compound) -> bool {
    let progress = read_progress(saved).unwrap_or(MAX_PROGRESS);
    read_int(saved, KEY_ENERGY) > 0
        || progress != MAX_PROGRESS
        || read_tank(saved).0 > 0
        || crate::has_seated_augment(saved)
}

/// The `minecraft:block_entity_data` payload a broken rancher's item carries, without the output
/// slots (they are a declared container and drop beside the block).
#[must_use]
pub(crate) fn carried_patch(saved: &Compound) -> Compound {
    let mut payload = saved.clone();
    payload.remove(&"Items".into());
    payload.insert("id".into(), Tag::String(ENTITY_ID.into()));
    let mut patch = Compound::new();
    patch.insert("minecraft:block_entity_data".into(), Tag::Compound(payload));
    patch
}

/// The corrections a placed rancher needs when its item carried values outside the capacities.
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
    if !(0..=MAX_PROGRESS).contains(&progress) {
        write_int(&mut patch, KEY_PROGRESS, progress.clamp(0, MAX_PROGRESS));
        changed = true;
    }
    let (tank_mb, milk) = read_tank(saved);
    let over = matches!(
        saved.get(&"fluids".into()),
        Some(Tag::List(entries)) if entries.iter().any(|entry| matches!(
            entry,
            Tag::Compound(entry) if matches!(entry.get(&"Amount".into()), Some(Tag::Int(amount)) if !(0..=TANK_MB).contains(amount))
        ))
    );
    if over {
        write_tank(&mut patch, tank_mb, milk);
        changed = true;
    }
    changed.then_some(patch)
}

// ---------------------------------------------------------------------------
// The commands
// ---------------------------------------------------------------------------

/// `/industrialforegoing energy`/`progress`/`milk` on the rancher.
pub(crate) fn set_int(host: &mut dyn ServerHost, pos: [i32; 3], key: &str, value: i32) -> bool {
    let Some(entity) = crate::our_entity(host, pos, BLOCK) else {
        return false;
    };
    let mut patch = Compound::new();
    write_int(&mut patch, key, value);
    entity.merge_data(host, patch).unwrap_or(false)
}

/// `/industrialforegoing milk`: write the tank (a mod fluid has no insert path at this pin, the
/// chamber's measured route).
pub(crate) fn set_milk(host: &mut dyn ServerHost, pos: [i32; 3], amount: i32) -> bool {
    let Some(entity) = crate::our_entity(host, pos, BLOCK) else {
        return false;
    };
    let mut patch = Compound::new();
    write_tank(&mut patch, amount, true);
    entity.merge_data(host, patch).unwrap_or(false)
}

/// `/industrialforegoing drain`: take milk out through the declared fluid capability.
pub(crate) fn drain(host: &mut dyn ServerHost, pos: [i32; 3], amount: i32) {
    let want = u32::try_from(amount.max(0)).unwrap_or(u32::MAX);
    let answer = host
        .transaction::<String, ()>(|host, tx| {
            let Some(level) = host.level(host.dimension()) else {
                return Ok(format!("Animal Rancher at {pos:?}: no drainable fluid."));
            };
            let Some(tanks) = level.capability(host, &FLUIDS, pos, Some(READ_FACE)) else {
                return Ok(format!("Animal Rancher at {pos:?}: no drainable fluid."));
            };
            let Some(resource) = tanks.resource(host, 0) else {
                return Ok(format!("Animal Rancher at {pos:?}: no drainable fluid."));
            };
            let taken = tanks.extract(host, tx, 0, &resource, want);
            if taken == 0 {
                return Ok(format!("Animal Rancher at {pos:?}: no drainable fluid."));
            }
            let name = match resource.fluid() {
                FluidIdentity::Resolved(fluid) => fluid.id(host),
                FluidIdentity::Unresolved(fluid) => fluid.id(host),
                _ => "an unknown fluid".to_owned(),
            };
            Ok(format!(
                "Animal Rancher at {pos:?}: drained {taken} mB of {name}."
            ))
        })
        .unwrap_or_else(|()| format!("{pos:?} is not an animal rancher."));
    crate::say(host, &answer);
}

/// `/industrialforegoing info` for the rancher: the buffer, the tank as the entity holds it and as
/// the declared capability reports it, the working area's arms, the output slots, the bar, the
/// augments, the facing and the game time.
pub(crate) fn readout(host: &mut dyn ServerHost, pos: [i32; 3]) -> String {
    let Some(entity) = crate::our_entity(host, pos, BLOCK) else {
        return format!("{pos:?} is not an animal rancher.");
    };
    let Some(data) = entity.data(&*host) else {
        return format!("{pos:?} is not a mod block entity.");
    };
    let (tank_mb, milk) = read_tank(&data);
    let progress = read_int(&data, KEY_PROGRESS);
    let augments = crate::read_augments(&data);
    let (cap_fluid, cap_mb) = capability_tank(host, pos);
    let found = animals_in(host, pos);
    let animals = found.animals.len();
    let sheep = found
        .animals
        .iter()
        .filter(|animal| matches!(animal, Anatomy::Sheep { .. }))
        .count();
    let cows = found
        .animals
        .iter()
        .filter(|animal| matches!(animal, Anatomy::Milker))
        .count();
    let squid = found
        .animals
        .iter()
        .filter(|animal| matches!(animal, Anatomy::Squid))
        .count();
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
        "Animal Rancher at {pos:?}: {}/{} FE, tank {} {}/{TANK_MB} mB, capability {cap_fluid} {cap_mb} mB, animals {animals} (sheep {sheep}, milkable {cows}, squid {squid}), output {output}, progress {progress}/{}, facing {facing}, augments speed {} efficiency {:.2}, t={time}",
        read_int(&data, KEY_ENERGY),
        CAPACITY_FE,
        if tank_mb > 0 {
            if milk { MILK } else { "foreign" }
        } else {
            "empty"
        },
        tank_mb,
        Rancher::bar_length(augments),
        augments.speed,
        augments.efficiency,
    )
}

/// The fifteen output slots as `"2x minecraft:white_wool, 1x minecraft:black_dye"`, or `empty`.
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
    use crate::rancher::anatomy;

    #[test]
    fn the_tank_is_eight_thousand_mb_and_both_ways() {
        let tanks = tanks();
        assert_eq!(tanks.tanks, vec![8_000]);
        assert_eq!(tanks.inputs, vec![0], "Titanium's default action is BOTH");
        assert_eq!(tanks.outputs, vec![0], "every face may drain it");
        let every = vec![0usize];
        assert_eq!(tanks.faces.to_vec(), vec![every; 6]);
    }

    #[test]
    fn the_container_declares_fifteen_slots_both_ways() {
        let container = container();
        assert_eq!(container.slots, 15);
        assert_eq!(container.inputs, (0..15).collect::<Vec<_>>());
        assert_eq!(container.outputs, (0..15).collect::<Vec<_>>());
        assert_eq!(container.faces.len(), 6);
    }

    #[test]
    fn the_working_area_is_the_single_block_behind_the_machine() {
        let (min, max) = working_area([200, 101, 0], "west").expect("a four-way facing");
        assert_eq!(
            min,
            [201.0, 101.0, 0.0],
            "west faces its placer, east is behind"
        );
        assert_eq!(max, [202.0, 102.0, 1.0], "one block, never expanded upward");
        assert_eq!(
            working_area([200, 101, 0], "up"),
            None,
            "a machine without a subfacing has no area"
        );
    }

    #[test]
    fn the_tank_round_trips_through_the_saved_compound() {
        let mut nbt = Compound::new();
        assert_eq!(read_tank(&nbt), (0, true));
        write_tank(&mut nbt, 1_000, true);
        assert_eq!(read_tank(&nbt), (1_000, true));
        write_tank(&mut nbt, 0, true);
        assert_eq!(read_tank(&nbt), (0, true));
        let mut water = Compound::new();
        let mut entry = Compound::new();
        entry.insert("Fluid".into(), Tag::String("minecraft:water".into()));
        entry.insert("Amount".into(), Tag::Int(500));
        water.insert("fluids".into(), Tag::List(vec![Tag::Compound(entry)]));
        assert_eq!(
            read_tank(&water),
            (500, false),
            "a pipe's water is not milk"
        );
    }

    #[test]
    fn the_sheep_s_saved_state_reads_both_keys() {
        let mut nbt = Compound::new();
        nbt.insert(KEY_SHEARED.into(), Tag::Byte(1));
        nbt.insert(KEY_COLOUR.into(), Tag::Byte(14));
        assert!(read_flag(&nbt, KEY_SHEARED));
        assert_eq!(read_int(&nbt, KEY_COLOUR), 14);
        assert_eq!(
            anatomy(
                "minecraft:sheep",
                read_flag(&nbt, KEY_SHEARED),
                read_int(&nbt, KEY_COLOUR),
                false
            ),
            Anatomy::Inert,
            "a sheared sheep has no wool arm, which is what the write-back has to produce"
        );
        let fresh = Compound::new();
        assert!(!read_flag(&fresh, KEY_SHEARED));
        assert_eq!(
            anatomy(
                "minecraft:sheep",
                false,
                read_int(&fresh, KEY_COLOUR),
                false
            ),
            Anatomy::Sheep {
                colour: Wool::White
            }
        );
    }

    #[test]
    fn the_wool_items_are_the_vanilla_ones() {
        assert_eq!(wool(Wool::White), "minecraft:white_wool");
        assert_eq!(wool(Wool::Black), "minecraft:black_wool");
        assert_eq!(wool(Wool::LightBlue), "minecraft:light_blue_wool");
    }

    #[test]
    fn a_broken_rancher_carries_its_buffer_tank_and_bar_but_not_its_output() {
        let mut nbt = Compound::new();
        write_int(&mut nbt, KEY_ENERGY, 9_600);
        write_int(&mut nbt, KEY_PROGRESS, 3);
        write_tank(&mut nbt, 1_000, true);
        let mut wool = Compound::new();
        wool.insert("id".into(), Tag::String("minecraft:white_wool".into()));
        wool.insert("count".into(), Tag::Int(2));
        nbt.insert("Items".into(), Tag::List(vec![Tag::Compound(wool)]));
        assert!(has_contents(&nbt));
        let patch = carried_patch(&nbt);
        let Some(Tag::Compound(payload)) = patch.get(&"minecraft:block_entity_data".into()) else {
            panic!("no block entity data in the patch");
        };
        assert_eq!(
            payload.get(&"id".into()),
            Some(&Tag::String(ENTITY_ID.into()))
        );
        assert_eq!(read_tank(payload), (1_000, true));
        assert_eq!(read_int(payload, KEY_ENERGY), 9_600);
        assert!(
            payload.get(&"Items".into()).is_none(),
            "the output container drops beside the block, never twice"
        );
    }

    #[test]
    fn a_placed_rancher_clamps_what_its_item_carried() {
        let mut over = Compound::new();
        write_int(&mut over, KEY_ENERGY, 90_000);
        write_int(&mut over, KEY_PROGRESS, 4_000);
        let mut entry = Compound::new();
        entry.insert("Fluid".into(), Tag::String(MILK.into()));
        entry.insert("Amount".into(), Tag::Int(999_999));
        over.insert("fluids".into(), Tag::List(vec![Tag::Compound(entry)]));
        let patch = clamp_patch(&over).expect("out-of-range values are corrected");
        assert_eq!(read_int(&patch, KEY_ENERGY), CAPACITY_FE);
        assert_eq!(read_int(&patch, KEY_PROGRESS), MAX_PROGRESS);
        assert_eq!(read_tank(&patch), (TANK_MB, true));
        assert!(
            clamp_patch(&Compound::new()).is_none(),
            "a fresh rancher is left alone"
        );
    }

    #[test]
    fn a_pinned_draw_is_spent_by_exactly_one_attempt() {
        let draws = Draws { wool: 2, dye: true };
        pin_draws("minecraft:the_end", [200, 101, 0], draws);
        assert_eq!(take_pinned("minecraft:the_end", [200, 101, 0]), Some(draws));
        assert_eq!(
            take_pinned("minecraft:the_end", [200, 101, 0]),
            None,
            "one attempt, one pin"
        );
    }

    #[test]
    fn a_sheared_sheep_alone_has_nothing_to_carry() {
        let mut nbt = Compound::new();
        write_tank(&mut nbt, 0, true);
        write_int(&mut nbt, KEY_PROGRESS, MAX_PROGRESS);
        assert!(!has_contents(&nbt));
    }
}
