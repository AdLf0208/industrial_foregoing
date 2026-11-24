//! The game side of the **Plant Sower** (`PlantSowerTile` at `ce2852db`).
//!
//! Upstream's `RangeManager.RangeType.TOP_UP` at range 0 points two blocks above the machine; the
//! soil is the block one above. On a finished bar, with that cell empty or water and 1,000 FE, the
//! first filtered seed whose block can live on that soil plants the block's **default state** and
//! shrinks by one. A powered miss still costs `powerPerOperation / 50 = 20` FE; an empty buffer
//! costs nothing. The pure arithmetic lives in [`crate::plantsower`]; `src/runtime.rs` is untouched.

use std::sync::OnceLock;

use mod_api::{
    self, BlockEntityPart, BlockEntityRef, BlockEntityTickEvent, BlockEntityTicksEvent, BlockPart,
    BlockState, BlockUpdateFlags, Compound, Host, InteractionResult, ItemKey, ItemPart, LevelHost,
    ModContainer, ModEnergyBuffer, OnBlockPlacedByEvent, Registrar, ServerHost,
    StateForPlacementEvent, Tag, UseItemOnEvent,
};

use crate::plantsower::{Attempt, CAPACITY_FE, MAX_PROGRESS, SLOTS};

/// `ModuleAgricultureHusbandry.PLANT_SOWER`.
pub const BLOCK: &str = "industrialforegoing:plant_sower";
/// The bare block entity type name; the mod's namespace qualifies it.
pub const ENTITY: &str = "plant_sower";
/// The `minecraft:block_entity_data` id.
pub const ENTITY_ID: &str = "industrialforegoing:plant_sower";
/// Titanium's `RotationType.FOUR_WAY` property, carried by the block's state file.
pub const SUBFACING: &str = "subfacing";
/// Saved keys: the buffer and the falling bar. The input is the block entity's container.
const KEY_ENERGY: &str = "energy";
const KEY_PROGRESS: &str = "progress";
/// `BushBlock.mayPlaceOn` for the crop families this slice can name at this pin.
const FARMLAND: &str = "minecraft:farmland";
const SOUL_SAND: &str = "minecraft:soul_sand";
const JUNGLE_LOG: &str = "minecraft:jungle_log";
/// The pointed cell must be empty or water (`level.isEmptyBlock(pos) || is(Blocks.WATER)`).
const AIR: &str = "minecraft:air";
const WATER: &str = "minecraft:water";

static KIND: OnceLock<mod_api::BlockEntityTypeKey> = OnceLock::new();
static ITEM: OnceLock<ItemKey> = OnceLock::new();

/// Declare the block entity type, its 10,000 FE buffer, its 9-slot input container and its Parts.
pub(crate) fn declare(r: &mut dyn Registrar) {
    let block = r.block_key(BLOCK);
    let item = r.item_key(BLOCK);
    let _ = ITEM.set(item);
    r.item_part(item, SowerItem);
    let kind = r.block_entity_type(ENTITY).blocks(&[ENTITY]).build();
    let _ = KIND.set(kind);
    r.block_entity_part(&kind, SowerPart)
        .expect("the plant sower's Part attaches to the type this mod just declared");
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

/// `SidedInventoryComponent("input", 54 + 18 * 3, 22, 9, 0)`: nine input-only slots. The pin has
/// no per-slot item predicate and no filter widget, so the tick enforces the seed shape (gap 86).
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
pub(crate) struct SowerPart;

#[mod_api::horizon_part]
impl BlockPart for SowerPart {
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

/// `RangeManager.RangeType.TOP_UP` at range 0: the target is two blocks up, the soil one up.
#[must_use]
pub fn area(pos: [i32; 3]) -> ([i32; 3], [i32; 3]) {
    ([pos[0], pos[1] + 2, pos[2]], [pos[0], pos[1] + 1, pos[2]])
}

/// The soil a seed's block may be planted on. The pin has no `BushBlock.mayPlaceOn`, so the crop
/// families the jar's `BushBlock` accepts are named explicitly; anything else is a miss (gap 85).
#[must_use]
pub fn soil_for(block: &str) -> Option<&'static str> {
    match block {
        "minecraft:wheat"
        | "minecraft:carrot"
        | "minecraft:potato"
        | "minecraft:beetroot"
        | "minecraft:torchflower"
        | "minecraft:pitcher_crop"
        | "minecraft:sweet_berry_bush" => Some(FARMLAND),
        "minecraft:nether_wart" => Some(SOUL_SAND),
        "minecraft:cocoa" => Some(JUNGLE_LOG),
        _ => None,
    }
}

#[mod_api::horizon_part]
impl BlockEntityPart for SowerPart {
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    /// One server tick: fall the bar; at zero, with power and a plantable seed, plant the block's
    /// default state two blocks above, shrink the stack and let the pure rule spend the FE.
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let entity = &mut *event.entity;
        if entity.items.len() < SLOTS {
            entity.items.resize(SLOTS, None);
        }
        let mut sower = crate::plantsower::PlantSower::from_saved(
            read_int(&entity.nbt, KEY_ENERGY),
            read_progress(&entity.nbt),
        );
        let augments = crate::read_augments(&entity.nbt);
        let attempt = if sower.progress > augments.speed.max(1) {
            Attempt::Miss
        } else if sower.energy < crate::plantsower::POWER_PER_OPERATION {
            Attempt::NoEnergy
        } else {
            let (target, soil) = area(pos);
            match plant(host, &mut entity.items, target, soil) {
                true => Attempt::Planted,
                false => Attempt::Miss,
            }
        };
        let _step = sower.tick(augments, attempt);
        write_int(&mut entity.nbt, KEY_ENERGY, sower.energy);
        write_int(&mut entity.nbt, KEY_PROGRESS, sower.progress);
    }
}

/// The pointed cell's block id, as `/setblock` reads it: `minecraft:air`, `minecraft:water`, …
fn block_id(host: &dyn Host, level: &mod_api::LevelRef, pos: [i32; 3]) -> Option<String> {
    let block = level.block(host, pos)?;
    Some(block.id(host))
}

/// Whether the first plantable seed in the nine slots was planted. `ItemBlock` is the pin's
/// `BlockItem.getBlock`, and the default state is the block's `defaultBlockState` — the seed rule
/// the artefact applies to `wheat_seeds` → `minecraft:wheat[age=0]`.
fn plant(
    host: &mut dyn ServerHost,
    items: &mut [Option<mod_api::ItemStack>],
    target: [i32; 3],
    soil: [i32; 3],
) -> bool {
    let Some(level) = host.level(host.dimension()) else {
        return false;
    };
    if !level.is_loaded(&*host, target)
        || !block_id(&*host, &level, target).is_some_and(|id| id == AIR || id == WATER)
    {
        return false;
    }
    let Some(wanted_soil) = block_id(&*host, &level, soil) else {
        return false;
    };
    let mut chosen = None;
    for (index, stack) in items.iter().enumerate() {
        let Some(stack) = stack else {
            continue;
        };
        let Some(block) = host.item_block(stack.item()) else {
            continue;
        };
        let id = block.id(&*host);
        if soil_for(&id).is_some_and(|wanted| wanted == wanted_soil) {
            chosen = Some((index, block));
            break;
        }
    }
    let Some((index, block)) = chosen else {
        return false;
    };
    let Some(state) = host.block_state_default(block) else {
        return false;
    };
    let _ = level.set_block(
        host,
        target,
        state,
        BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS,
    );
    if let Some(stack) = items.get_mut(index).and_then(Option::take) {
        items[index] = stack.shrink(1);
    }
    true
}

/// The machine's item: a broken one carries its buffer and bar back.
pub(crate) struct SowerItem;

#[mod_api::horizon_part]
impl ItemPart for SowerItem {
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

/// FE, an unfinished bar or a seed in the input give the broken machine something to carry.
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

/// `/industrialforegoing info` for this machine: the buffer, the bar, the pointed cell and soil,
/// the saved input, the facing and the game time — all read from the game, not from `runtime.rs`.
pub(crate) fn readout(host: &mut dyn ServerHost, pos: [i32; 3]) -> String {
    let Some(entity) = crate::our_entity(host, pos, BLOCK) else {
        return format!("{pos:?} is not a plant sower.");
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
    let (target, soil) = area(pos);
    let soil_name = level
        .as_ref()
        .and_then(|level| block_id(host, level, soil))
        .unwrap_or_else(|| "none".to_owned());
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
    format!(
        "Plant Sower at {pos:?}: {energy}/{CAPACITY_FE} FE, progress {progress}/{}, pointing {target:?}, soil {soil_name}, input {input}, facing {facing}, augments speed {} efficiency {:.2}, t={time}",
        crate::plantsower::PlantSower::bar_length(augments),
        augments.speed,
        augments.efficiency,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_container_declares_nine_input_slots() {
        let container = container();
        assert_eq!(container.slots, 9);
        assert_eq!(container.inputs, (0..9).collect::<Vec<_>>());
        assert_eq!(container.faces.len(), 6);
    }

    #[test]
    fn the_working_area_is_two_blocks_above_the_machine() {
        assert_eq!(area([10, 20, 30]), ([10, 22, 30], [10, 21, 30]));
    }

    #[test]
    fn the_crop_soils_are_the_artefact_s_may_place_on_families() {
        assert_eq!(soil_for("minecraft:wheat"), Some(FARMLAND));
        assert_eq!(soil_for("minecraft:nether_wart"), Some(SOUL_SAND));
        assert_eq!(soil_for("minecraft:cocoa"), Some(JUNGLE_LOG));
        assert_eq!(soil_for("minecraft:stone"), None);
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
        write_int(&mut nbt, KEY_PROGRESS, 8);
        let patch = carried_patch(&nbt);
        let Some(Tag::Compound(payload)) = patch.get(&"minecraft:block_entity_data".into()) else {
            panic!("no block entity data in the patch");
        };
        assert_eq!(
            payload.get(&"id".into()),
            Some(&Tag::String(ENTITY_ID.into()))
        );
        assert_eq!(read_int(payload, KEY_ENERGY), 4_000);
        assert_eq!(read_progress(payload), Some(8));
    }

    #[test]
    fn an_empty_machine_has_nothing_to_carry() {
        assert!(!has_contents(&Compound::new()));
    }
}
