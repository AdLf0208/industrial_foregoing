//! The game side of the **Plant Fertilizer** (`PlantFertilizerTile` at `ce2852db`).
//!
//! An `IndustrialAreaWorkingTile(RangeType.BEHIND)` at range 0: its working area is the single
//! block behind the machine's `subfacing`. On a finished bar, with a fertilizer in the 18-slot
//! input and 1,000 FE, it bonemeals that block through the pin's `LevelRef::bone_meal`, which runs
//! the same `isValidBonemealTarget`/`isBonemealSuccess`/`performBonemeal` trio the jar calls. The
//! pure arithmetic lives in [`crate::plantfert`]; `src/runtime.rs` is untouched.

use std::sync::OnceLock;

use mod_api::{
    self, BlockEntityPart, BlockEntityRef, BlockEntityTickEvent, BlockEntityTicksEvent, BlockPart,
    BlockState, Compound, Host, HostBlockStates, InteractionResult, ItemKey, ItemPart, ItemStack,
    LevelHost, ModContainer, ModEnergyBuffer, OnBlockPlacedByEvent, Registrar, ServerHost,
    StateForPlacementEvent, Tag, UseItemOnEvent,
};

use crate::plantfert::{
    Attempt, CAPACITY_FE, FERTILIZER, MAX_PROGRESS, PlantFertilizer, POWER_PER_OPERATION, SLOTS,
    Step,
};

/// `ModuleAgricultureHusbandry.PLANT_FERTILIZER`.
pub const BLOCK: &str = "industrialforegoing:plant_fertilizer";
/// The bare block entity type name; the mod's namespace qualifies it.
pub const ENTITY: &str = "plant_fertilizer";
/// The `minecraft:block_entity_data` id.
pub const ENTITY_ID: &str = "industrialforegoing:plant_fertilizer";
/// Titanium's `RotationType.FOUR_WAY` property, carried by the block's state file.
pub const SUBFACING: &str = "subfacing";
/// Saved keys: the buffer and the falling bar. The input is the block entity's container.
const KEY_ENERGY: &str = "energy";
const KEY_PROGRESS: &str = "progress";
/// Vanilla's `age` property on a bonemealable crop.
const AGE: &str = "age";

static KIND: OnceLock<mod_api::BlockEntityTypeKey> = OnceLock::new();
static ITEM: OnceLock<ItemKey> = OnceLock::new();

/// Declare the block entity type, its 10,000 FE buffer, its 18-slot input container and its Parts.
pub(crate) fn declare(r: &mut dyn Registrar) {
    let block = r.block_key(BLOCK);
    let item = r.item_key(BLOCK);
    let _ = ITEM.set(item);
    r.item_part(item, PlantItem);
    let kind = r.block_entity_type(ENTITY).blocks(&[ENTITY]).build();
    let _ = KIND.set(kind);
    r.block_entity_part(&kind, PlantPart)
        .expect("the plant fertilizer's Part attaches to the type this mod just declared");
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
    r.block_container(&block, container());
}

/// `SidedInventoryComponent("fertilizer", 50, 22, 3 * 6, 0)`: 18 input slots and no output. The
/// pin has no per-slot item predicate, so the accept filter (`c:fertilizers`) is enforced by the
/// tick, which only ever takes `industrialforegoing:fertilizer` out of these slots.
#[must_use]
pub fn container() -> ModContainer {
    let every: Vec<usize> = (0..SLOTS).collect();
    let mut container = ModContainer {
        slots: SLOTS,
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
    container.inputs = (0..SLOTS).collect();
    container
}

/// The placement rotation and the augment seat, then the block entity's tick.
pub(crate) struct PlantPart;

#[mod_api::horizon_part]
impl BlockPart for PlantPart {
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
impl BlockEntityPart for PlantPart {
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    /// One server tick: fall the bar; at zero take the first fertilizer, check the 1,000 FE gate,
    /// bonemeal the block behind and let the pure rule spend the FE and choose the cooldown.
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let entity = &mut *event.entity;
        if entity.items.len() < SLOTS {
            entity.items.resize(SLOTS, None);
        }
        let mut plant = PlantFertilizer::from_saved(
            read_int(&entity.nbt, KEY_ENERGY),
            read_progress(&entity.nbt),
        );
        let augments = crate::read_augments(&entity.nbt);
        let (attempt, slot) = if plant.progress > augments.speed.max(1) {
            // The scan and the bonemeal are made only on the attempt, not on every falling tick.
            (Attempt::NoInput, None)
        } else {
            let slot = first_fertilizer(host, &entity.items);
            let attempt = match slot {
                None => Attempt::NoInput,
                Some(_) if plant.energy < POWER_PER_OPERATION => Attempt::NoEnergy,
                Some(_) => match pointed(host, pos).and_then(|at| grow(host, at)) {
                    None => Attempt::NotATarget,
                    Some(still_valid) => Attempt::Grew { still_valid },
                },
            };
            (attempt, slot)
        };
        let step = plant.tick(augments, attempt);
        if let Step::Fertilized { consumed, .. } = step
            && let Some(slot) = slot
            && let Some(stack) = entity.items.get_mut(slot).and_then(Option::take)
        {
            // `ItemStack.shrink` returns None at zero, which leaves the slot empty as upstream's
            // `stack.shrink(1)` does.
            entity.items[slot] = stack.shrink(u32::try_from(consumed).unwrap_or(u32::MAX));
        }
        write_int(&mut entity.nbt, KEY_ENERGY, plant.energy);
        write_int(&mut entity.nbt, KEY_PROGRESS, plant.progress);
    }
}

/// The first `industrialforegoing:fertilizer` in the 18 slots (`ItemHandlerUtil.getFirstItem`).
fn first_fertilizer(host: &dyn Host, items: &[Option<ItemStack>]) -> Option<usize> {
    items.iter().position(|stack| {
        stack.as_ref().is_some_and(|stack| stack.item().id(host).as_str() == FERTILIZER)
    })
}

/// The offset of the single block behind a machine facing `facing` (`Direction.getOpposite`, as
/// the animal rancher's own glue models it).
#[must_use]
pub fn behind(facing: &str) -> Option<[i32; 3]> {
    match facing {
        "north" => Some([0, 0, 1]),
        "south" => Some([0, 0, -1]),
        "east" => Some([-1, 0, 0]),
        "west" => Some([1, 0, 0]),
        _ => None,
    }
}

/// The `RangeType.BEHIND` cell at range 0, read from the placed block's own `subfacing`.
fn pointed(host: &mut dyn ServerHost, pos: [i32; 3]) -> Option<[i32; 3]> {
    let level = host.level(host.dimension())?;
    let facing = level.block_property(&*host, pos, SUBFACING)?;
    let [dx, dy, dz] = behind(&facing)?;
    Some([pos[0] + dx, pos[1] + dy, pos[2] + dz])
}

/// Apply the pin's bone meal to the pointed block. `None` means it was not a valid target, or it
/// did not grow. `Some(still_valid)` reports whether it remains a bonemealable target afterwards —
/// vanilla's `isValidBonemealTarget` after `performBonemeal`, which chooses `0.25` over `0.5`.
fn grow(host: &mut dyn ServerHost, pos: [i32; 3]) -> Option<bool> {
    let level = host.level(host.dimension())?;
    if !level.is_loaded(&*host, pos) || !level.bone_meal(host, pos) {
        return None;
    }
    // A crop keeps an `age` property and stays a target below its maximum; a grown sapling becomes
    // a tree and has none. Both answer "finished" here.
    let still_valid = level
        .block_property(&*host, pos, AGE)
        .and_then(|age| age.parse::<i32>().ok())
        .zip(
            level
                .block(&*host, pos)
                .and_then(|block| host.block_states().definition(block))
                .and_then(|properties| {
                    properties
                        .into_iter()
                        .find(|property| property.name == AGE)
                })
                .and_then(|property| property.values.last().cloned())
                .and_then(|last| last.parse::<i32>().ok()),
        )
        .is_some_and(|(age, max)| age < max);
    Some(still_valid)
}

/// The machine's item: a broken one carries its buffer and bar back.
pub(crate) struct PlantItem;

#[mod_api::horizon_part]
impl ItemPart for PlantItem {
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
        let mut patch = Compound::new();
        let energy = read_int(&saved, KEY_ENERGY);
        if !(0..=CAPACITY_FE).contains(&energy) {
            write_int(&mut patch, KEY_ENERGY, energy.clamp(0, CAPACITY_FE));
        }
        if let Some(progress) = read_progress(&saved)
            && !(0..=MAX_PROGRESS).contains(&progress)
        {
            write_int(&mut patch, KEY_PROGRESS, progress.clamp(0, MAX_PROGRESS));
        }
        if !patch.is_empty() {
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

/// A bar the save never wrote starts full (`ProgressBarComponent(50, 50)`).
fn read_progress(nbt: &Compound) -> Option<i32> {
    match nbt.get(&KEY_PROGRESS.into()) {
        Some(Tag::Int(value)) => Some(*value),
        _ => None,
    }
}

/// FE, an unfinished bar or a fertilizer in a slot all give the broken machine something to carry.
pub(crate) fn has_contents(saved: &Compound) -> bool {
    read_int(saved, KEY_ENERGY) > 0
        || read_progress(saved).is_some_and(|progress| progress != MAX_PROGRESS)
        || saved
            .get(&"Items".into())
            .is_some_and(|items| matches!(items, Tag::List(entries) if !entries.is_empty()))
        || crate::has_seated_augment(saved)
}

/// The `minecraft:block_entity_data` payload a broken machine's item carries.
#[must_use]
pub(crate) fn carried_patch(saved: &Compound) -> Compound {
    let mut payload = saved.clone();
    payload.insert("id".into(), Tag::String(ENTITY_ID.into()));
    let mut patch = Compound::new();
    patch.insert("minecraft:block_entity_data".into(), Tag::Compound(payload));
    patch
}

/// Which of this module's block entity types this is, for the augment seat.
pub(crate) fn kind_of(entity: BlockEntityRef, host: &dyn Host) -> bool {
    entity.kind(host).is_some_and(|kind| KIND.get() == Some(&kind))
}

/// The item `block_drops` may write to.
pub(crate) fn item() -> Option<ItemKey> {
    ITEM.get().copied()
}

// ---------------------------------------------------------------------------
// The harness commands
// ---------------------------------------------------------------------------

/// `/industrialforegoing energy|progress` on this machine: one int of its saved compound.
pub(crate) fn set_int(
    host: &mut dyn ServerHost,
    pos: [i32; 3],
    key: &str,
    value: i32,
) -> bool {
    let Some(entity) = crate::our_entity(host, pos, BLOCK) else {
        return false;
    };
    let mut patch = Compound::new();
    write_int(&mut patch, key, value);
    entity.merge_data(host, patch).unwrap_or(false)
}

/// `/industrialforegoing info` for this machine. The crop age is read from the pointed block, so
/// the in-game leg can assert the bonemeal's exact step without trusting the block entity alone.
pub(crate) fn readout(host: &mut dyn ServerHost, pos: [i32; 3]) -> String {
    let Some(entity) = crate::our_entity(host, pos, BLOCK) else {
        return format!("{pos:?} is not a plant fertilizer.");
    };
    let Some(data) = entity.data(&*host) else {
        return format!("{pos:?} is not a mod block entity.");
    };
    let energy = read_int(&data, KEY_ENERGY);
    let progress = read_int(&data, KEY_PROGRESS);
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
    let at = pointed(host, pos);
    let mut input = Vec::new();
    for slot in 0..SLOTS as u32 {
        if let Ok(Some(stack)) = entity.item_slot(&*host, slot) {
            input.push(format!("{}x {}", stack.count(), stack.item().id(&*host)));
        }
    }
    let input = if input.is_empty() {
        "empty".to_owned()
    } else {
        input.join(", ")
    };
    let crop = at
        .and_then(|at| level.as_ref()?.block_property(host, at, AGE))
        .unwrap_or_else(|| "none".to_owned());
    let pointing = at
        .map(|at| format!("{at:?}"))
        .unwrap_or_else(|| "none".to_owned());
    format!(
        "Plant Fertilizer at {pos:?}: {energy}/{CAPACITY_FE} FE, input {input}, progress {progress}/{}, pointing {pointing}, crop age {crop}, facing {facing}, augments speed {} efficiency {:.2}, t={time}",
        PlantFertilizer::bar_length(augments),
        augments.speed,
        augments.efficiency,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_container_has_eighteen_input_and_no_output_slots() {
        let container = container();
        assert_eq!(container.slots, 18);
        assert_eq!(container.inputs, (0..18).collect::<Vec<_>>());
        assert!(container.outputs.is_empty(), "setOutputFilter(stack -> false)");
        assert_eq!(container.faces.len(), 6);
    }

    #[test]
    fn the_working_area_is_the_single_block_behind_the_machine() {
        assert_eq!(behind("west"), Some([1, 0, 0]));
        assert_eq!(behind("east"), Some([-1, 0, 0]));
        assert_eq!(behind("north"), Some([0, 0, 1]));
        assert_eq!(behind("south"), Some([0, 0, -1]));
        assert_eq!(behind("up"), None);
    }

    #[test]
    fn a_saved_machine_clamps_its_numbers_and_a_fresh_bar_is_full() {
        let mut nbt = Compound::new();
        assert_eq!(read_int(&nbt, KEY_ENERGY), 0);
        assert_eq!(read_progress(&nbt), None);
        write_int(&mut nbt, KEY_ENERGY, 9_000);
        write_int(&mut nbt, KEY_PROGRESS, 12);
        assert_eq!(read_int(&nbt, KEY_ENERGY), 9_000);
        assert_eq!(read_progress(&nbt), Some(12));
        assert!(has_contents(&nbt));
    }

    #[test]
    fn a_broken_machine_carries_its_numbers_but_nothing_else() {
        let mut nbt = Compound::new();
        write_int(&mut nbt, KEY_ENERGY, 9_000);
        write_int(&mut nbt, KEY_PROGRESS, 12);
        let patch = carried_patch(&nbt);
        let Some(Tag::Compound(payload)) = patch.get(&"minecraft:block_entity_data".into()) else {
            panic!("no block entity data in the patch");
        };
        assert_eq!(
            payload.get(&"id".into()),
            Some(&Tag::String(ENTITY_ID.into()))
        );
        assert_eq!(read_int(payload, KEY_ENERGY), 9_000);
        assert_eq!(read_progress(payload), Some(12));
    }

    #[test]
    fn an_empty_machine_has_nothing_to_carry() {
        let nbt = Compound::new();
        assert!(!has_contents(&nbt));
    }
}
