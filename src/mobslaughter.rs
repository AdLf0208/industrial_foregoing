//! The game side of the **Mob Slaughter Factory** (`ModuleAgricultureHusbandry.SLAUGHTER_FACTORY`).
//!
//! Upstream's `SlaughterFactoryTile` is an `IndustrialAreaWorkingTile` over Titanium's
//! `ProgressBarComponent`: a bar that counts **down** from 100, a 10,000 FE buffer, a
//! **8,000 mB meat** tank and an **8,000 mB pink slime** tank, both `Action.DRAIN` with a
//! validator per fluid, and a working area of the block **behind** the machine expanded two blocks
//! up. At zero the attempt kills the first Mob in that area for 400 FE and fills the tanks from its
//! health; the arithmetic is [`crate::slaughter`].
//!
//! # The entity scan
//!
//! `LevelRef::entities_in` returns handles; `Host::mob` narrows one to a vanilla `Mob`, which is
//! what `getEntitiesOfClass(Mob.class, …)` selects (players and items are not Mobs), and
//! `LivingRef::alive`/`health` and `EntityRef::permanently_invulnerable` are the three filters
//! `work()` applies. The kill is `EntityRef::remove(KILLED)` — vanilla's removal reason, which
//! (like upstream) drops no loot and no experience.
//!
//! `entity instanceof Animal` has no pin expression: the port reads the type's `MobCategory`, where
//! vanilla's animals are `CREATURE` (the Axolotl is `AXOLOTLS`). The one vanilla mob that is a
//! `CREATURE` but not an `Animal` is the wandering trader; recorded as a deviation.
//!
//! # The tanks
//!
//! `native_fluid_insert` takes water and lava only, so a mod fluid cannot be inserted into either
//! tank; `Registrar::block_fluids` declares both as real drain-only handlers and the machine writes
//! its own `fluids` compound (one `{Fluid, Amount}` per tank, meat first), as slice 5's chamber does.

use std::sync::OnceLock;

use mod_api::{
    self, BlockEntityPart, BlockEntityRef, BlockEntityTickEvent, BlockEntityTicksEvent, BlockPart,
    Compound, Direction, EntityRef, FLUIDS, FluidIdentity, Host, HostTransaction, ItemKey,
    ItemPart, LevelHost, MobCategory, ModEnergyBuffer, ModFluidTanks, OnBlockPlacedByEvent,
    Registrar, RemovalReason, ServerHost, StateForPlacementEvent, Tag,
};

use crate::slaughter::{
    AREA_UP, CAPACITY_FE, Factory, MEAT, MEAT_TANK_MB, PINK_SLIME, PINK_SLIME_TANK_MB, Step, Victim,
};

/// The machine (`ModuleAgricultureHusbandry.SLAUGHTER_FACTORY`).
pub const BLOCK: &str = "industrialforegoing:mob_slaughter_factory";
/// Its block entity type, a bare name so the mod's namespace qualifies it.
pub const ENTITY: &str = "mob_slaughter_factory";
/// The block entity type's registry id, which `minecraft:block_entity_data` tags its payload with.
pub const ENTITY_ID: &str = "industrialforegoing:mob_slaughter_factory";
/// The state property Titanium's `RotationType.FOUR_WAY` sets.
pub const SUBFACING: &str = "subfacing";
/// The saved keys: the buffer's amount (the energy declaration's own) and the bar.
const KEY_ENERGY: &str = "energy";
const KEY_PROGRESS: &str = "progress";
/// The two tanks, in declaration order: meat is tank 0, pink slime tank 1.
const TANKS: [&str; 2] = [MEAT, PINK_SLIME];
/// The side the tanks are read through (all six reach them).
const READ_FACE: Direction = Direction::Up;

/// The block entity type and the item, resolved once in `register`: `block_drops` and the command
/// path run with a read-only host, which cannot name either.
static KIND: OnceLock<mod_api::BlockEntityTypeKey> = OnceLock::new();
static ITEM: OnceLock<ItemKey> = OnceLock::new();

// ---------------------------------------------------------------------------
// The declarations
// ---------------------------------------------------------------------------

/// Declare the machine: its block entity type, its 10,000 FE buffer, its two 8,000 mB drain-only
/// tanks, the Part that ticks it and the Part on its item.
pub(crate) fn declare(r: &mut dyn Registrar) {
    let block = r.block_key(BLOCK);
    let item = r.item_key(BLOCK);
    let _ = ITEM.set(item);
    r.item_part(item, FactoryItem);
    let kind = r.block_entity_type(ENTITY).blocks(&[ENTITY]).build();
    let _ = KIND.set(kind);
    r.block_entity_part(&kind, FactoryPart)
        .expect("the factory's Part attaches to the type this mod just declared");
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
}

/// The two tanks: `SidedFluidTankComponent("meat", 8000, …, Action.DRAIN)` and the same for
/// `"pink_slime"`. Every face reaches both for reading and draining; no face may fill either (the
/// machine fills them itself), so `inputs` is empty.
#[must_use]
pub fn tanks() -> ModFluidTanks {
    let both = vec![0usize, 1];
    let mut tanks = ModFluidTanks::default();
    tanks.tanks = vec![MEAT_TANK_MB, PINK_SLIME_TANK_MB];
    tanks.faces = [
        both.clone(),
        both.clone(),
        both.clone(),
        both.clone(),
        both.clone(),
        both,
    ];
    tanks.outputs = vec![0, 1];
    tanks
}

// ---------------------------------------------------------------------------
// The Part
// ---------------------------------------------------------------------------

/// The factory: its placement rotation, the augment seat and its tick.
pub(crate) struct FactoryPart;

#[mod_api::horizon_part]
impl BlockPart for FactoryPart {
    /// `RotationType.FOUR_WAY`: a placed factory faces its placer.
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
impl BlockEntityPart for FactoryPart {
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    /// One server tick: fall the bar, and at zero kill the first mob in the working area for 400 FE.
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let entity = &mut *event.entity;
        let augments = crate::read_augments(&entity.nbt);
        let progress = match entity.nbt.get(&KEY_PROGRESS.into()) {
            Some(Tag::Int(value)) => Some(*value),
            _ => None,
        };
        let [meat, slime] = read_tanks(&entity.nbt);
        let mut factory =
            Factory::from_saved(read_int(&entity.nbt, KEY_ENERGY), progress, meat, slime);
        let mut found: Option<Found> = None;
        let step = factory.tick_with(augments, || {
            found = victim(host, pos);
            found.as_ref().map(|found| found.victim)
        });
        if let (Step::Killed(_), Some(found)) = (step, found) {
            let _ = found.entity.remove(host, RemovalReason::Killed);
        }
        write_int(&mut entity.nbt, KEY_ENERGY, factory.energy);
        write_int(&mut entity.nbt, KEY_PROGRESS, factory.progress);
        write_tanks(&mut entity.nbt, [factory.meat_mb, factory.pink_slime_mb]);
    }
}

/// A mob the machine may kill: its handle and the two numbers the rules read.
struct Found {
    entity: EntityRef,
    victim: Victim,
}

/// The first mob in `RangeManager(BEHIND)`'s box, expanded two blocks up: alive and not permanently
/// invulnerable, as `work()` filters them.
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
        if host.mob(entity).is_none() {
            continue;
        }
        let Some(living) = host.living(entity) else {
            continue;
        };
        if !living.alive(&*host).unwrap_or(false) {
            continue;
        }
        if entity.permanently_invulnerable(&*host).unwrap_or(false) {
            continue;
        }
        let health = living.health(&*host).unwrap_or(0.0);
        let animal = animal_category(entity.classification(&*host));
        return Some(Found {
            entity,
            victim: Victim { health, animal },
        });
    }
    None
}

/// The offset of the block behind a machine facing `facing` (`Direction.getOpposite`), which is
/// where `RangeManager.RangeType.BEHIND` puts the working area's floor.
fn behind(facing: &str) -> Option<[i32; 3]> {
    match facing {
        "north" => Some([0, 0, 1]),
        "south" => Some([0, 0, -1]),
        "east" => Some([-1, 0, 0]),
        "west" => Some([1, 0, 0]),
        _ => None,
    }
}

/// `entity instanceof Animal`, as far as the pin can answer it: vanilla's animals are the
/// `CREATURE` category, and the Axolotl is `AXOLOTLS`. The wandering trader is the one vanilla
/// `CREATURE` that is not an `Animal` (deviation, recorded in `notes.md`).
fn animal_category(category: Option<MobCategory>) -> bool {
    matches!(
        category,
        Some(MobCategory::Creature | MobCategory::Axolotls)
    )
}

// ---------------------------------------------------------------------------
// The item
// ---------------------------------------------------------------------------

/// The factory's item: a broken machine's drop carries both tanks, the buffer and the bar, and
/// placing it clamps them back to the capacities (slice 2's route).
pub(crate) struct FactoryItem;

#[mod_api::horizon_part]
impl ItemPart for FactoryItem {
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

/// The two tanks' amounts from the entity's `fluids`: a tank only counts while it holds its own
/// fluid, so a `/data merge` that put water in the meat tank reads as empty.
fn read_tanks(nbt: &Compound) -> [i32; 2] {
    let mut out = [0; 2];
    let Some(Tag::List(entries)) = nbt.get(&"fluids".into()) else {
        return out;
    };
    for entry in entries {
        let Tag::Compound(entry) = entry else {
            continue;
        };
        let fluid = match entry.get(&"Fluid".into()) {
            Some(Tag::String(fluid)) if fluid.to_string_lossy() != "minecraft:empty" => {
                fluid.to_string_lossy()
            }
            _ => continue,
        };
        let Some(index) = TANKS.iter().position(|tank| *tank == fluid) else {
            continue;
        };
        let capacity = if index == 0 {
            MEAT_TANK_MB
        } else {
            PINK_SLIME_TANK_MB
        };
        let amount = match entry.get(&"Amount".into()) {
            Some(Tag::Int(amount)) => *amount,
            _ => 0,
        };
        out[index] = amount.clamp(0, capacity);
    }
    out
}

/// Both tanks back in the pin's shape, in declaration order (meat first).
fn write_tanks(nbt: &mut Compound, amounts: [i32; 2]) {
    let entries = amounts
        .iter()
        .enumerate()
        .map(|(index, mb)| {
            let capacity = if index == 0 {
                MEAT_TANK_MB
            } else {
                PINK_SLIME_TANK_MB
            };
            let mut entry = Compound::new();
            entry.insert(
                "Fluid".into(),
                Tag::String(
                    if *mb > 0 {
                        TANKS[index]
                    } else {
                        "minecraft:empty"
                    }
                    .into(),
                ),
            );
            entry.insert("Amount".into(), Tag::Int(i32::clamp(*mb, 0, capacity)));
            Tag::Compound(entry)
        })
        .collect();
    nbt.insert("fluids".into(), Tag::List(entries));
}

/// Whether the factory holds anything worth carrying on its item.
pub(crate) fn has_contents(saved: &Compound) -> bool {
    let [meat, slime] = read_tanks(saved);
    // A save the machine never wrote has no bar yet, which is a **full** bar, not an empty one.
    let progress = match saved.get(&KEY_PROGRESS.into()) {
        Some(Tag::Int(value)) => *value,
        _ => crate::slaughter::MAX_PROGRESS,
    };
    read_int(saved, KEY_ENERGY) > 0
        || progress < crate::slaughter::MAX_PROGRESS
        || meat > 0
        || slime > 0
        || crate::has_seated_augment(saved)
}

/// The `minecraft:block_entity_data` payload a broken factory's item carries.
pub(crate) fn carried_patch(saved: &Compound) -> Compound {
    let mut payload = saved.clone();
    payload.insert("id".into(), Tag::String(ENTITY_ID.into()));
    let mut patch = Compound::new();
    patch.insert("minecraft:block_entity_data".into(), Tag::Compound(payload));
    patch
}

/// The corrections a placed factory needs when its item carried values outside the capacities.
#[must_use]
pub(crate) fn clamp_patch(saved: &Compound) -> Option<Compound> {
    let [meat, slime] = read_tanks(saved);
    let mut patch = Compound::new();
    let mut changed = false;
    let energy = read_int(saved, KEY_ENERGY);
    if !(0..=CAPACITY_FE).contains(&energy) {
        write_int(&mut patch, KEY_ENERGY, energy.clamp(0, CAPACITY_FE));
        changed = true;
    }
    let progress = read_int(saved, KEY_PROGRESS);
    if !(0..=crate::slaughter::MAX_PROGRESS).contains(&progress) {
        write_int(
            &mut patch,
            KEY_PROGRESS,
            progress.clamp(0, crate::slaughter::MAX_PROGRESS),
        );
        changed = true;
    }
    if let Some(Tag::List(entries)) = saved.get(&"fluids".into()) {
        let too_full = entries.iter().any(|entry| match entry {
            Tag::Compound(entry) => match entry.get(&"Amount".into()) {
                Some(Tag::Int(amount)) => *amount > PINK_SLIME_TANK_MB,
                _ => false,
            },
            _ => false,
        });
        if too_full {
            write_tanks(&mut patch, [meat, slime]);
            changed = true;
        }
    }
    changed.then_some(patch)
}

// ---------------------------------------------------------------------------
// The commands
// ---------------------------------------------------------------------------

/// `/industrialforegoing energy`/`progress` on the factory: one int of its saved compound.
pub(crate) fn set_int(host: &mut dyn ServerHost, pos: [i32; 3], key: &str, value: i32) -> bool {
    let Some(entity) = crate::our_entity(host, pos, BLOCK) else {
        return false;
    };
    let mut patch = Compound::new();
    write_int(&mut patch, key, value);
    entity.merge_data(host, patch).unwrap_or(false)
}

/// `/industrialforegoing drain`: take both tanks out through the declared capability, which is what
/// makes them handlers rather than private counters.
pub(crate) fn drain(host: &mut dyn ServerHost, pos: [i32; 3], amount: i32) {
    let want = u32::try_from(amount.max(0)).unwrap_or(u32::MAX);
    let answer = host
        .transaction::<String, ()>(|host, tx| {
            let Some(level) = host.level(host.dimension()) else {
                return Ok(format!(
                    "Mob Slaughter Factory at {pos:?}: no drainable fluid."
                ));
            };
            let Some(tanks) = level.capability(host, &FLUIDS, pos, Some(READ_FACE)) else {
                return Ok(format!(
                    "Mob Slaughter Factory at {pos:?}: no drainable fluid."
                ));
            };
            let mut parts = Vec::new();
            for index in 0..TANKS.len() {
                let Some(resource) = tanks.resource(host, index) else {
                    continue;
                };
                let taken = tanks.extract(host, tx, index, &resource, want);
                if taken == 0 {
                    continue;
                }
                let name = match resource.fluid() {
                    FluidIdentity::Resolved(fluid) => fluid.id(host),
                    FluidIdentity::Unresolved(fluid) => fluid.id(host),
                    _ => "an unknown fluid".to_owned(),
                };
                parts.push(format!("{taken} mB of {name}"));
            }
            Ok(if parts.is_empty() {
                format!("Mob Slaughter Factory at {pos:?}: no drainable fluid.")
            } else {
                format!(
                    "Mob Slaughter Factory at {pos:?}: drained {}.",
                    parts.join(" and ")
                )
            })
        })
        .unwrap_or_else(|()| format!("{pos:?} is not a mob slaughter factory."));
    crate::say(host, &answer);
}

/// `/industrialforegoing info` for the factory: the buffer, both tanks as the entity holds them
/// **and** as the declared capability reports them, the bar, the mobs in the working area, the
/// facing and the game time.
pub(crate) fn readout(host: &mut dyn ServerHost, pos: [i32; 3]) -> String {
    let Some(entity) = crate::our_entity(host, pos, BLOCK) else {
        return format!("{pos:?} is not a mob slaughter factory.");
    };
    let Some(data) = entity.data(&*host) else {
        return format!("{pos:?} is not a mod block entity.");
    };
    let [meat, slime] = read_tanks(&data);
    let progress = read_int(&data, KEY_PROGRESS);
    let augments = crate::read_augments(&data);
    let [(cap_meat, cap_meat_mb), (cap_slime, cap_slime_mb)] = capability_tanks(host, pos);
    let victims = victim_count(host, pos);
    let facing = entity
        .level(&*host)
        .and_then(|level| level.block_property(host, pos, SUBFACING))
        .unwrap_or_else(|| "unknown".to_owned());
    let time = entity
        .level(&*host)
        .and_then(|level| level.game_time(host))
        .unwrap_or(0);
    format!(
        "Mob Slaughter Factory at {pos:?}: {}/{} FE, tank {} {}/{MEAT_TANK_MB} mB, capability {cap_meat} {cap_meat_mb} mB, tank {} {slime}/{PINK_SLIME_TANK_MB} mB, capability {cap_slime} {cap_slime_mb} mB, progress {progress}/{}, victims {victims}, facing {facing}, augments speed {} efficiency {:.2}, t={time}",
        read_int(&data, KEY_ENERGY),
        CAPACITY_FE,
        if meat > 0 { MEAT } else { "empty" },
        meat,
        if slime > 0 { PINK_SLIME } else { "empty" },
        Factory::bar_length(augments),
        augments.speed,
        augments.efficiency,
    )
}

/// How many mobs the machine's working area holds right now — the counterfactual's own read.
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
    entities
        .into_iter()
        .filter(|entity| host.mob(*entity).is_some())
        .filter(|entity| {
            host.living(*entity)
                .and_then(|living| living.alive(&*host))
                .unwrap_or(false)
        })
        .count()
}

/// Both tanks as the declared fluid capability reports them, in declaration order.
fn capability_tanks(host: &mut dyn ServerHost, pos: [i32; 3]) -> [(String, i32); 2] {
    let none = ("none".to_owned(), 0);
    let Some(level) = host.level(host.dimension()) else {
        return [none.clone(), none];
    };
    let Some(tanks) = level.capability(host, &FLUIDS, pos, Some(READ_FACE)) else {
        return [none.clone(), none];
    };
    let read = |host: &mut dyn ServerHost, index: usize| -> (String, i32) {
        let Some(resource) = tanks.resource(host, index) else {
            return ("empty".to_owned(), 0);
        };
        let name = match resource.fluid() {
            FluidIdentity::Resolved(fluid) => fluid.id(host),
            FluidIdentity::Unresolved(fluid) => fluid.id(host),
            _ => "unknown".to_owned(),
        };
        let amount = i32::try_from(tanks.amount(host, index)).unwrap_or(i32::MAX);
        (name, amount)
    };
    let first = read(host, 0);
    let second = read(host, 1);
    [first, second]
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
    fn the_factory_declares_two_eight_thousand_mb_tanks() {
        let tanks = tanks();
        assert_eq!(tanks.tanks, vec![MEAT_TANK_MB, PINK_SLIME_TANK_MB]);
        assert!(
            tanks.inputs.is_empty(),
            "both are Action.DRAIN: no face may fill them"
        );
        assert_eq!(tanks.outputs, vec![0, 1]);
        assert_eq!(tanks.faces.len(), 6);
        for face in &tanks.faces {
            assert_eq!(face, &vec![0, 1], "every face reaches both tanks");
        }
    }

    #[test]
    fn the_tanks_round_trip_through_the_saved_compound() {
        let mut nbt = Compound::new();
        assert_eq!(read_tanks(&nbt), [0, 0]);
        write_tanks(&mut nbt, [10, 200]);
        assert_eq!(read_tanks(&nbt), [10, 200]);
        let Some(Tag::List(entries)) = nbt.get(&"fluids".into()) else {
            panic!("no fluids list");
        };
        assert_eq!(entries.len(), 2, "one entry per tank, meat first");
        write_tanks(&mut nbt, [0, 0]);
        assert_eq!(read_tanks(&nbt), [0, 0]);
    }

    #[test]
    fn a_fluid_in_the_wrong_tank_reads_as_empty() {
        let mut nbt = Compound::new();
        let mut entry = Compound::new();
        entry.insert("Fluid".into(), Tag::String("minecraft:water".into()));
        entry.insert("Amount".into(), Tag::Int(500));
        nbt.insert("fluids".into(), Tag::List(vec![Tag::Compound(entry)]));
        assert_eq!(
            read_tanks(&nbt),
            [0, 0],
            "the tank's validator only takes its own fluid"
        );
    }

    #[test]
    fn the_working_area_is_the_block_behind_the_machine() {
        assert_eq!(
            behind("west"),
            Some([1, 0, 0]),
            "behind a west-facing machine"
        );
        assert_eq!(behind("east"), Some([-1, 0, 0]));
        assert_eq!(behind("north"), Some([0, 0, 1]));
        assert_eq!(behind("south"), Some([0, 0, -1]));
        assert_eq!(behind("up"), None, "FOUR_WAY has no vertical facing");
    }

    #[test]
    fn an_animal_is_a_creature_or_an_axolotl() {
        assert!(animal_category(Some(MobCategory::Creature)));
        assert!(animal_category(Some(MobCategory::Axolotls)));
        assert!(!animal_category(Some(MobCategory::Monster)));
        assert!(!animal_category(Some(MobCategory::Misc)));
        assert!(!animal_category(None));
    }

    #[test]
    fn a_broken_factory_carries_both_tanks_and_its_buffer() {
        let mut nbt = Compound::new();
        assert!(!has_contents(&nbt), "a fresh factory carries nothing");
        write_tanks(&mut nbt, [10, 200]);
        assert!(has_contents(&nbt));
        write_int(&mut nbt, KEY_ENERGY, 4_600);
        assert!(has_contents(&nbt));
        let patch = carried_patch(&nbt);
        let Some(Tag::Compound(payload)) = patch.get(&"minecraft:block_entity_data".into()) else {
            panic!("no block entity data in the patch");
        };
        assert_eq!(
            payload.get(&"id".into()),
            Some(&Tag::String(ENTITY_ID.into()))
        );
        assert_eq!(read_tanks(payload), [10, 200]);
        assert_eq!(read_int(payload, KEY_ENERGY), 4_600);
    }

    #[test]
    fn a_placed_factory_clamps_what_its_item_carried() {
        let mut over = Compound::new();
        write_int(&mut over, KEY_ENERGY, 50_000);
        write_int(&mut over, KEY_PROGRESS, 4_000);
        let mut entry = Compound::new();
        entry.insert("Fluid".into(), Tag::String(MEAT.into()));
        entry.insert("Amount".into(), Tag::Int(999_999));
        let mut second = Compound::new();
        second.insert("Fluid".into(), Tag::String(PINK_SLIME.into()));
        second.insert("Amount".into(), Tag::Int(-5));
        over.insert(
            "fluids".into(),
            Tag::List(vec![Tag::Compound(entry), Tag::Compound(second)]),
        );
        let patch = clamp_patch(&over).expect("out-of-range values are corrected");
        assert_eq!(read_int(&patch, KEY_ENERGY), CAPACITY_FE);
        assert_eq!(
            read_int(&patch, KEY_PROGRESS),
            crate::slaughter::MAX_PROGRESS
        );
        assert_eq!(read_tanks(&patch), [MEAT_TANK_MB, 0]);
        assert!(
            clamp_patch(&Compound::new()).is_none(),
            "a fresh factory is left alone"
        );
    }

    #[test]
    fn the_constants_are_the_config_s_own() {
        assert_eq!(crate::slaughter::POWER_PER_OPERATION, 400);
        assert_eq!(CAPACITY_FE, 10_000);
        assert_eq!(MEAT_TANK_MB, 8_000);
        assert_eq!(PINK_SLIME_TANK_MB, 8_000);
        assert_eq!(AREA_UP, 2);
    }
}
