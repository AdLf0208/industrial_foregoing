//! The game side of the **Plant Gatherer** (`PlantGathererTile` at `ce2852db`).
//!
//! At range 0 its `RangeType.BEHIND` area is the single cell opposite the machine's `subfacing`.
//! On a finished bar, with at least 400 FE, the cell is checked for a mature crop: the pin rolls
//! that block's own loot table (`LevelRef::block_drops`, vanilla's `Block.getDrops`), inserts every
//! stack into the 15-slot output and replaces the crop with air, exactly as upstream's
//! `BlockCropPlantRecollectable` does. The pure arithmetic lives in [`crate::plantgather`];
//! `src/runtime.rs` is untouched.

use std::sync::OnceLock;

use mod_api::{
    self, BlockEntityPart, BlockEntityRef, BlockEntityTickEvent, BlockEntityTicksEvent, BlockPart,
    BlockState, BlockUpdateFlags, Compound, Host, HostBlockStates, InteractionResult, ItemKey,
    ItemPart, LevelHost, ModContainer, ModEnergyBuffer, OnBlockPlacedByEvent, Registrar,
    ServerHost, StateForPlacementEvent, Tag, UseItemOnEvent,
};

use crate::plantgather::{
    Attempt, CAPACITY_FE, MAX_PROGRESS, OUTPUT_SLOTS, POWER_PER_OPERATION, PlantGatherer,
};

/// `ModuleAgricultureHusbandry.PLANT_GATHERER`.
pub const BLOCK: &str = "industrialforegoing:plant_gatherer";
/// The bare block entity type name; the mod's namespace qualifies it.
pub const ENTITY: &str = "plant_gatherer";
/// The `minecraft:block_entity_data` id.
pub const ENTITY_ID: &str = "industrialforegoing:plant_gatherer";
/// Titanium's `RotationType.FOUR_WAY` property, carried by the block's state file.
pub const SUBFACING: &str = "subfacing";
/// Saved keys: the buffer and the falling bar. The output is the block entity's container.
const KEY_ENERGY: &str = "energy";
const KEY_PROGRESS: &str = "progress";
/// The mature-crop property (`CropBlock.isMaxAge` reads the last value of `age`).
const AGE: &str = "age";
/// What a harvested crop leaves behind (`BlockCropPlantRecollectable.doHarvestOperation`).
const AIR: &str = "minecraft:air";

static KIND: OnceLock<mod_api::BlockEntityTypeKey> = OnceLock::new();
static ITEM: OnceLock<ItemKey> = OnceLock::new();

/// Declare the block entity type, its 10,000 FE buffer, its 15-slot output container and its Parts.
pub(crate) fn declare(r: &mut dyn Registrar) {
    let block = r.block_key(BLOCK);
    let item = r.item_key(BLOCK);
    let _ = ITEM.set(item);
    r.item_part(item, GathererItem);
    let kind = r.block_entity_type(ENTITY).blocks(&[ENTITY]).build();
    let _ = KIND.set(kind);
    r.block_entity_part(&kind, GathererPart)
        .expect("the plant gatherer's Part attaches to the type this mod just declared");
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

/// `SidedInventoryComponent("output", 78, 22, 3 * 5, 0)`: 15 slots, no input filter. The harvest
/// never takes from them; the tick uses them only to decide whether another drop fits.
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

/// The placement rotation and the augment seat, then the block entity's tick.
pub(crate) struct GathererPart;

#[mod_api::horizon_part]
impl BlockPart for GathererPart {
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
impl BlockEntityPart for GathererPart {
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    /// One server tick: fall the bar; at zero, with 400 FE and room in the output, harvest the crop
    /// behind the machine and let the pure rule spend the FE and choose the cooldown.
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let entity = &mut *event.entity;
        if entity.items.len() < OUTPUT_SLOTS {
            entity.items.resize(OUTPUT_SLOTS, None);
        }
        let mut gatherer = PlantGatherer::from_saved(
            read_int(&entity.nbt, KEY_ENERGY),
            read_progress(&entity.nbt),
        );
        let augments = crate::read_augments(&entity.nbt);
        // The area scan and the loot roll belong to the attempt, not to every falling tick.
        let attempt = if gatherer.progress > augments.speed.max(1) {
            Attempt::NotATarget
        } else if gatherer.energy < POWER_PER_OPERATION {
            Attempt::NoEnergy
        } else if entity.items.iter().all(Option::is_some) {
            Attempt::OutputFull
        } else {
            match pointed(host, pos).and_then(|at| harvest(host, at, &mut entity.items)) {
                None => Attempt::NotATarget,
                Some(stacks) => Attempt::Harvested { stacks },
            }
        };
        let _step = gatherer.tick(augments, attempt);
        write_int(&mut entity.nbt, KEY_ENERGY, gatherer.energy);
        write_int(&mut entity.nbt, KEY_PROGRESS, gatherer.progress);
    }
}

/// The `RangeType.BEHIND` cell at range 0, read from the placed block's own `subfacing`.
fn pointed(host: &mut dyn ServerHost, pos: [i32; 3]) -> Option<[i32; 3]> {
    let level = host.level(host.dimension())?;
    let facing = level.block_property(&*host, pos, SUBFACING)?;
    let [dx, dy, dz] = crate::plantnet::behind(&facing)?;
    Some([pos[0] + dx, pos[1] + dy, pos[2] + dz])
}

/// `CropBlock.isMaxAge(state)`: an `age` property at its last declared value. The jar's other
/// recollectables (nether wart, berries, cocoa) also key off `age`, so this covers the same family.
fn mature(host: &dyn ServerHost, pos: [i32; 3]) -> bool {
    let Some(level) = host.level(host.dimension()) else {
        return false;
    };
    let Some(age) = level
        .block_property(host, pos, AGE)
        .and_then(|age| age.parse::<i32>().ok())
    else {
        return false;
    };
    level
        .block(host, pos)
        .and_then(|block| host.block_states().definition(block))
        .and_then(|properties| properties.into_iter().find(|property| property.name == AGE))
        .and_then(|property| property.values.last().cloned())
        .and_then(|last| last.parse::<i32>().ok())
        == Some(age)
}

/// Roll the pointed block's loot table, put every stack in the output and clear the crop. `None`
/// means it was not mature, or the level could not be read; the block is only removed on success.
fn harvest(
    host: &mut dyn ServerHost,
    pos: [i32; 3],
    output: &mut [Option<mod_api::ItemStack>],
) -> Option<usize> {
    if !mature(&*host, pos) {
        return None;
    }
    let level = host.level(host.dimension())?;
    let drops = level.block_drops(host, pos, None, None).ok()??;
    // `LevelRef` is a copyable handle, so it survives the mutable borrow of the host above.
    let state = host.block_state_id(AIR)?;
    let _ = level.set_block(
        host,
        pos,
        state,
        BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS,
    );
    let mut inserted = 0;
    for stack in drops {
        if crate::dissolution::put(&*host, output, stack) {
            inserted += 1;
        }
    }
    Some(inserted)
}

/// The machine's item: a broken one carries its buffer and bar back.
pub(crate) struct GathererItem;

#[mod_api::horizon_part]
impl ItemPart for GathererItem {
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
// The saved compound and the harness
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

/// A bar the save never wrote starts full (`ProgressBarComponent(40, 40)`).
fn read_progress(nbt: &Compound) -> Option<i32> {
    match nbt.get(&KEY_PROGRESS.into()) {
        Some(Tag::Int(value)) => Some(*value),
        _ => None,
    }
}

/// FE, an unfinished bar or a drop in the output all give the broken machine something to carry.
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
    entity
        .kind(host)
        .is_some_and(|kind| KIND.get() == Some(&kind))
}

/// The item `block_drops` may write to.
pub(crate) fn item() -> Option<ItemKey> {
    ITEM.get().copied()
}

/// `/industrialforegoing energy|progress` on this machine: one int of its saved compound.
pub(crate) fn set_int(host: &mut dyn ServerHost, pos: [i32; 3], key: &str, value: i32) -> bool {
    let Some(entity) = crate::our_entity(host, pos, BLOCK) else {
        return false;
    };
    let mut patch = Compound::new();
    write_int(&mut patch, key, value);
    entity.merge_data(host, patch).unwrap_or(false)
}

/// `/industrialforegoing info` for this machine: the buffer, the bar, the pointed cell, the crop's
/// age, the saved output and the game time — all read from the game, not from `runtime.rs`.
pub(crate) fn readout(host: &mut dyn ServerHost, pos: [i32; 3]) -> String {
    let Some(entity) = crate::our_entity(host, pos, BLOCK) else {
        return format!("{pos:?} is not a plant gatherer.");
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
    let crop = at
        .and_then(|at| level.as_ref()?.block_property(host, at, AGE))
        .unwrap_or_else(|| "none".to_owned());
    let pointing = at
        .map(|at| format!("{at:?}"))
        .unwrap_or_else(|| "none".to_owned());
    let mut output = Vec::new();
    for slot in 0..OUTPUT_SLOTS as u32 {
        if let Ok(Some(stack)) = entity.item_slot(&*host, slot) {
            output.push(format!("{}x {}", stack.count(), stack.item().id(&*host)));
        }
    }
    let output = if output.is_empty() {
        "empty".to_owned()
    } else {
        output.join(", ")
    };
    format!(
        "Plant Gatherer at {pos:?}: {energy}/{CAPACITY_FE} FE, progress {progress}/{}, pointing {pointing}, crop age {crop}, output {output}, facing {facing}, augments speed {} efficiency {:.2}, t={time}",
        PlantGatherer::bar_length(augments),
        augments.speed,
        augments.efficiency,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_container_declares_fifteen_output_slots() {
        let container = container();
        assert_eq!(container.slots, 15);
        assert_eq!(container.outputs, (0..15).collect::<Vec<_>>());
        assert_eq!(container.faces.len(), 6);
    }

    #[test]
    fn the_working_area_is_the_single_block_behind_the_machine() {
        assert_eq!(crate::plantnet::behind("west"), Some([1, 0, 0]));
        assert_eq!(crate::plantnet::behind("east"), Some([-1, 0, 0]));
        assert_eq!(crate::plantnet::behind("north"), Some([0, 0, 1]));
        assert_eq!(crate::plantnet::behind("south"), Some([0, 0, -1]));
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
        write_int(&mut nbt, KEY_ENERGY, 4_000);
        write_int(&mut nbt, KEY_PROGRESS, 12);
        let patch = carried_patch(&nbt);
        let Some(Tag::Compound(payload)) = patch.get(&"minecraft:block_entity_data".into()) else {
            panic!("no block entity data in the patch");
        };
        assert_eq!(
            payload.get(&"id".into()),
            Some(&Tag::String(ENTITY_ID.into()))
        );
        assert_eq!(read_int(payload, KEY_ENERGY), 4_000);
        assert_eq!(read_progress(payload), Some(12));
    }

    #[test]
    fn an_empty_machine_has_nothing_to_carry() {
        assert!(!has_contents(&Compound::new()));
    }
}
