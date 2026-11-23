//! The game side of the **Mob Duplicator** (`ModuleAgricultureHusbandry.MOB_DUPLICATOR`) and its
//! **Mob Imprisonment Tool**.
//!
//! Upstream's `MobDuplicatorTile` is an `IndustrialAreaWorkingTile(TOP_UP)`: a bar that counts down
//! from 62, a 50,000 FE buffer, an 8,000 mB essence tank (`Action.FILL`) and a one-slot input
//! container whose filter is the Mob Imprisonment Tool. At zero it reads the tool's entity and, if
//! the working area holds 32 or fewer of that class, spawns `1 + random.nextInt(min(canSpawn, 4))`
//! fresh mobs for 5,000 FE, draining `health * 12` mB each. The arithmetic is [`crate::duplicator`].
//!
//! # The tool
//!
//! The item is the jar's `MobImprisonmentToolItem`: `stacksTo(1)`, a captured entity in the
//! `industrialforegoing:mob_imprisonment_tool` component as `{entity: "<type id>", …saveWithoutId()}`,
//! right-click a living entity to capture it, right-click a block face to release it. The machine
//! reads the same component. The blacklists are the jar's two empty entity-type tags; a pack may add
//! to them, and both are read through [`Host::entity_type_in_tag`].
//!
//! # The spawn
//!
//! The pin's `Host::summon` is the jar's `addFreshEntity`: a fresh entity of the tool's type (`exactCopy`
//! is false, its default) at the first clear block of the working column. The working area is
//! `RangeManager(TOP_UP)` — a 1-wide, two-block column two blocks above the machine, never rotated
//! (`MobDuplicatorBlock` has no `getRotationType()`).

use std::sync::OnceLock;

use mod_api::{
    self, BlockEntityPart, BlockEntityRef, BlockEntityTickEvent, BlockEntityTicksEvent, BlockPart,
    ComponentId, Compound, Direction, FLUIDS, FluidIdentity, Host, HostSide, InteractionResult,
    ItemKey, ItemPart, ItemResource, ItemStack, ModContainer, ModEnergyBuffer, ModFluidTanks,
    OnBlockPlacedByEvent, RandomSource, Registrar, ServerHost, Tag, Text, UseItemOnEvent,
    UseOnEvent,
};

use crate::duplicator::{
    Attempt, CAPACITY_FE, Duplicator, ESSENCE, INPUT_SLOTS, MAX_PROGRESS, Step, TANK_MB,
};

/// The machine (`ModuleAgricultureHusbandry.MOB_DUPLICATOR`).
pub const BLOCK: &str = "industrialforegoing:mob_duplicator";
/// Its block entity type, a bare name so the mod's namespace qualifies it.
pub const ENTITY: &str = "mob_duplicator";
/// The block entity type's registry id, which `minecraft:block_entity_data` tags its payload with.
pub const ENTITY_ID: &str = "industrialforegoing:mob_duplicator";
/// The item (`ModuleTool.MOB_IMPRISONMENT_TOOL`).
pub const TOOL: &str = "industrialforegoing:mob_imprisonment_tool";
/// The tool's captured-entity component (`IFAttachments.MOB_IMPRISONMENT_TOOL`).
pub const TOOL_COMPONENT: &str = "industrialforegoing:mob_imprisonment_tool";
/// `IndustrialTags.EntityTypes.MOB_IMPRISONMENT_TOOL_BLACKLIST`.
pub const TOOL_BLACKLIST: &str = "industrialforegoing:mob_imprisonment_tool_blacklist";
/// `IndustrialTags.EntityTypes.MOB_DUPLICATOR_BLACKLIST`.
pub const DUPLICATOR_BLACKLIST: &str = "industrialforegoing:mob_duplicator_blacklist";
/// The saved keys: the buffer, the bar and the tool's component's `entity`.
const KEY_ENERGY: &str = "energy";
const KEY_PROGRESS: &str = "progress";
const KEY_ENTITY: &str = "entity";
const KEY_HEALTH: &str = "Health";
/// The side the tank is read through (all six reach it).
const READ_FACE: Direction = Direction::Up;

/// The block entity type and the item, resolved once in `register`: `block_drops` and the command
/// path run with a read-only host, which cannot name either.
static KIND: OnceLock<mod_api::BlockEntityTypeKey> = OnceLock::new();
static ITEM: OnceLock<ItemKey> = OnceLock::new();

// ---------------------------------------------------------------------------
// The declarations
// ---------------------------------------------------------------------------

/// Declare the machine and the tool: the block entity type, the 50,000 FE buffer, the 8,000 mB
/// fill-only essence tank, the one-slot input container, the Part that ticks it, the Part on its
/// item and the Part on the tool.
pub(crate) fn declare(r: &mut dyn Registrar) {
    let block = r.block_key(BLOCK);
    let item = r.item_key(BLOCK);
    let _ = ITEM.set(item);
    r.item_part(item, DuplicatorItem);
    let tool = r.item_key(TOOL);
    r.item_part(tool, ToolItem);
    let kind = r.block_entity_type(ENTITY).blocks(&[ENTITY]).build();
    let _ = KIND.set(kind);
    r.block_entity_part(&kind, DuplicatorPart)
        .expect("the duplicator's Part attaches to the type this mod just declared");
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

/// The one tank: `SidedFluidTankComponent("essence", 8000, …, Action.FILL)`. Every face reaches it;
/// it may be filled but not drained (the machine spends it itself).
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
    tanks.outputs = Vec::new();
    tanks
}

/// The one slot: `SidedInventoryComponent("mob_imprisonment_tool", …, 1, 1)` with
/// `setInputFilter((stack, slot) -> stack.getItem() == MOB_IMPRISONMENT_TOOL)`. The pin's
/// `ModContainer` names slots and policies, not a per-slot filter (deviation, `notes.md`).
#[must_use]
pub fn container() -> ModContainer {
    let every = vec![0usize];
    let mut container = ModContainer {
        slots: INPUT_SLOTS,
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
    container.inputs = vec![0];
    container
}

// ---------------------------------------------------------------------------
// The Part
// ---------------------------------------------------------------------------

/// The duplicator: the augment seat and its tick.
pub(crate) struct DuplicatorPart;

#[mod_api::horizon_part]
impl BlockPart for DuplicatorPart {
    /// `IndustrialWorkingTile.canAcceptAugment`: both a speed and an efficiency addon seat, in the
    /// same two saved slots the other machines use.
    fn use_item_on(
        &self,
        host: &mut dyn ServerHost,
        event: &mut UseItemOnEvent<'_>,
    ) -> Option<InteractionResult> {
        crate::seat_from_click(host, event)
    }
}

#[mod_api::horizon_part]
impl BlockEntityPart for DuplicatorPart {
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    /// One server tick: fall the bar and, at zero, read the tool and spawn.
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let dimension = host.dimension().to_owned();
        let entity = &mut *event.entity;
        if entity.items.len() < INPUT_SLOTS {
            entity.items.resize(INPUT_SLOTS, None);
        }
        let mut duplicator = Duplicator::from_saved(
            read_int(&entity.nbt, KEY_ENERGY),
            read_progress(&entity.nbt),
            read_tank(&entity.nbt),
        );
        let augments = crate::read_augments(&entity.nbt);
        let found = attempt_for(host, pos, &entity.items, read_tank(&entity.nbt));
        let step = duplicator.tick(augments, found.as_ref().map(|found| found.attempt));
        if let (
            Step::Spawn {
                count,
                essence_each,
            },
            Some(found),
        ) = (step, found)
        {
            for _ in 0..count {
                let Some(at) = spawn_position(host, pos) else {
                    break;
                };
                let Some(_) = host.summon(&dimension, &found.kind, at, Compound::new()) else {
                    continue;
                };
                duplicator.spend_essence(essence_each);
            }
        }
        write_int(&mut entity.nbt, KEY_ENERGY, duplicator.energy);
        write_int(&mut entity.nbt, KEY_PROGRESS, duplicator.progress);
        write_tank(&mut entity.nbt, duplicator.essence_mb);
    }
}

/// What the machine found in its tool slot this attempt.
struct Found {
    attempt: Attempt,
    kind: String,
}

/// The tool's entity and the area's count, or `None` when the slot holds no readable, allowed tool.
fn attempt_for(
    host: &mut dyn ServerHost,
    pos: [i32; 3],
    slots: &[Option<ItemStack>],
    tank: i32,
) -> Option<Found> {
    let stack = slots.first()?.as_ref()?;
    let component = tool_of(&*host, stack)?;
    let kind = tool_kind(&component)?;
    let health = tool_health(&component)?;
    if host.entity_type_in_tag(&kind, DUPLICATOR_BLACKLIST) {
        return None;
    }
    let nearby = nearby_count(host, pos, &kind);
    let each = crate::duplicator::essence_for_health(health).max(1);
    let can_spawn = (tank / each).max(1);
    let bound = can_spawn.min(crate::duplicator::MAX_SPAWN);
    let draw = host
        .level(host.dimension())
        .and_then(|level| {
            level.with_random(host, |random| random.next_int_bound(bound).unwrap_or(0))
        })
        .unwrap_or(0);
    Some(Found {
        attempt: Attempt {
            health,
            nearby,
            draw,
        },
        kind,
    })
}

/// How many alive entities of `kind` the working column holds.
fn nearby_count(host: &mut dyn ServerHost, pos: [i32; 3], kind: &str) -> usize {
    let Some(level) = host.level(host.dimension()) else {
        return 0;
    };
    let (min, max) = working_area(pos);
    let Some(entities) = level.entities_in(&*host, min, max) else {
        return 0;
    };
    entities
        .into_iter()
        .filter(|entity| {
            host.living(*entity)
                .and_then(|living| living.alive(&*host))
                .unwrap_or(false)
                && entity
                    .kind(&*host)
                    .is_some_and(|found| found.id(&*host) == kind)
        })
        .count()
}

/// `RangeManager(TOP_UP)`: the 1-wide, two-block column two blocks above the machine, expanded one
/// block up — the box the jar counts and spawns in.
#[must_use]
pub fn working_area(pos: [i32; 3]) -> ([f64; 3], [f64; 3]) {
    let [x, y, z] = pos;
    (
        [f64::from(x), f64::from(y + 2), f64::from(z)],
        [f64::from(x + 1), f64::from(y + 4), f64::from(z + 1)],
    )
}

/// The first clear block of the working column, as the jar's `isUnobstructed` + `containsAnyLiquid`
/// check it: a mob-sized box with no collision and no fluid at its feet.
fn spawn_position(host: &mut dyn ServerHost, pos: [i32; 3]) -> Option<[f64; 3]> {
    let level = host.level(host.dimension())?;
    let [x, y, z] = pos;
    for dy in 2..4 {
        let feet = [x, y + dy, z];
        let box6 = [
            f64::from(x) + 0.05,
            f64::from(y + dy),
            f64::from(z) + 0.05,
            f64::from(x) + 0.95,
            f64::from(y + dy) + 1.8,
            f64::from(z) + 0.95,
        ];
        let clear = level
            .collision_boxes(&*host, box6)
            .is_some_and(|boxes| boxes.is_empty());
        let dry = level
            .fluid_state(&*host, feet)
            .is_some_and(|state| state.is_empty())
            && level
                .fluid_state(&*host, [x, y + dy + 1, z])
                .is_some_and(|state| state.is_empty());
        if clear && dry {
            return Some([f64::from(x) + 0.5, f64::from(y + dy), f64::from(z) + 0.5]);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// The tool's item
// ---------------------------------------------------------------------------

/// The Mob Imprisonment Tool: capture, release and its tooltip.
pub(crate) struct ToolItem;

#[mod_api::horizon_part]
impl ItemPart for ToolItem {
    fn item_interact_mob(
        &self,
        host: HostSide,
        event: &mut mod_api::ItemInteractMobEvent<'_>,
    ) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        if tool_of(&*host, event.stack).is_some() {
            return Some(InteractionResult::Fail);
        }
        let entity = host.entity(u32::try_from(event.entity).ok()?)?;
        let living = host.living(entity)?;
        if !living.alive(&*host).unwrap_or(false) {
            return Some(InteractionResult::Fail);
        }
        let kind = entity.kind(&*host)?.id(&*host);
        if host.entity_type_in_tag(&kind, TOOL_BLACKLIST)
            || host.entity_type_in_tag(&kind, "c:bosses")
            || host.entity_type_in_tag(&kind, "minecraft:bosses")
        {
            return Some(InteractionResult::Fail);
        }
        let nbt = entity.nbt(host)?;
        let payload = tool_payload(&kind, nbt);
        let component = ItemResource::of(event.stack)
            .with_component(&*host, &tool_component(), Tag::Compound(payload))
            .ok()?;
        let player = host.players().acting_player()?;
        let slot = held_slot(&*host, player, event.hand)?;
        if player
            .set_inventory_slot(host, slot, Some(component.as_stack().clone()))
            .is_err()
        {
            return Some(InteractionResult::Fail);
        }
        let _ = entity.discard(host);
        Some(InteractionResult::Success)
    }

    fn use_on(&self, host: HostSide, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        let Some(component) = tool_of(&*host, event.stack) else {
            return Some(InteractionResult::Fail);
        };
        let Some(kind) = tool_kind(&component) else {
            return Some(InteractionResult::Fail);
        };
        let mut nbt = component;
        nbt.remove(&KEY_ENTITY.into());
        let at = [
            f64::from(event.pos[0]) + f64::from(event.face[0]) + 0.5,
            f64::from(event.pos[1] + event.face[1]),
            f64::from(event.pos[2]) + f64::from(event.face[2]) + 0.5,
        ];
        let dimension = host.dimension().to_owned();
        if host.summon(&dimension, &kind, at, nbt).is_none() {
            return Some(InteractionResult::Fail);
        }
        let Some(player) = host.player(event.player) else {
            return Some(InteractionResult::Success);
        };
        let empty = ItemResource::of(event.stack)
            .without_component(&*host, &tool_component())
            .as_stack()
            .clone();
        if let Some(slot) = held_slot(&*host, player, event.hand) {
            let _ = player.set_inventory_slot(host, slot, Some(empty));
        }
        Some(InteractionResult::Success)
    }

    fn tooltip(&self, host: &dyn mod_api::ClientHost, event: &mut mod_api::TooltipEvent<'_>) {
        let Some(component) = tool_of(host, event.stack) else {
            return;
        };
        if let Some(kind) = tool_kind(&component) {
            event
                .lines
                .push((Text::literal(format!("Mob: {kind}")), 0xFF80_8080));
        }
        if let Some(health) = tool_health(&component) {
            event
                .lines
                .push((Text::literal(format!("Health: {health}")), 0xFF80_8080));
        }
    }
}

/// The inventory slot the hand uses: the selected hotbar slot for the main hand, 40 for the off.
fn held_slot(host: &dyn Host, player: mod_api::PlayerRef, hand: mod_api::Hand) -> Option<u32> {
    match hand {
        mod_api::Hand::Main => player.selected_slot(host).map(|slot| slot as u32),
        mod_api::Hand::Off => Some(40),
    }
}

// ---------------------------------------------------------------------------
// The machine's item
// ---------------------------------------------------------------------------

/// The duplicator's item: a broken machine's drop carries its buffer, tank and bar, and placing it
/// clamps the numbers back to the capacities (slice 2's route). The tool slot is a container and
/// drops beside the block, so the payload never carries `Items`.
pub(crate) struct DuplicatorItem;

#[mod_api::horizon_part]
impl ItemPart for DuplicatorItem {
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

/// The tank's amount from the entity's `fluids`, only while it holds essence.
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

/// The tool's component id.
#[must_use]
pub fn tool_component() -> ComponentId {
    ComponentId::new(TOOL_COMPONENT).expect("the tool's component id is namespaced")
}

/// The tool's component from a stack, only while it is this package's tool.
pub(crate) fn tool_of(host: &dyn Host, stack: &ItemStack) -> Option<Compound> {
    if stack.item().id(host) != TOOL {
        return None;
    }
    match ItemResource::of(stack).component(&tool_component()) {
        Some(Tag::Compound(component)) => Some(component),
        _ => None,
    }
}

/// The `{entity, …saveWithoutId()}` the tool holds: the jar's own shape.
#[must_use]
pub fn tool_payload(kind: &str, mut nbt: Compound) -> Compound {
    nbt.insert(KEY_ENTITY.into(), Tag::String(kind.into()));
    nbt
}

/// The captured type id.
#[must_use]
pub fn tool_kind(component: &Compound) -> Option<String> {
    match component.get(&KEY_ENTITY.into()) {
        Some(Tag::String(kind)) => Some(kind.to_string_lossy()),
        _ => None,
    }
}

/// The captured health, as the cost reads it.
#[must_use]
pub fn tool_health(component: &Compound) -> Option<f32> {
    match component.get(&KEY_HEALTH.into()) {
        Some(Tag::Double(bits)) => Some(f64::from_bits(*bits) as f32),
        Some(Tag::Float(bits)) => Some(f32::from_bits(*bits)),
        Some(Tag::Int(value)) => Some(*value as f32),
        _ => None,
    }
}

/// Whether the duplicator holds anything worth carrying on its item.
pub(crate) fn has_contents(saved: &Compound) -> bool {
    let progress = read_progress(saved).unwrap_or(MAX_PROGRESS);
    read_int(saved, KEY_ENERGY) > 0
        || progress != MAX_PROGRESS
        || read_tank(saved) > 0
        || crate::has_seated_augment(saved)
}

/// The `minecraft:block_entity_data` payload a broken duplicator's item carries, without the tool
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

/// The corrections a placed duplicator needs when its item carried values outside the capacities.
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

/// `/industrialforegoing energy`/`progress` on the duplicator: one int of its saved compound.
pub(crate) fn set_int(host: &mut dyn ServerHost, pos: [i32; 3], key: &str, value: i32) -> bool {
    let Some(entity) = crate::our_entity(host, pos, BLOCK) else {
        return false;
    };
    let mut patch = Compound::new();
    write_int(&mut patch, key, value);
    entity.merge_data(host, patch).unwrap_or(false)
}

/// `/industrialforegoing essence`: write the tank the machine spends (the pin's fluid insert takes
/// only water and lava, the chamber's measured route).
pub(crate) fn set_essence(host: &mut dyn ServerHost, pos: [i32; 3], amount: i32) -> bool {
    let Some(entity) = crate::our_entity(host, pos, BLOCK) else {
        return false;
    };
    let mut patch = Compound::new();
    write_tank(&mut patch, amount);
    entity.merge_data(host, patch).unwrap_or(false)
}

/// `/industrialforegoing info` for the duplicator: the buffer, tank, tool, its cost, the nearby
/// count, the bar, the augments and the game time.
pub(crate) fn readout(host: &mut dyn ServerHost, pos: [i32; 3]) -> String {
    let Some(entity) = crate::our_entity(host, pos, BLOCK) else {
        return format!("{pos:?} is not a mob duplicator.");
    };
    let Some(data) = entity.data(&*host) else {
        return format!("{pos:?} is not a mod block entity.");
    };
    let essence = read_tank(&data);
    let progress = read_int(&data, KEY_PROGRESS);
    let augments = crate::read_augments(&data);
    let (cap_fluid, cap_mb) = capability_tank(host, pos);
    let (tool, needs) = tool_readout(host, pos);
    let nearby = tool
        .as_deref()
        .map(|kind| nearby_count(host, pos, kind))
        .unwrap_or(0);
    let time = entity
        .level(&*host)
        .and_then(|level| level.game_time(host))
        .unwrap_or(0);
    format!(
        "Mob Duplicator at {pos:?}: {}/{} FE, tank {} {}/{TANK_MB} mB, capability {cap_fluid} {cap_mb} mB, tool {}, needs {needs} mB, nearby {nearby}, progress {progress}/{}, augments speed {} efficiency {:.2}, t={time}",
        read_int(&data, KEY_ENERGY),
        CAPACITY_FE,
        if essence > 0 { ESSENCE } else { "empty" },
        essence,
        tool.as_deref().unwrap_or("empty"),
        Duplicator::bar_length(augments),
        augments.speed,
        augments.efficiency,
    )
}

/// The tool's stored type and the essence one fresh mob of it costs, or `("empty", 0)`.
fn tool_readout(host: &mut dyn ServerHost, pos: [i32; 3]) -> (Option<String>, i32) {
    let Some(entity) = crate::our_entity(host, pos, BLOCK) else {
        return (None, 0);
    };
    let Ok(Some(stack)) = entity.item_slot(&*host, 0) else {
        return (None, 0);
    };
    let Some(component) = tool_of(&*host, &stack) else {
        return (None, 0);
    };
    let kind = tool_kind(&component);
    let needs = tool_health(&component)
        .map(crate::duplicator::essence_for_health)
        .unwrap_or(0);
    (kind, needs)
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
    fn the_tank_is_eight_thousand_mb_and_fill_only() {
        let tanks = tanks();
        assert_eq!(tanks.tanks, vec![8_000]);
        assert_eq!(tanks.inputs, vec![0], "Action.FILL: a pipe may fill it");
        assert!(
            tanks.outputs.is_empty(),
            "Action.FILL: no face may drain it"
        );
        let every = vec![0usize];
        assert_eq!(tanks.faces.to_vec(), vec![every; 6]);
    }

    #[test]
    fn the_container_declares_one_input_slot() {
        let container = container();
        assert_eq!(container.slots, 1);
        assert_eq!(container.inputs, vec![0]);
        assert!(container.outputs.is_empty());
        assert_eq!(container.faces.len(), 6);
    }

    #[test]
    fn the_tool_payload_round_trips() {
        let mut nbt = Compound::new();
        nbt.insert(KEY_HEALTH.into(), Tag::Double(10.0_f64.to_bits()));
        let payload = tool_payload("minecraft:cow", nbt);
        assert_eq!(tool_kind(&payload).as_deref(), Some("minecraft:cow"));
        assert_eq!(tool_health(&payload), Some(10.0));
        let mut float = Compound::new();
        float.insert(KEY_HEALTH.into(), Tag::Float(4.0_f32.to_bits()));
        assert_eq!(tool_health(&float), Some(4.0));
        assert_eq!(tool_health(&Compound::new()), None);
    }

    #[test]
    fn the_tank_round_trips_through_the_saved_compound() {
        let mut nbt = Compound::new();
        assert_eq!(read_tank(&nbt), 0);
        write_tank(&mut nbt, 120);
        assert_eq!(read_tank(&nbt), 120);
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
    fn a_tool_alone_does_not_make_the_machine_carry_contents() {
        let mut nbt = Compound::new();
        let mut tool = Compound::new();
        tool.insert("id".into(), Tag::String(TOOL.into()));
        tool.insert("count".into(), Tag::Int(1));
        nbt.insert("Items".into(), Tag::List(vec![Tag::Compound(tool)]));
        assert!(!has_contents(&nbt), "the slot drops beside the block");
    }

    #[test]
    fn a_broken_duplicator_carries_its_buffer_tank_and_bar_but_not_its_slot() {
        let mut nbt = Compound::new();
        write_int(&mut nbt, KEY_ENERGY, 45_000);
        write_int(&mut nbt, KEY_PROGRESS, 3);
        write_tank(&mut nbt, 120);
        let mut tool = Compound::new();
        tool.insert("id".into(), Tag::String(TOOL.into()));
        tool.insert("count".into(), Tag::Int(1));
        nbt.insert("Items".into(), Tag::List(vec![Tag::Compound(tool)]));
        assert!(has_contents(&nbt));
        let patch = carried_patch(&nbt);
        let Some(Tag::Compound(payload)) = patch.get(&"minecraft:block_entity_data".into()) else {
            panic!("no block entity data in the patch");
        };
        assert_eq!(
            payload.get(&"id".into()),
            Some(&Tag::String(ENTITY_ID.into()))
        );
        assert_eq!(read_tank(payload), 120);
        assert_eq!(read_int(payload, KEY_ENERGY), 45_000);
        assert!(
            payload.get(&"Items".into()).is_none(),
            "the tool drops beside the block, never twice"
        );
    }

    #[test]
    fn a_placed_duplicator_clamps_what_its_item_carried() {
        let mut over = Compound::new();
        write_int(&mut over, KEY_ENERGY, 90_000);
        write_int(&mut over, KEY_PROGRESS, 4_000);
        let mut entry = Compound::new();
        entry.insert("Fluid".into(), Tag::String(ESSENCE.into()));
        entry.insert("Amount".into(), Tag::Int(999_999));
        over.insert("fluids".into(), Tag::List(vec![Tag::Compound(entry)]));
        let patch = clamp_patch(&over).expect("out-of-range values are corrected");
        assert_eq!(read_int(&patch, KEY_ENERGY), CAPACITY_FE);
        assert_eq!(read_int(&patch, KEY_PROGRESS), MAX_PROGRESS);
        assert_eq!(read_tank(&patch), TANK_MB);
        assert!(
            clamp_patch(&Compound::new()).is_none(),
            "a fresh duplicator is left alone"
        );
    }
}
