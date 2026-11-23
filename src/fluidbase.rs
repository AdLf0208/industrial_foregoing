//! The game side of the **fluid laser base** and its own recipe type,
//! `industrialforegoing:laser_drill_fluid`.
//!
//! Upstream's `FluidLaserBaseTile` is an `IndustrialMachineTile` with a 20-tick bar that
//! only a laser drill advances (the drill's `isValidTarget` is any `ILaserBase`, so the
//! drill of the previous slice charges this base too), a single lens slot, a 32,000 mB
//! drain-only output tank and **no FE of its own**. At the top of the bar `onWork()` runs
//! the first `laser_drill_fluid` recipe whose catalyst the lens slot holds, whose first
//! valid rarity answers the base's `miningDepth`, biome tags and dimension type, and whose
//! optional nearby-entity gate passes; it adds the recipe's amount to the tank and takes 5
//! health from the matching entity. The arithmetic is [`crate::fluidlaser`].
//!
//! # The tank is the entity's own `fluids` compound
//!
//! A mod fluid cannot be inserted through the pin's tank capability (`native_fluid_insert`
//! takes water and lava only), so the machine writes its own one-entry `fluids` list and
//! `Registrar::block_fluids` declares it as a real drain-only handler; the readout reads
//! the tank **through that capability**, which is what makes it a handler and not a
//! private counter. `industrialforegoing:ether_gas` is a declared fluid key (slice 5).
//!
//! # The recipe type
//!
//! `industrialforegoing:laser_drill_fluid` is declared with
//! `Registrar::register_recipe_type` and kept by `Registrar::register_raw_recipe_serializer`
//! — API 36's machine route, exactly as the chamber's, the extractor's and the ore base's.
//! The table is decoded once per table identity, so `/reload` rebuilds it. A recipe whose
//! output fluid the game does not know (the jar's `pneumaticcraft:oil` without that mod) is
//! left out of the table and counted on the `files` side of `recipes n/m`.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

use mod_api::{
    self, BlockEntityPart, BlockEntityTickEvent, BlockEntityTicksEvent, BlockPart, Compound,
    Direction, FLUIDS, Host, HostTransaction, ItemKey, ItemPart, ModContainer, ModFluidTanks,
    OnBlockPlacedByEvent, Registrar, ServerHost, Tag,
};

use crate::fluidlaser::{
    Base, CATALYST_SLOTS, ENTITY_DAMAGE, ENTITY_MIN_HEALTH, EntityGate, FluidRecipe,
    TANK_CAPACITY_MB, Tank, choice,
};

/// The fluid laser base (`FluidLaserBaseBlock`).
pub const BLOCK: &str = "industrialforegoing:fluid_laser_base";
/// Its block entity type, a bare name so the mod's namespace qualifies it.
pub const ENTITY: &str = "fluid_laser_base";
/// The base's own recipe type (`ModuleCore.LASER_DRILL_FLUID_TYPE`).
pub const RECIPE_TYPE: &str = "industrialforegoing:laser_drill_fluid";
/// The saved keys.
const KEY_PROGRESS: &str = "progress";
const KEY_DEPTH: &str = "depth";
/// The one lens slot.
pub const SLOTS: usize = CATALYST_SLOTS;
/// The side the tank is read through (every face reaches it).
const READ_FACE: Direction = Direction::Up;

/// The block entity type and the item, resolved once in `register`: `block_drops` and the
/// command path run with a read-only host, which cannot name either.
static KIND: OnceLock<mod_api::BlockEntityTypeKey> = OnceLock::new();
static ITEM: OnceLock<ItemKey> = OnceLock::new();

/// The decoded fluid recipes and the identity of the table they came from.
#[derive(Debug)]
pub(crate) struct Table {
    /// `(id, the file's own allocation)`: the table's identity, so a `/reload` that changed
    /// it rebuilds the decode and every tick in between reads this instead of the game.
    fingerprint: Vec<(String, usize)>,
    /// The decoded rules, in file order.
    pub(crate) rules: Vec<FluidRecipe>,
}

/// The recipe table, as last decoded.
static TABLE: Mutex<Option<Arc<Table>>> = Mutex::new(None);

// ---------------------------------------------------------------------------
// The declarations
// ---------------------------------------------------------------------------

/// Declare the machine: its block entity type, the 32,000 mB drain-only tank, the one lens
/// slot and the Part that ticks it. The recipe type and its read-only serializer are
/// declared beside the other three in [`crate::Mod::register`].
pub(crate) fn declare(r: &mut dyn Registrar) {
    let block = r.block_key(BLOCK);
    let item = r.item_key(BLOCK);
    let _ = ITEM.set(item);
    r.item_part(item, FluidBaseItem);
    let kind = r.block_entity_type(ENTITY).blocks(&[ENTITY]).build();
    let _ = KIND.set(kind);
    r.block_entity_part(&kind, FluidBasePart)
        .expect("the fluid base's Part attaches to the type this mod just declared");
    r.block_fluids(&block, tank());
    r.block_container(&block, container());
}

/// `SidedFluidTankComponent("output", 32000, …, Action.DRAIN)`: every face reaches the one
/// tank, no face may fill it, and only tank 0 is drainable.
#[must_use]
pub fn tank() -> ModFluidTanks {
    let all = vec![0usize];
    let mut tanks = ModFluidTanks::default();
    tanks.tanks = vec![TANK_CAPACITY_MB];
    tanks.faces = std::array::from_fn(|_| all.clone());
    tanks.outputs = vec![0];
    tanks
}

/// `SidedInventoryComponent("lens", …, 1, 0)` with every face mode `NONE`: one lens slot the
/// machine reads, unreachable from any face.
#[must_use]
pub fn container() -> ModContainer {
    ModContainer {
        slots: SLOTS,
        faces: std::array::from_fn(|_| Vec::new()),
        inputs: vec![0],
        outputs: Vec::new(),
        menu: None,
    }
}

// ---------------------------------------------------------------------------
// The recipe table
// ---------------------------------------------------------------------------

/// The fluid recipes in force, decoded once per table.
pub(crate) fn recipes(host: &mut dyn ServerHost) -> Arc<Table> {
    let listed = host.recipes_of_type(RECIPE_TYPE);
    let fingerprint: Vec<(String, usize)> = listed
        .iter()
        .map(|recipe| (recipe.id.clone(), Arc::as_ptr(&recipe.json) as usize))
        .collect();
    if let Ok(cached) = TABLE.lock()
        && let Some(table) = cached.as_ref()
        && table.fingerprint == fingerprint
    {
        return Arc::clone(table);
    }
    let mut rules = Vec::new();
    for recipe in listed.iter() {
        if let Some(rule) = one(&*host, &recipe.id, &recipe.json) {
            rules.push(rule);
        }
    }
    let table = Arc::new(Table { fingerprint, rules });
    if let Ok(mut cached) = TABLE.lock() {
        *cached = Some(Arc::clone(&table));
    }
    table
}

/// One recipe file (`LaserDrillFluidRecipe.CODEC`): a sized fluid `output`, the `catalyst`,
/// an optional `entity_data` gate and the `rarity` list. A file whose fluid the game does not
/// know could never fill this tank, so it is left out — the same outcome as the game dropping
/// a file it cannot read.
fn one(host: &dyn Host, id: &str, value: &mod_api::serde_json::Value) -> Option<FluidRecipe> {
    use mod_api::serde_json::Value;

    let output = value.get("output")?;
    let fluid = output
        .get("fluid")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| {
            output
                .get("fluids")?
                .as_array()?
                .first()?
                .as_str()
                .map(str::to_owned)
        })?;
    host.fluid_key(&fluid)?;
    let amount = output
        .get("amount")
        .and_then(Value::as_i64)
        .and_then(|amount| i32::try_from(amount).ok())
        .unwrap_or(0)
        .max(0);
    let catalyst = crate::laserdrill::ingredient(host, value.get("catalyst")?)?;
    let entity = value.get("entity_data").and_then(entity_gate);
    let rarities = value.get("rarity")?.as_array()?;
    let mut decoded = Vec::new();
    for rarity in rarities {
        decoded.push(crate::laserdrill::rarity_of(rarity)?);
    }
    if decoded.is_empty() {
        return None;
    }
    Some(FluidRecipe {
        id: id.to_owned(),
        fluid,
        amount,
        catalyst,
        entity,
        rarities: decoded,
    })
}

/// The optional `entity_data` gate: `entity.type` and the non-empty `data` object.
fn entity_gate(value: &mod_api::serde_json::Value) -> Option<EntityGate> {
    let entity = value.get("entity")?.get("type")?.as_str()?.to_owned();
    let data = value
        .get("data")
        .filter(|data| data.as_object().is_some_and(|data| !data.is_empty()))
        .and_then(|data| mod_api::serde_json::to_string(data).ok());
    Some(EntityGate { entity, data })
}

// ---------------------------------------------------------------------------
// The Part
// ---------------------------------------------------------------------------

/// The fluid base: its tick and its efficiency-only augment seat.
pub(crate) struct FluidBasePart;

#[mod_api::horizon_part]
impl BlockPart for FluidBasePart {
    /// `canAcceptAugment`: the base reads only an efficiency addon — its bar is advanced by
    /// the drill, so a speed addon has nothing to move.
    fn use_item_on(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::UseItemOnEvent<'_>,
    ) -> Option<mod_api::InteractionResult> {
        let stack = event.click.stack?;
        let component = crate::augment_component(&*host, stack)?;
        let augment = crate::decode_augment(&component)?;
        if augment.kind != crate::runtime::AugmentType::Efficiency {
            return None;
        }
        crate::seat_from_click(host, event)
    }
}

#[mod_api::horizon_part]
impl BlockEntityPart for FluidBasePart {
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    /// One server tick: take the advances the drills paid for, and at the top of the bar run
    /// the first matching fluid recipe into the tank.
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let dimension = host.dimension();
        let entity = &mut *event.entity;
        if entity.items.len() < SLOTS {
            return;
        }
        let augments = crate::read_augments(&entity.nbt);
        let depth = crate::laserdrill::read_depth(&entity.nbt, pos);
        if !entity.nbt.contains_key(&KEY_DEPTH.into()) {
            write_int(&mut entity.nbt, KEY_DEPTH, depth);
        }
        let mut base = Base::from_saved(read_int(&entity.nbt, KEY_PROGRESS));
        let mut completed = false;
        for _ in 0..crate::laserdrill::take_advances(dimension, pos) {
            if base.advance(augments) {
                completed = true;
            }
        }
        write_int(&mut entity.nbt, KEY_PROGRESS, base.progress);
        if completed {
            let lens = entity
                .items
                .first()
                .and_then(|slot| slot.as_ref())
                .map(|stack| stack.item().id(&*host));
            let mut stored = read_tank(&entity.nbt);
            if let Some(lens) = lens
                && on_work(host, &mut stored, pos, depth, &lens)
            {
                write_tank(&mut entity.nbt, &stored);
            }
        }
    }
}

/// One completed bar: the first matching recipe's fluid goes into the tank
/// (`onWork`). The entity gate is applied (the matching entity takes
/// [`ENTITY_DAMAGE`]).
fn on_work(
    host: &mut dyn ServerHost,
    stored: &mut Tank,
    pos: [i32; 3],
    depth: i32,
    lens: &str,
) -> bool {
    let table = recipes(&mut *host);
    let dimension = host.dimension();
    let Some(level) = host.level(dimension) else {
        return false;
    };
    let biome = level.biome_at(&*host, pos).unwrap_or_default();
    let gates: BTreeMap<String, bool> = table
        .rules
        .iter()
        .map(|recipe| {
            let passes = recipe
                .entity
                .as_ref()
                .is_none_or(|gate| entity_matches(host, pos, gate, true));
            (recipe.id.clone(), passes)
        })
        .collect();
    let in_tag = |tag: &str| level.biome_in_tag(&*host, &biome, tag).unwrap_or(false);
    let Some(chosen) = choice(&table.rules, dimension, depth, &in_tag, lens, &|recipe| {
        gates.get(&recipe.id).copied().unwrap_or(false)
    }) else {
        return false;
    };
    let (fluid, amount) = (chosen.fluid.clone(), chosen.amount);
    stored.fill(&fluid, amount);
    true
}

/// Whether a living entity of the gate's kind (and, when the gate names data, carrying those
/// values) stands in the base's 3×3×3 box, from one below to two above (`onWork`'s
/// `Shapes.box(-1, 0, -1, 2, 3, 2)` moved to `y - 1`). `damage` is whether the match takes
/// [`ENTITY_DAMAGE`] health, as `onWork` does; a readout passes `false`.
fn entity_matches(
    host: &mut dyn ServerHost,
    pos: [i32; 3],
    gate: &EntityGate,
    damage: bool,
) -> bool {
    let Some(level) = host.level(host.dimension()) else {
        return false;
    };
    let min = [
        f64::from(pos[0]) - 1.0,
        f64::from(pos[1]) - 1.0,
        f64::from(pos[2]) - 1.0,
    ];
    let max = [
        f64::from(pos[0]) + 2.0,
        f64::from(pos[1]) + 2.0,
        f64::from(pos[2]) + 2.0,
    ];
    let Some(entities) = level.entities_in(&*host, min, max) else {
        return false;
    };
    for entity in entities {
        let Some(living) = host.living(entity) else {
            continue;
        };
        if living.health(&*host).unwrap_or(0.0) <= ENTITY_MIN_HEALTH {
            continue;
        }
        if entity.kind(&*host).map(|kind| kind.id(&*host)).as_deref() != Some(gate.entity.as_str())
        {
            continue;
        }
        if let Some(data) = &gate.data {
            let Some(saved) = entity.nbt(host) else {
                continue;
            };
            if !crate::fluidlaser::data_matches(data, &saved) {
                continue;
            }
        }
        if damage {
            let _ = entity.hurt(host, "minecraft:generic", ENTITY_DAMAGE, None, None);
        }
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// The item
// ---------------------------------------------------------------------------

/// The base's item: a broken machine's drop carries its bar and tank, and placing it clamps
/// them back to the capacities (slice 2's route).
pub(crate) struct FluidBaseItem;

#[mod_api::horizon_part]
impl ItemPart for FluidBaseItem {
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

/// The one tank from the entity's `fluids` (the pin's shape: one `{Fluid, Amount}`).
fn read_tank(nbt: &Compound) -> Tank {
    let Some(Tag::List(entries)) = nbt.get(&"fluids".into()) else {
        return Tank::empty();
    };
    let Some(Tag::Compound(entry)) = entries.first() else {
        return Tank::empty();
    };
    let fluid = match entry.get(&"Fluid".into()) {
        Some(Tag::String(fluid)) if fluid.to_string_lossy() != "minecraft:empty" => {
            fluid.to_string_lossy()
        }
        _ => return Tank::empty(),
    };
    let mb = match entry.get(&"Amount".into()) {
        Some(Tag::Int(amount)) => *amount,
        _ => 0,
    };
    Tank {
        fluid,
        mb: mb.clamp(0, TANK_CAPACITY_MB),
    }
}

/// The tank back in the pin's shape.
fn write_tank(nbt: &mut Compound, tank: &Tank) {
    let mut entry = Compound::new();
    entry.insert(
        "Fluid".into(),
        Tag::String(if tank.is_empty() {
            "minecraft:empty".into()
        } else {
            tank.fluid.as_str().into()
        }),
    );
    entry.insert(
        "Amount".into(),
        Tag::Int(tank.mb.clamp(0, TANK_CAPACITY_MB)),
    );
    nbt.insert("fluids".into(), Tag::List(vec![Tag::Compound(entry)]));
}

/// Whether the base holds anything worth carrying on its item.
pub(crate) fn has_contents(saved: &Compound) -> bool {
    // The engine drops a mod container's lens slot itself, so the bar, the tank and the
    // augments are what the item carries (carrying `Items` too would duplicate the lens).
    !read_tank(saved).is_empty()
        || read_int(saved, KEY_PROGRESS) > 0
        || crate::has_seated_augment(saved)
}

/// The `minecraft:block_entity_data` payload a broken base's item carries.
pub(crate) fn carried_patch(saved: &Compound) -> Compound {
    let mut payload = saved.clone();
    payload.remove(&"Items".into());
    payload.insert("id".into(), Tag::String(BLOCK.into()));
    let mut patch = Compound::new();
    patch.insert("minecraft:block_entity_data".into(), Tag::Compound(payload));
    patch
}

/// The corrections a placed base needs when its item carried values outside the capacities.
#[must_use]
pub(crate) fn clamp_patch(saved: &Compound) -> Option<Compound> {
    let mut patch = Compound::new();
    let mut changed = false;
    let progress = read_int(saved, KEY_PROGRESS);
    if !(0..=crate::fluidlaser::BASE_MAX_PROGRESS).contains(&progress) {
        write_int(
            &mut patch,
            KEY_PROGRESS,
            progress.clamp(0, crate::fluidlaser::BASE_MAX_PROGRESS),
        );
        changed = true;
    }
    if let Some(Tag::List(entries)) = saved.get(&"fluids".into()) {
        let amount = match entries.first() {
            Some(Tag::Compound(entry)) => match entry.get(&"Amount".into()) {
                Some(Tag::Int(amount)) => *amount,
                _ => 0,
            },
            _ => 0,
        };
        if amount > TANK_CAPACITY_MB {
            let tank = read_tank(saved);
            let mut fixed = Compound::new();
            write_tank(&mut fixed, &tank);
            if let Some(Tag::List(entries)) = fixed.get(&"fluids".into()) {
                patch.insert("fluids".into(), Tag::List(entries.clone()));
                changed = true;
            }
        }
    }
    changed.then_some(patch)
}

// ---------------------------------------------------------------------------
// The commands
// ---------------------------------------------------------------------------

/// `/industrialforegoing progress`/`depth` on the base: one int of its saved compound.
pub(crate) fn set_int(host: &mut dyn ServerHost, pos: [i32; 3], key: &str, value: i32) -> bool {
    let Some(entity) = crate::our_entity(host, pos, BLOCK) else {
        return false;
    };
    let mut patch = Compound::new();
    write_int(&mut patch, key, value);
    entity.merge_data(host, patch).unwrap_or(false)
}

/// `/industrialforegoing depth`: the base's `miningDepth`.
pub(crate) fn set_depth(host: &mut dyn ServerHost, pos: [i32; 3], value: i32) -> bool {
    set_int(host, pos, KEY_DEPTH, value)
}

/// `/industrialforegoing drain`: take the tank out through the declared capability, which is
/// what makes it a handler rather than a private counter.
pub(crate) fn drain(host: &mut dyn ServerHost, pos: [i32; 3], amount: i32) {
    let want = u32::try_from(amount.max(0)).unwrap_or(u32::MAX);
    let answer = host
        .transaction::<String, ()>(|host, tx| {
            let Some(level) = host.level(host.dimension()) else {
                return Ok(format!("Fluid Laser Base at {pos:?}: no drainable fluid."));
            };
            let Some(tanks) = level.capability(host, &FLUIDS, pos, Some(READ_FACE)) else {
                return Ok(format!("Fluid Laser Base at {pos:?}: no drainable fluid."));
            };
            let Some(resource) = tanks.resource(host, 0) else {
                return Ok(format!("Fluid Laser Base at {pos:?}: no drainable fluid."));
            };
            let taken = tanks.extract(host, tx, 0, &resource, want);
            if taken == 0 {
                return Ok(format!("Fluid Laser Base at {pos:?}: no drainable fluid."));
            }
            let name = match resource.fluid() {
                mod_api::FluidIdentity::Resolved(fluid) => fluid.id(host),
                mod_api::FluidIdentity::Unresolved(fluid) => fluid.id(host),
                _ => "an unknown fluid".to_owned(),
            };
            Ok(format!(
                "Fluid Laser Base at {pos:?}: drained {taken} mB of {name}."
            ))
        })
        .unwrap_or_else(|()| format!("{pos:?} is not a fluid laser base."));
    crate::say(host, &answer);
}

/// `/industrialforegoing info` for the base: the tank from its own compound and from the
/// declared capability, its bar, depth, biome, dimension, the recipe its lens/catalyst,
/// rarity and entity gate resolve, the table's size, the lens, augments and game time.
pub(crate) fn readout(host: &mut dyn ServerHost, pos: [i32; 3]) -> String {
    let Some(entity) = crate::our_entity(host, pos, BLOCK) else {
        return format!("{pos:?} is not a fluid laser base.");
    };
    let Some(data) = entity.data(&*host) else {
        return format!("{pos:?} is not a mod block entity.");
    };
    let base = Base::from_saved(read_int(&data, KEY_PROGRESS));
    let augments = crate::read_augments(&data);
    let depth = crate::laserdrill::read_depth(&data, pos);
    let stored = read_tank(&data);
    let (capability_fluid, capability_mb) = capability_tank(host, pos);
    let table = recipes(&mut *host);
    let dimension = host.dimension();
    let Some(level) = host.level(dimension) else {
        return format!("{pos:?} is not a fluid laser base.");
    };
    let biome = level
        .biome_at(&*host, pos)
        .unwrap_or_else(|| "unknown".to_owned());
    let lens = entity
        .item_slot(&*host, 0)
        .ok()
        .flatten()
        .map(|stack| stack.item().id(&*host));
    let recipe = lens.as_deref().and_then(|lens| {
        let gates: BTreeMap<String, bool> = table
            .rules
            .iter()
            .map(|recipe| {
                let passes = recipe
                    .entity
                    .as_ref()
                    .is_none_or(|gate| entity_matches(host, pos, gate, false));
                (recipe.id.clone(), passes)
            })
            .collect();
        let in_tag = |tag: &str| level.biome_in_tag(&*host, &biome, tag).unwrap_or(false);
        choice(&table.rules, dimension, depth, &in_tag, lens, &|recipe| {
            gates.get(&recipe.id).copied().unwrap_or(false)
        })
        .map(|recipe| recipe.id.clone())
    });
    let time = level.game_time(host).unwrap_or(0);
    format!(
        "Fluid Laser Base at {pos:?}: tank {} {}/{} mB, capability {capability_fluid} {capability_mb} mB, progress {}/{}, depth {depth}, biome {biome}, dimension {dimension}, recipe {}, recipes {}/{}, lens {}, augments speed {} efficiency {:.2}, t={time}",
        if stored.is_empty() {
            "empty"
        } else {
            stored.fluid.as_str()
        },
        stored.mb,
        TANK_CAPACITY_MB,
        base.progress,
        Base::bar_length(augments),
        recipe.unwrap_or_else(|| "none".to_owned()),
        table.rules.len(),
        table.fingerprint.len(),
        lens.unwrap_or_else(|| "empty".to_owned()),
        augments.speed,
        augments.efficiency,
    )
}

/// The one tank as the declared fluid capability reports it.
fn capability_tank(host: &mut dyn ServerHost, pos: [i32; 3]) -> (String, i32) {
    let Some(level) = host.level(host.dimension()) else {
        return ("none".to_owned(), 0);
    };
    let Some(tanks) = level.capability(host, &FLUIDS, pos, Some(READ_FACE)) else {
        return ("none".to_owned(), 0);
    };
    let Some(resource) = tanks.resource(host, 0) else {
        return ("empty".to_owned(), 0);
    };
    let name = match resource.fluid() {
        mod_api::FluidIdentity::Resolved(fluid) => fluid.id(host),
        mod_api::FluidIdentity::Unresolved(fluid) => fluid.id(host),
        _ => "unknown".to_owned(),
    };
    let amount = i32::try_from(tanks.amount(host, 0)).unwrap_or(i32::MAX);
    (name, amount)
}

/// The block entity type the declaration made, for the augment seat.
pub(crate) fn kind_of(entity: mod_api::BlockEntityRef, host: &dyn Host) -> bool {
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
    fn the_base_declares_one_lens_slot_reachable_from_no_face() {
        let container = container();
        assert_eq!(container.slots, 1);
        assert_eq!(container.inputs, vec![0]);
        assert!(container.outputs.is_empty());
        for face in &container.faces {
            assert!(face.is_empty(), "upstream's lens inventory is all NONE");
        }
    }

    #[test]
    fn the_tank_is_drain_only_and_thirty_two_thousand_mb() {
        let tanks = tank();
        assert_eq!(tanks.tanks, vec![TANK_CAPACITY_MB]);
        assert_eq!(tanks.outputs, vec![0]);
        assert_eq!(tanks.faces.len(), 6);
        for face in &tanks.faces {
            assert_eq!(face, &vec![0]);
        }
    }

    #[test]
    fn the_tank_round_trips_through_the_saved_compound() {
        let mut nbt = Compound::new();
        assert!(read_tank(&nbt).is_empty());
        write_tank(
            &mut nbt,
            &Tank {
                fluid: "industrialforegoing:ether_gas".to_owned(),
                mb: 10,
            },
        );
        let tank = read_tank(&nbt);
        assert_eq!(tank.fluid, "industrialforegoing:ether_gas");
        assert_eq!(tank.mb, 10);
        assert!(!tank.is_empty());
        assert!(nbt.contains_key(&"fluids".into()));
    }

    #[test]
    fn a_broken_base_carries_its_tank_and_not_its_lens() {
        let mut nbt = Compound::new();
        assert!(!has_contents(&nbt));
        write_tank(
            &mut nbt,
            &Tank {
                fluid: "industrialforegoing:ether_gas".to_owned(),
                mb: 10,
            },
        );
        assert!(has_contents(&nbt));
        let mut item = Compound::new();
        item.insert("Slot".into(), Tag::Byte(0));
        item.insert(
            "id".into(),
            Tag::String("industrialforegoing:purple_laser_lens".into()),
        );
        item.insert("count".into(), Tag::Int(1));
        nbt.insert("Items".into(), Tag::List(vec![Tag::Compound(item)]));
        let patch = carried_patch(&nbt);
        let Some(Tag::Compound(payload)) = patch.get(&"minecraft:block_entity_data".into()) else {
            panic!("no block entity data in the patch");
        };
        assert_eq!(payload.get(&"id".into()), Some(&Tag::String(BLOCK.into())));
        assert!(
            payload.get(&"Items".into()).is_none(),
            "the lens is the engine's own drop; carrying it would duplicate it"
        );
    }

    #[test]
    fn a_placed_base_clamps_what_its_item_carried() {
        let mut over = Compound::new();
        let mut entry = Compound::new();
        entry.insert(
            "Fluid".into(),
            Tag::String("industrialforegoing:ether_gas".into()),
        );
        entry.insert("Amount".into(), Tag::Int(TANK_CAPACITY_MB + 500));
        over.insert("fluids".into(), Tag::List(vec![Tag::Compound(entry)]));
        let patch = clamp_patch(&over).expect("an over-full tank is clamped");
        assert_eq!(read_tank(&patch).mb, TANK_CAPACITY_MB);
        let mut bar = Compound::new();
        write_int(&mut bar, KEY_PROGRESS, 99);
        let patch = clamp_patch(&bar).expect("an over-long bar is clamped");
        assert_eq!(
            read_int(&patch, KEY_PROGRESS),
            crate::fluidlaser::BASE_MAX_PROGRESS
        );
        assert!(clamp_patch(&Compound::new()).is_none());
    }
}
