//! The game side of the **sewage chain**: the `sewer` and the `sewage_composter`.
//!
//! Upstream's `SewerTile` is an `IndustrialAreaWorkingTile(RangeType.TOP)`: its one saved tank
//! (`sewage`, 8,000 mB, `Action.DRAIN`) is filled from the `Animal`s in the **single block above**,
//! 50 mB each, at 10 FE an animal (charged for one extra operation by the jar's `++amount`). The
//! jar's second tank (`essence`, fed by XP orbs and players) is a separate family and is recorded
//! as `not ported` in `notes.md`.
//!
//! `SewageComposterTile` is an `IndustrialProcessingTile`: a fill-only 8,000 mB sewage tank, a
//! 12-slot output-only fertilizer container, 40 FE a tick and a 100-tick bar; at the top it drains
//! 1,000 mB and inserts one `industrialforegoing:fertilizer`. A mod fluid has no insert path at
//! this pin (`native_fluid_insert` takes water and lava only), so `/industrialforegoing sewage`
//! writes the block entity's own `fluids` compound, as the dissolution chamber's `/fluid` does,
//! and the declared capability is what reads it back.
//!
//! The pure arithmetic is [`crate::sewage`].

use std::sync::OnceLock;

use mod_api::{
    self, BlockEntityPart, BlockEntityRef, BlockEntityTickEvent, BlockEntityTicksEvent, BlockPart,
    BlockState, Compound, Direction, FLUIDS, FluidIdentity, Host, HostTransaction,
    InteractionResult, ItemKey, ItemPart, ItemStack, LevelHost, ModContainer, ModEnergyBuffer,
    ModFluidTanks, OnBlockPlacedByEvent, Registrar, ServerHost, StateForPlacementEvent, Tag,
    UseItemOnEvent,
};

use crate::sewage::{
    CAPACITY_FE, Composter, ComposterStep, FERTILIZER, OUTPUT_SLOTS, SEWAGE, SEWER_MAX_PROGRESS,
    Sewer, TANK_MB,
};

/// `ModuleAgricultureHusbandry.SEWER`.
pub const BLOCK: &str = "industrialforegoing:sewer";
/// `ModuleAgricultureHusbandry.SEWAGE_COMPOSTER`.
pub const COMPOSTER_BLOCK: &str = "industrialforegoing:sewage_composter";
/// The block entity type names, bare so the mod's namespace qualifies them.
pub const ENTITY: &str = "sewer";
pub const COMPOSTER_ENTITY: &str = "sewage_composter";
/// The ids `minecraft:block_entity_data` tags a payload with.
pub const ENTITY_ID: &str = "industrialforegoing:sewer";
pub const COMPOSTER_ENTITY_ID: &str = "industrialforegoing:sewage_composter";
/// Titanium's `RotationType.FOUR_WAY` property, carried by both blocks' state files.
pub const SUBFACING: &str = "subfacing";
/// Saved keys: the buffer and the bar (the tank is the pin's `fluids` list).
const KEY_ENERGY: &str = "energy";
const KEY_PROGRESS: &str = "progress";
/// Every face reaches the one tank for reading/draining (sewer) or filling (composter).
const FACE: Direction = Direction::Up;

/// The vanilla `Animal` subclasses the pin can name by type id. NeoForge's
/// `getEntitiesOfClass(Animal.class, …)` needs a class test the SDK does not expose; the ids cover
/// every vanilla animal, and a mod's own animal can be added here (recorded as a deviation).
const ANIMALS: &[&str] = &[
    "minecraft:axolotl",
    "minecraft:bee",
    "minecraft:cat",
    "minecraft:camel",
    "minecraft:chicken",
    "minecraft:cow",
    "minecraft:dolphin",
    "minecraft:donkey",
    "minecraft:fox",
    "minecraft:frog",
    "minecraft:goat",
    "minecraft:horse",
    "minecraft:llama",
    "minecraft:mooshroom",
    "minecraft:mule",
    "minecraft:ocelot",
    "minecraft:panda",
    "minecraft:pig",
    "minecraft:polar_bear",
    "minecraft:rabbit",
    "minecraft:sheep",
    "minecraft:sniffer",
    "minecraft:tadpole",
    "minecraft:turtle",
    "minecraft:wolf",
];

static KIND: OnceLock<mod_api::BlockEntityTypeKey> = OnceLock::new();
static COMPOSTER_KIND: OnceLock<mod_api::BlockEntityTypeKey> = OnceLock::new();
static ITEM: OnceLock<ItemKey> = OnceLock::new();
static COMPOSTER_ITEM: OnceLock<ItemKey> = OnceLock::new();

// ---------------------------------------------------------------------------
// The declarations
// ---------------------------------------------------------------------------

/// Declare both machines: their block entity types, buffers, tanks, the composter's container, the
/// Parts that tick and rotate them, and the Parts on their items.
pub(crate) fn declare(r: &mut dyn Registrar) {
    let sewer = r.block_key(BLOCK);
    let sewer_item = r.item_key(BLOCK);
    let _ = ITEM.set(sewer_item);
    r.item_part(sewer_item, SewerItem);
    let kind = r.block_entity_type(ENTITY).blocks(&[ENTITY]).build();
    let _ = KIND.set(kind);
    r.block_entity_part(&kind, SewerPart)
        .expect("the sewer's Part attaches to the type this mod just declared");
    r.block_energy(
        &sewer,
        ModEnergyBuffer {
            capacity: CAPACITY_FE,
            max_insert: CAPACITY_FE,
            max_extract: CAPACITY_FE,
            insert_faces: [true; 6],
            extract_faces: [true; 6],
        },
    );
    r.block_fluids(&sewer, sewer_tanks());

    let composter = r.block_key(COMPOSTER_BLOCK);
    let composter_item = r.item_key(COMPOSTER_BLOCK);
    let _ = COMPOSTER_ITEM.set(composter_item);
    r.item_part(composter_item, ComposterItem);
    let kind = r
        .block_entity_type(COMPOSTER_ENTITY)
        .blocks(&[COMPOSTER_ENTITY])
        .build();
    let _ = COMPOSTER_KIND.set(kind);
    r.block_entity_part(&kind, ComposterPart)
        .expect("the composter's Part attaches to the type this mod just declared");
    r.block_energy(
        &composter,
        ModEnergyBuffer {
            capacity: CAPACITY_FE,
            max_insert: CAPACITY_FE,
            max_extract: CAPACITY_FE,
            insert_faces: [true; 6],
            extract_faces: [true; 6],
        },
    );
    r.block_fluids(&composter, composter_tanks());
    r.block_container(&composter, container());
}

/// `SidedFluidTankComponent("sewage", 8000, …, 0)` with `Action.DRAIN`: every face reads and drains
/// it, no face may fill it — the machine fills it itself.
#[must_use]
pub fn sewer_tanks() -> ModFluidTanks {
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

/// `SidedFluidTankComponent("sewage", 8000, …, 0)` with `Action.FILL` and a sewage validator: the
/// tank can only be filled, by this machine's own operation or a `/data merge` write.
#[must_use]
pub fn composter_tanks() -> ModFluidTanks {
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
    tanks
}

/// `SidedInventoryComponent("fertilizer", 90, 22, 12, 1)` with an accept-nothing input filter:
/// twelve output-only slots.
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
// The Parts
// ---------------------------------------------------------------------------

/// The sewer: placement rotation, the augment seat and its tick.
pub(crate) struct SewerPart;

#[mod_api::horizon_part]
impl BlockPart for SewerPart {
    fn state_for_placement(
        &self,
        level: &dyn LevelHost,
        event: &mut StateForPlacementEvent<'_>,
    ) -> Option<Option<BlockState>> {
        let faced = level.state_with_property(
            event.state,
            SUBFACING,
            crate::facing_for_yaw(event.context.yaw),
        )?;
        Some(Some(faced))
    }

    fn use_item_on(
        &self,
        host: &mut dyn ServerHost,
        event: &mut UseItemOnEvent<'_>,
    ) -> Option<InteractionResult> {
        crate::seat_from_click(host, event)
    }
}

#[mod_api::horizon_part]
impl BlockEntityPart for SewerPart {
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    /// One server tick: fall the bar; at zero count the animals in the block above and let the pure
    /// rule decide how much sewage to add and how much FE to spend. The scan is made only on the
    /// attempt (the review's powered-attempt-only rule), not on every falling tick.
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let entity = &mut *event.entity;
        let mut sewer = Sewer::from_saved(
            read_int(&entity.nbt, KEY_ENERGY),
            read_progress(&entity.nbt),
            read_tank_mb(&entity.nbt),
        );
        let augments = crate::read_augments(&entity.nbt);
        if sewer.progress > augments.speed.max(1) {
            sewer.tick(augments, 0);
        } else {
            let animals = animals_above(host, pos);
            // `Sewer::tick` already calls `fill` for the clamped amount (as `fillForced` does); the
            // step only reports what it added, so the glue must not add it a second time.
            let _ = sewer.tick(augments, animals);
        }
        write_int(&mut entity.nbt, KEY_ENERGY, sewer.energy);
        write_int(&mut entity.nbt, KEY_PROGRESS, sewer.progress);
        if sewer.sewage_mb != read_tank_mb(&entity.nbt) {
            write_tank(&mut entity.nbt, sewer.sewage_mb);
        }
    }
}

/// The composter: placement rotation, the augment seat and its rising bar.
pub(crate) struct ComposterPart;

#[mod_api::horizon_part]
impl BlockPart for ComposterPart {
    fn state_for_placement(
        &self,
        level: &dyn LevelHost,
        event: &mut StateForPlacementEvent<'_>,
    ) -> Option<Option<BlockState>> {
        let faced = level.state_with_property(
            event.state,
            SUBFACING,
            crate::facing_for_yaw(event.context.yaw),
        )?;
        Some(Some(faced))
    }

    fn use_item_on(
        &self,
        host: &mut dyn ServerHost,
        event: &mut UseItemOnEvent<'_>,
    ) -> Option<InteractionResult> {
        crate::seat_from_click(host, event)
    }
}

#[mod_api::horizon_part]
impl BlockEntityPart for ComposterPart {
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    /// One server tick: a bar that can work rises and spends 40 FE; at the top it drains 1,000 mB
    /// and inserts one fertilizer into the 12-slot output container.
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let entity = &mut *event.entity;
        if entity.items.len() < OUTPUT_SLOTS {
            entity.items.resize(OUTPUT_SLOTS, None);
        }
        let mut composter = Composter::from_saved(
            read_int(&entity.nbt, KEY_ENERGY),
            read_int(&entity.nbt, KEY_PROGRESS),
            read_tank_mb(&entity.nbt),
        );
        let accepts = entity.items.iter().any(Option::is_none);
        let step = composter.tick(crate::read_augments(&entity.nbt), accepts);
        if matches!(step, ComposterStep::Done { .. }) {
            let Some(item) = item_key(&*host, FERTILIZER) else {
                return;
            };
            for _ in 0..crate::sewage::FERTILIZER_COUNT {
                let Some(stack) = ItemStack::new(item, 1) else {
                    break;
                };
                crate::dissolution::put(&*host, &mut entity.items, stack);
            }
        }
        write_int(&mut entity.nbt, KEY_ENERGY, composter.energy);
        write_int(&mut entity.nbt, KEY_PROGRESS, composter.progress);
        if composter.sewage_mb != read_tank_mb(&entity.nbt) {
            write_tank(&mut entity.nbt, composter.sewage_mb);
        }
    }
}

/// The working area of `RangeType.TOP` at range 0: the single block directly above the machine.
#[must_use]
pub fn working_area(pos: [i32; 3]) -> ([f64; 3], [f64; 3]) {
    let [x, y, z] = pos;
    (
        [f64::from(x), f64::from(y + 1), f64::from(z)],
        [f64::from(x) + 1.0, f64::from(y) + 2.0, f64::from(z) + 1.0],
    )
}

/// Every `Animal` in that block. The pin has no class test, so the vanilla animal ids are named;
/// a `NoAI` cow parked in the column is counted exactly like a free-moving one.
fn animals_above(host: &mut dyn ServerHost, pos: [i32; 3]) -> i32 {
    let Some(level) = host.level(host.dimension()) else {
        return 0;
    };
    let (min, max) = working_area(pos);
    let Some(entities) = level.entities_in(&*host, min, max) else {
        return 0;
    };
    let mut count = 0;
    for entity in entities {
        let Some(living) = host.living(entity) else {
            continue;
        };
        if !living.alive(&*host).unwrap_or(false) {
            continue;
        }
        if entity
            .kind(&*host)
            .is_some_and(|kind| ANIMALS.contains(&kind.id(&*host).as_str()))
        {
            count += 1;
        }
    }
    count
}

/// An item id resolved through the game's registry.
fn item_key(host: &dyn Host, id: &str) -> Option<ItemKey> {
    host.item_key(&mod_api::Identifier::parse(id).ok()?)
}

// ---------------------------------------------------------------------------
// The machines' items
// ---------------------------------------------------------------------------

/// A broken sewer or composter's item carries its buffer, tank and bar (no container: the engine
/// spills the composter's twelve slots itself), and placing it clamps them back to the capacities.
pub(crate) struct SewerItem;
pub(crate) struct ComposterItem;

#[mod_api::horizon_part]
impl ItemPart for SewerItem {
    fn on_block_placed_by(&self, host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
        placed(host, event, BLOCK);
    }
}

#[mod_api::horizon_part]
impl ItemPart for ComposterItem {
    fn on_block_placed_by(&self, host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
        placed(host, event, COMPOSTER_BLOCK);
    }
}

fn placed(host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>, block: &str) {
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
    if crate::current_block(host, event.pos).as_deref() != Some(block) {
        return;
    }
    if let Some(patch) = clamp_patch(&saved) {
        let _ = entity.merge_data(host, patch);
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

/// A bar the save never wrote is full for the sewer, empty for the composter.
fn read_progress(nbt: &Compound) -> Option<i32> {
    match nbt.get(&KEY_PROGRESS.into()) {
        Some(Tag::Int(value)) => Some(*value),
        _ => None,
    }
}

/// The pin's one-tank shape: one `{Fluid, Amount}` entry in the `fluids` list.
fn read_tank_mb(nbt: &Compound) -> i32 {
    let Some(Tag::List(entries)) = nbt.get(&"fluids".into()) else {
        return 0;
    };
    for entry in entries {
        let Tag::Compound(entry) = entry else {
            continue;
        };
        let fluid = match entry.get(&"Fluid".into()) {
            Some(Tag::String(fluid)) => fluid.to_string_lossy(),
            _ => continue,
        };
        if fluid == "minecraft:empty" {
            return 0;
        }
        return match entry.get(&"Amount".into()) {
            Some(Tag::Int(amount)) => (*amount).clamp(0, TANK_MB),
            _ => 0,
        };
    }
    0
}

fn write_tank(nbt: &mut Compound, mb: i32) {
    let mb = mb.clamp(0, TANK_MB);
    let mut entry = Compound::new();
    entry.insert(
        "Fluid".into(),
        Tag::String(if mb == 0 { "minecraft:empty" } else { SEWAGE }.into()),
    );
    entry.insert("Amount".into(), Tag::Int(mb));
    nbt.insert("fluids".into(), Tag::List(vec![Tag::Compound(entry)]));
}

/// A machine with FE, an unfinished bar, sewage or a seated augment has something to carry.
pub(crate) fn has_contents(block: &str, saved: &Compound) -> bool {
    let full = if block == BLOCK {
        SEWER_MAX_PROGRESS
    } else {
        0
    };
    read_int(saved, KEY_ENERGY) > 0
        || read_progress(saved).is_some_and(|progress| progress != full)
        || read_tank_mb(saved) > 0
        || crate::has_seated_augment(saved)
}

/// The `minecraft:block_entity_data` payload a broken machine's item carries.
#[must_use]
pub(crate) fn carried_patch(block: &str, saved: &Compound) -> Compound {
    let mut payload = saved.clone();
    payload.remove(&"Items".into());
    payload.insert(
        "id".into(),
        Tag::String(
            if block == BLOCK {
                ENTITY_ID
            } else {
                COMPOSTER_ENTITY_ID
            }
            .into(),
        ),
    );
    let mut patch = Compound::new();
    patch.insert("minecraft:block_entity_data".into(), Tag::Compound(payload));
    patch
}

/// Corrections for a placed machine whose item carried values outside the capacities.
#[must_use]
pub(crate) fn clamp_patch(saved: &Compound) -> Option<Compound> {
    let mut patch = Compound::new();
    let mut changed = false;
    let energy = read_int(saved, KEY_ENERGY);
    if !(0..=CAPACITY_FE).contains(&energy) {
        write_int(&mut patch, KEY_ENERGY, energy.clamp(0, CAPACITY_FE));
        changed = true;
    }
    if let Some(progress) = read_progress(saved)
        && !(0..=SEWER_MAX_PROGRESS).contains(&progress)
    {
        write_int(
            &mut patch,
            KEY_PROGRESS,
            progress.clamp(0, SEWER_MAX_PROGRESS),
        );
        changed = true;
    }
    if matches!(
        saved.get(&"fluids".into()),
        Some(Tag::List(entries)) if entries.iter().any(|entry| matches!(
            entry,
            Tag::Compound(entry) if matches!(entry.get(&"Amount".into()), Some(Tag::Int(mb)) if !(0..=TANK_MB).contains(mb))
        ))
    ) {
        write_tank(&mut patch, read_tank_mb(saved));
        changed = true;
    }
    changed.then_some(patch)
}

// ---------------------------------------------------------------------------
// The commands
// ---------------------------------------------------------------------------

/// `/industrialforegoing energy|progress` on either machine: one int of its saved compound.
pub(crate) fn set_int(
    host: &mut dyn ServerHost,
    pos: [i32; 3],
    block: &str,
    key: &str,
    value: i32,
) -> bool {
    let Some(entity) = crate::our_entity(host, pos, block) else {
        return false;
    };
    let mut patch = Compound::new();
    write_int(&mut patch, key, value);
    entity.merge_data(host, patch).unwrap_or(false)
}

/// `/industrialforegoing sewage`: write the composter's own tank as `/data merge block` does
/// (a mod fluid has no insert path at this pin, the dissolution chamber's measured route).
pub(crate) fn set_sewage(host: &mut dyn ServerHost, pos: [i32; 3], amount: i32) -> bool {
    let Some(entity) = crate::our_entity(host, pos, COMPOSTER_BLOCK) else {
        return false;
    };
    let mut patch = Compound::new();
    write_tank(&mut patch, amount);
    entity.merge_data(host, patch).unwrap_or(false)
}

/// `/industrialforegoing drain`: take sewage out through the declared fluid capability.
pub(crate) fn drain(host: &mut dyn ServerHost, pos: [i32; 3], block: &str, amount: i32) {
    let name = if block == BLOCK {
        "Sewer"
    } else {
        "Sewage Composter"
    };
    let want = u32::try_from(amount.max(0)).unwrap_or(u32::MAX);
    let answer = host
        .transaction::<String, ()>(|host, tx| {
            let Some(level) = host.level(host.dimension()) else {
                return Ok(format!("{name} at {pos:?}: no drainable fluid."));
            };
            let Some(tanks) = level.capability(host, &FLUIDS, pos, Some(FACE)) else {
                return Ok(format!("{name} at {pos:?}: no drainable fluid."));
            };
            let Some(resource) = tanks.resource(host, 0) else {
                return Ok(format!("{name} at {pos:?}: no drainable fluid."));
            };
            let taken = tanks.extract(host, tx, 0, &resource, want);
            if taken == 0 {
                return Ok(format!("{name} at {pos:?}: no drainable fluid."));
            }
            Ok(format!(
                "{name} at {pos:?}: drained {taken} mB of {SEWAGE}."
            ))
        })
        .unwrap_or_else(|()| format!("{pos:?} is not a sewage machine."));
    crate::say(host, &answer);
}

/// `/industrialforegoing info` for either machine. The sewer prints the tank and the capability's
/// view of it, the pen's animal count and the bar; the composter adds its output slots.
pub(crate) fn readout(host: &mut dyn ServerHost, pos: [i32; 3], block: &str) -> String {
    let Some(entity) = crate::our_entity(host, pos, block) else {
        return format!("{pos:?} is not a sewage machine.");
    };
    let Some(data) = entity.data(&*host) else {
        return format!("{pos:?} is not a mod block entity.");
    };
    let tank_mb = read_tank_mb(&data);
    let (cap_fluid, cap_mb) = capability_tank(host, pos);
    let augments = crate::read_augments(&data);
    let level = entity.level(&*host);
    let facing = level
        .as_ref()
        .and_then(|level| level.block_property(host, pos, SUBFACING))
        .unwrap_or_else(|| "unknown".to_owned());
    let time = level
        .as_ref()
        .and_then(|level| level.game_time(host))
        .unwrap_or(0);
    if block == BLOCK {
        format!(
            "Sewer at {pos:?}: {}/{} FE, tank {SEWAGE} {tank_mb}/{TANK_MB} mB, capability {cap_fluid} {cap_mb} mB, animals {}, progress {}/{}, facing {facing}, augments speed {} efficiency {:.2}, t={time}",
            read_int(&data, KEY_ENERGY),
            CAPACITY_FE,
            animals_above(host, pos),
            read_int(&data, KEY_PROGRESS),
            Sewer::bar_length(augments),
            augments.speed,
            augments.efficiency,
        )
    } else {
        format!(
            "Sewage Composter at {pos:?}: {}/{} FE, tank {SEWAGE} {tank_mb}/{TANK_MB} mB, capability {cap_fluid} {cap_mb} mB, output {}, progress {}/{}, facing {facing}, augments speed {} efficiency {:.2}, t={time}",
            read_int(&data, KEY_ENERGY),
            CAPACITY_FE,
            output_contents(host, entity),
            read_int(&data, KEY_PROGRESS),
            Composter::bar_length(augments),
            augments.speed,
            augments.efficiency,
        )
    }
}

/// The composter's output slots as `"1x industrialforegoing:fertilizer"`, or `empty`.
fn output_contents(host: &mut dyn ServerHost, entity: BlockEntityRef) -> String {
    let mut parts = Vec::new();
    for slot in 0..OUTPUT_SLOTS as u32 {
        if let Ok(Some(stack)) = entity.item_slot(&*host, slot) {
            parts.push(format!("{}x {}", stack.count(), stack.item().id(&*host)));
        }
    }
    if parts.is_empty() {
        "empty".to_owned()
    } else {
        parts.join(", ")
    }
}

/// The tank as the declared fluid capability reports it (the oracle the in-game assertion reads).
fn capability_tank(host: &mut dyn ServerHost, pos: [i32; 3]) -> (String, i32) {
    let none = ("none".to_owned(), 0);
    let Some(level) = host.level(host.dimension()) else {
        return none;
    };
    let Some(tanks) = level.capability(host, &FLUIDS, pos, Some(FACE)) else {
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
    (
        name,
        i32::try_from(tanks.amount(host, 0)).unwrap_or(i32::MAX),
    )
}

/// The item `block_drops` may write to.
pub(crate) fn item(block: &str) -> Option<ItemKey> {
    if block == BLOCK {
        ITEM.get().copied()
    } else {
        COMPOSTER_ITEM.get().copied()
    }
}

/// Which of this module's block entity types this is, for the augment seat.
pub(crate) fn kind_of(entity: BlockEntityRef, host: &dyn Host) -> bool {
    entity
        .kind(host)
        .is_some_and(|kind| KIND.get() == Some(&kind) || COMPOSTER_KIND.get() == Some(&kind))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sewer_tank_is_eight_thousand_mb_and_drain_only() {
        let tanks = sewer_tanks();
        assert_eq!(tanks.tanks, vec![8_000]);
        assert!(tanks.inputs.is_empty(), "Action.DRAIN: no fill face");
        assert_eq!(tanks.outputs, vec![0]);
        assert_eq!(tanks.faces.len(), 6);
    }

    #[test]
    fn the_composter_tank_is_eight_thousand_mb_and_fill_only() {
        let tanks = composter_tanks();
        assert_eq!(tanks.tanks, vec![8_000]);
        assert_eq!(tanks.inputs, vec![0], "Action.FILL: every face fills it");
        assert!(tanks.outputs.is_empty(), "nothing drains through a face");
    }

    #[test]
    fn the_composter_container_has_twelve_output_only_slots() {
        let container = container();
        assert_eq!(container.slots, 12);
        assert!(
            container.inputs.is_empty(),
            "setInputFilter(stack -> false)"
        );
        assert_eq!(container.outputs, (0..12).collect::<Vec<_>>());
        assert_eq!(container.faces.len(), 6);
    }

    #[test]
    fn the_working_area_is_the_single_block_above() {
        let (min, max) = working_area([200, 101, 0]);
        assert_eq!(min, [200.0, 102.0, 0.0]);
        assert_eq!(max, [201.0, 103.0, 1.0]);
    }

    #[test]
    fn the_tank_round_trips_and_empties_through_the_saved_compound() {
        let mut nbt = Compound::new();
        assert_eq!(read_tank_mb(&nbt), 0);
        write_tank(&mut nbt, 1_000);
        assert_eq!(read_tank_mb(&nbt), 1_000);
        write_tank(&mut nbt, 0);
        assert_eq!(read_tank_mb(&nbt), 0);
    }

    #[test]
    fn a_broken_machine_carries_its_numbers_but_not_the_composter_output() {
        let mut nbt = Compound::new();
        write_int(&mut nbt, KEY_ENERGY, 9_980);
        write_int(&mut nbt, KEY_PROGRESS, 1);
        write_tank(&mut nbt, 50);
        nbt.insert("Items".into(), Tag::List(vec![]));
        assert!(has_contents(BLOCK, &nbt));
        let patch = carried_patch(COMPOSTER_BLOCK, &nbt);
        let Some(Tag::Compound(payload)) = patch.get(&"minecraft:block_entity_data".into()) else {
            panic!("no block entity data in the patch");
        };
        assert_eq!(
            payload.get(&"id".into()),
            Some(&Tag::String(COMPOSTER_ENTITY_ID.into()))
        );
        assert_eq!(read_tank_mb(payload), 50);
        assert!(payload.get(&"Items".into()).is_none());
    }

    #[test]
    fn a_placed_machine_clamps_what_its_item_carried() {
        let mut over = Compound::new();
        write_int(&mut over, KEY_ENERGY, 90_000);
        write_int(&mut over, KEY_PROGRESS, 4_000);
        let mut entry = Compound::new();
        entry.insert("Fluid".into(), Tag::String(SEWAGE.into()));
        entry.insert("Amount".into(), Tag::Int(999_999));
        over.insert("fluids".into(), Tag::List(vec![Tag::Compound(entry)]));
        let patch = clamp_patch(&over).expect("out-of-range values are corrected");
        assert_eq!(read_int(&patch, KEY_ENERGY), CAPACITY_FE);
        assert_eq!(read_int(&patch, KEY_PROGRESS), SEWER_MAX_PROGRESS);
        assert_eq!(read_tank_mb(&patch), TANK_MB);
        assert!(clamp_patch(&Compound::new()).is_none());
    }

    #[test]
    fn the_feed_and_tick_commands_are_in_the_artefact_s_units() {
        assert_eq!(crate::sewage::COMPOST_SEWAGE_MB, 1_000);
        assert_eq!(crate::sewage::SEWER_POWER_PER_OPERATION, 10);
        assert_eq!(crate::sewage::COMPOSTER_MAX_PROGRESS, 100);
    }
}
