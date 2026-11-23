//! The game side of the laser drill: the **laser drill** and the **ore laser base**
//! (`ModuleResourceProduction.LASER_DRILL` and `ORE_LASER_BASE`).
//!
//! Upstream's `LaserDrillTile` is an `IndustrialAreaWorkingTile(BEHIND)` — a 50-tick bar
//! counting down, one laser base charged per completed bar for 1,000 FE — and
//! `OreLaserBaseTile` is an `IndustrialMachineTile` whose 100-tick bar is advanced only by
//! the drill and, at the top, picks one weighted `laser_drill_ore` recipe and puts its
//! output in the fifteen output slots. The arithmetic is [`crate::laser`]; this file reads
//! and writes the game.
//!
//! # The drill's advance, queued rather than written across entities
//!
//! The drill's tick must advance **another** block entity's bar. A block-entity write made
//! during a tick is overwritten by that entity's own checked write-back (HANDOVER §9d.10), so
//! the drill does not write the base: it queues one advance in a static map keyed by the
//! base's position and dimension, and the base's own tick takes it. Every write to the base
//! still happens inside the base's tick, which is the rule the augment seating already
//! follows.
//!
//! # The recipe type
//!
//! `industrialforegoing:laser_drill_ore` is declared with `Registrar::register_recipe_type`
//! and kept by `Registrar::register_raw_recipe_serializer` — API 36's machine route, exactly
//! as the dissolution chamber and the fluid extractor are. That pair is what an add-on's own
//! `laser_drill_ore` files load under (`industrialforegoingsouls` is blocked on it), and the
//! table is decoded once per table identity, so `/reload` rebuilds it.
//!
//! # What the filter reads
//!
//! `LaserDrillRarity` filters by the level's **dimension type id** (`builtin:the_end` …),
//! its **biome tags** (`minecraft:is_mountain`) and the base's `miningDepth` (its own Y until
//! the GUI changes it). The pin exposes all three: `LevelRef::dimension_type`,
//! `LevelRef::biome_at` and `LevelRef::biome_in_tag`. A recipe with `entity_data` needs a
//! nearby entity of that kind, which this machine slice does not port; such a file is left
//! out of the table and the readout counts it in the `files` side of `recipes n/m`.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

use mod_api::{
    self, BlockEntityPart, BlockEntityTickEvent, BlockEntityTicksEvent, BlockPart, BlockState,
    Compound, Host, InteractionResult, ItemKey, ItemPart, ItemStack, LevelHost, ModContainer,
    ModEnergyBuffer, OnBlockPlacedByEvent, RandomSource, Registrar, ServerHost,
    StateForPlacementEvent, Tag, UseItemOnEvent,
};

use crate::laser::{
    Base, DRILL_CAPACITY_FE, DRILL_MAX_PROGRESS, Drill, DrillStep, Filter, LENS_SLOTS,
    OUTPUT_SLOTS, OreRecipe, Rarity, candidates, pick,
};

/// The laser drill (`LaserDrillBlock`).
pub const DRILL_BLOCK: &str = "industrialforegoing:laser_drill";
/// Its block entity type, a bare name so the mod's namespace qualifies it.
pub const DRILL_ENTITY: &str = "laser_drill";
/// The ore laser base (`OreLaserBaseBlock`).
pub const BASE_BLOCK: &str = "industrialforegoing:ore_laser_base";
/// Its block entity type.
pub const BASE_ENTITY: &str = "ore_laser_base";
/// The ore base's own recipe type (`ModuleCore.LASER_DRILL_TYPE`).
pub const RECIPE_TYPE: &str = "industrialforegoing:laser_drill_ore";
/// `IndustrialBlock.RotationType.SIX_WAY`'s state property.
pub const FACING: &str = "facing";
/// The saved keys.
const KEY_ENERGY: &str = "energy";
const KEY_PROGRESS: &str = "progress";
const KEY_DEPTH: &str = "depth";
/// The container's total slot count: six lens slots and fifteen output slots.
pub const BASE_SLOTS: usize = LENS_SLOTS + OUTPUT_SLOTS;

/// The block entity types and the items, resolved once in `register`: `block_drops`, the
/// command path and the seat run with a read-only host, which cannot name either.
static DRILL_KIND: OnceLock<mod_api::BlockEntityTypeKey> = OnceLock::new();
static BASE_KIND: OnceLock<mod_api::BlockEntityTypeKey> = OnceLock::new();
static DRILL_ITEM: OnceLock<ItemKey> = OnceLock::new();
static BASE_ITEM: OnceLock<ItemKey> = OnceLock::new();

/// A base's position and the dimension it stands in.
type BaseKey = ([i32; 3], &'static str);

/// The advances the drills paid for, waiting for the base's own tick: one per charge.
static ADVANCES: Mutex<BTreeMap<BaseKey, i32>> = Mutex::new(BTreeMap::new());

/// The decoded ore recipes and the identity of the table they came from.
#[derive(Debug)]
pub(crate) struct Table {
    /// `(id, the file's own allocation)`: the table's identity, so a `/reload` that changed
    /// it rebuilds the decode and every tick in between reads this instead of the game.
    fingerprint: Vec<(String, usize)>,
    /// The decoded rules, in file order.
    pub(crate) rules: Vec<OreRecipe>,
}

/// The recipe table, as last decoded.
static TABLE: Mutex<Option<Arc<Table>>> = Mutex::new(None);

// ---------------------------------------------------------------------------
// The declarations
// ---------------------------------------------------------------------------

/// Declare both machines: their block entity types, the drill's FE buffer, the base's
/// twenty-one slots, and the Parts that tick and rotate them.
///
/// Called from [`crate::Mod::register`]; the recipe type and its read-only serializer are
/// declared there beside the chamber's and the extractor's.
pub(crate) fn declare(r: &mut dyn Registrar) {
    let block = r.block_key(DRILL_BLOCK);
    let item = r.item_key(DRILL_BLOCK);
    let _ = DRILL_ITEM.set(item);
    r.item_part(item, DrillItem);
    let kind = r
        .block_entity_type(DRILL_ENTITY)
        .blocks(&[DRILL_ENTITY])
        .build();
    let _ = DRILL_KIND.set(kind);
    r.block_entity_part(&kind, DrillPart)
        .expect("the drill's Part attaches to the type this mod just declared");
    r.block_energy(&block, battery());

    let block = r.block_key(BASE_BLOCK);
    let item = r.item_key(BASE_BLOCK);
    let _ = BASE_ITEM.set(item);
    r.item_part(item, BaseItem);
    let kind = r
        .block_entity_type(BASE_ENTITY)
        .blocks(&[BASE_ENTITY])
        .build();
    let _ = BASE_KIND.set(kind);
    r.block_entity_part(&kind, BasePart)
        .expect("the base's Part attaches to the type this mod just declared");
    r.block_container(&block, base_container());
}

/// `new EnergyStorageComponent<>(LaserDrillConfig.maxStoredPower, 10, 20)`: unlimited IO up
/// to the capacity.
#[must_use]
pub fn battery() -> ModEnergyBuffer {
    ModEnergyBuffer {
        capacity: DRILL_CAPACITY_FE,
        max_insert: DRILL_CAPACITY_FE,
        max_extract: DRILL_CAPACITY_FE,
        insert_faces: [true; 6],
        extract_faces: [true; 6],
    }
}

/// The base's one container: six lens slots (0–5, `SidedInventoryComponent("lens", …, 6, 0)`,
/// every face mode `NONE`) and fifteen output slots (6–20, output-only). Only the output
/// slots are reachable from any face, as upstream's lens inventory exposes none.
#[must_use]
pub fn base_container() -> ModContainer {
    let outputs: Vec<usize> = (LENS_SLOTS..BASE_SLOTS).collect();
    ModContainer {
        slots: BASE_SLOTS,
        faces: std::array::from_fn(|_| outputs.clone()),
        inputs: (0..LENS_SLOTS).collect(),
        outputs,
        menu: None,
    }
}

// ---------------------------------------------------------------------------
// The recipe table
// ---------------------------------------------------------------------------

/// The ore recipes in force, decoded once per table.
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

/// One recipe file (`LaserDrillOreRecipe.CODEC`): `output` (a sized item or `#tag`),
/// `catalyst`, the `rarity` list, and an optional `entity_data` this machines-only slice
/// does not match. A file whose output or catalyst resolves to nothing could never run, so
/// it is left out — the same outcome as the game dropping a recipe it cannot read.
fn one(host: &dyn Host, id: &str, value: &mod_api::serde_json::Value) -> Option<OreRecipe> {
    use mod_api::serde_json::Value;

    if value.get("entity_data").is_some() {
        return None;
    }
    let output = value.get("output")?;
    let count = output
        .get("count")
        .and_then(Value::as_i64)
        .and_then(|count| i32::try_from(count).ok())
        .unwrap_or(1)
        .max(0);
    let items = ingredient(host, output)?;
    let catalyst = ingredient(host, value.get("catalyst")?)?;
    let rarities = value.get("rarity")?.as_array()?;
    let mut decoded = Vec::new();
    for rarity in rarities {
        decoded.push(rarity_of(rarity)?);
    }
    if decoded.is_empty() {
        return None;
    }
    Some(OreRecipe {
        id: id.to_owned(),
        output: items,
        count,
        catalyst,
        rarities: decoded,
    })
}

/// One ingredient in the form `Host::recipe_ingredient` reads: a bare item id, a `#tag`, or
/// the file's own object. The jar writes `{"item": …}` / `{"tag": …}`, and the pin's parser
/// wants a string for those two shapes (it reads an object only for a NeoForge kind), so they
/// are flattened here; any other object is passed on as it is, for a custom ingredient type.
pub(crate) fn ingredient_spec(value: &mod_api::serde_json::Value) -> Option<String> {
    use mod_api::serde_json::Value;

    match value {
        Value::String(item) => Some(item.clone()),
        Value::Object(_) => {
            if let Some(Value::String(item)) = value.get("item") {
                return Some(item.clone());
            }
            if let Some(Value::String(tag)) = value.get("tag") {
                return Some(format!("#{tag}"));
            }
            // A NeoForge kind (`type`/`neoforge:ingredient_type`) is read as the object it is.
            (value.get("type").is_some() || value.get("neoforge:ingredient_type").is_some())
                .then(|| mod_api::serde_json::to_string(value).ok())
                .flatten()
        }
        _ => None,
    }
}

/// The item ids an ingredient (`{"item": …}`, `{"tag": …}` or a bare id) resolves to.
pub(crate) fn ingredient(
    host: &dyn Host,
    value: &mod_api::serde_json::Value,
) -> Option<Vec<String>> {
    use mod_api::serde_json::Value;

    let spec = ingredient_spec(value)?;
    let text = mod_api::serde_json::to_string(&Value::String(spec)).ok()?;
    let parsed = host.recipe_ingredient(&text).ok().flatten()?;
    let keys = parsed.item_keys(host).ok()?;
    let items: Vec<String> = keys.iter().map(|key| key.id(host)).collect();
    (!items.is_empty()).then_some(items)
}

/// One `LaserDrillRarity`: `biome_filter`, `dimension_filter`, `depth_min`, `depth_max`,
/// `weight`.
pub(crate) fn rarity_of(value: &mod_api::serde_json::Value) -> Option<Rarity> {
    use mod_api::serde_json::Value;

    let number = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_i64)
            .and_then(|number| i32::try_from(number).ok())
    };
    Some(Rarity {
        biome: filter(value.get("biome_filter"))?,
        dimension: filter(value.get("dimension_filter"))?,
        depth_min: number("depth_min")?,
        depth_max: number("depth_max")?,
        weight: number("weight")?,
    })
}

/// One `whitelist`/`blacklist` pair.
fn filter(value: Option<&mod_api::serde_json::Value>) -> Option<Filter> {
    let value = value?;
    let list = |key: &str| {
        value
            .get(key)
            .and_then(|list| list.as_array())
            .map(|list| {
                list.iter()
                    .filter_map(|entry| entry.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    };
    Some(Filter {
        whitelist: list("whitelist"),
        blacklist: list("blacklist"),
    })
}

// ---------------------------------------------------------------------------
// The Parts
// ---------------------------------------------------------------------------

/// The drill: its six-way placement, its tick and the augment seat.
pub(crate) struct DrillPart;

#[mod_api::horizon_part]
impl BlockPart for DrillPart {
    fn state_for_placement(
        &self,
        level: &dyn LevelHost,
        event: &mut StateForPlacementEvent<'_>,
    ) -> Option<Option<BlockState>> {
        let faced = level.state_with_property(
            event.state,
            FACING,
            crate::facing_for_placement(event.context.yaw, event.context.pitch),
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
impl BlockEntityPart for DrillPart {
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    /// One server tick: fall the bar, and at zero charge the laser base in the working area
    /// for 1,000 FE (`work()`), queueing the base's advance for the base's own tick.
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let target = target_base(host, pos);
        let entity = &mut *event.entity;
        let mut drill = Drill::from_saved(
            read_int(&entity.nbt, KEY_ENERGY),
            read_progress(&entity.nbt),
        );
        let step = drill.tick(target.is_some(), crate::read_augments(&entity.nbt));
        write_int(&mut entity.nbt, KEY_ENERGY, drill.energy);
        write_int(&mut entity.nbt, KEY_PROGRESS, drill.progress);
        if step == DrillStep::Charged
            && let Some(target) = target
        {
            queue_advance(host.dimension(), target);
        }
    }
}

/// The ore laser base: its efficiency-only augment seat and its tick.
pub(crate) struct BasePart;

#[mod_api::horizon_part]
impl BlockPart for BasePart {
    /// `canAcceptAugment`: the base reads only an efficiency addon — its bar is advanced by
    /// the drill, so a speed addon has nothing to move. Anything else passes through, as
    /// upstream falls through to the item's own `useOn`.
    fn use_item_on(
        &self,
        host: &mut dyn ServerHost,
        event: &mut UseItemOnEvent<'_>,
    ) -> Option<InteractionResult> {
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
impl BlockEntityPart for BasePart {
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    /// One server tick: take the advances the drills paid for, and at the top of the bar pick
    /// one weighted ore recipe and put its output in the fifteen output slots (`onWork`).
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let dimension = host.dimension();
        let entity = &mut *event.entity;
        if entity.items.len() < BASE_SLOTS {
            return;
        }
        let augments = crate::read_augments(&entity.nbt);
        let depth = read_depth(&entity.nbt, pos);
        if !entity.nbt.contains_key(&KEY_DEPTH.into()) {
            write_int(&mut entity.nbt, KEY_DEPTH, depth);
        }
        let mut base = Base::from_saved(read_int(&entity.nbt, KEY_PROGRESS));
        let mut completed = false;
        for _ in 0..take_advances(dimension, pos) {
            if base.advance(augments) {
                completed = true;
            }
        }
        write_int(&mut entity.nbt, KEY_PROGRESS, base.progress);
        if completed {
            let (lens, output) = entity.items.split_at_mut(LENS_SLOTS);
            on_work(host, lens, output, pos, depth);
        }
    }
}

/// Both machines' items: a broken machine's drop carries its buffer, bar and contents, and
/// placing it clamps them back to the capacities (slice 2's route).
pub(crate) struct DrillItem;

#[mod_api::horizon_part]
impl ItemPart for DrillItem {
    fn on_block_placed_by(&self, host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
        placed(host, event, DRILL_BLOCK);
    }
}

/// The base's item.
pub(crate) struct BaseItem;

#[mod_api::horizon_part]
impl ItemPart for BaseItem {
    fn on_block_placed_by(&self, host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
        placed(host, event, BASE_BLOCK);
    }
}

/// Clamp what a placed machine's item carried, as `/give`'s hand-written component may not.
fn placed(host: &mut dyn ServerHost, event: &OnBlockPlacedByEvent<'_>, block: &str) {
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
    if let Some(patch) = clamp_patch(block, &saved) {
        let _ = entity.merge_data(host, patch);
    }
}

// ---------------------------------------------------------------------------
// The drill's working area and the base it charges
// ---------------------------------------------------------------------------

/// The block coordinates `getWorkingArea` covers: `Shapes.box(-1, 0, -1, 2, 3, 2)` moved two
/// blocks along the direction **opposite** the drill's `facing`, lowered one block. Returned
/// as inclusive bounds.
#[must_use]
pub fn working_area(pos: [i32; 3], facing: &str) -> Option<([i32; 3], [i32; 3])> {
    // The facing's own normal, then the direction the drill looks (its opposite).
    let normal = match facing {
        "down" => [0, -1, 0],
        "up" => [0, 1, 0],
        "north" => [0, 0, -1],
        "south" => [0, 0, 1],
        "west" => [-1, 0, 0],
        "east" => [1, 0, 0],
        _ => return None,
    };
    let anchor = [
        pos[0] - normal[0] * 2,
        pos[1] - normal[1] * 2 - 1,
        pos[2] - normal[2] * 2,
    ];
    Some((
        [anchor[0] - 1, anchor[1], anchor[2] - 1],
        [anchor[0] + 1, anchor[1] + 2, anchor[2] + 1],
    ))
}

/// The first laser base in the working area, in the area's x/y/z order (`findTarget`).
fn target_base(host: &mut dyn ServerHost, pos: [i32; 3]) -> Option<[i32; 3]> {
    let level = host.level(host.dimension())?;
    let facing = level.block_property(host, pos, FACING)?;
    let (min, max) = working_area(pos, &facing)?;
    // `LaserDrillTile.isValidTarget` is any `ILaserBase`, so the same drill charges an ore
    // base or a fluid base.
    let ore = host.block_key(BASE_BLOCK)?;
    let fluid = host.block_key(crate::fluidbase::BLOCK);
    for x in min[0]..=max[0] {
        for y in min[1]..=max[1] {
            for z in min[2]..=max[2] {
                let candidate = [x, y, z];
                let block = level.block(host, candidate);
                if block == Some(ore) || (fluid.is_some() && block == fluid) {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

/// Queue one advance for the base at `pos` (the drill paid for it).
pub(crate) fn queue_advance(dimension: &'static str, pos: [i32; 3]) {
    if let Ok(mut advances) = ADVANCES.lock() {
        *advances.entry((pos, dimension)).or_insert(0) += 1;
    }
}

/// Take every advance queued for the base at `pos`.
pub(crate) fn take_advances(dimension: &'static str, pos: [i32; 3]) -> i32 {
    ADVANCES
        .lock()
        .ok()
        .and_then(|mut advances| advances.remove(&(pos, dimension)))
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// `onWork`: the pick, and the output slots
// ---------------------------------------------------------------------------

/// One completed base bar: pick a weighted recipe and insert one stack of its output.
fn on_work(
    host: &mut dyn ServerHost,
    lens: &[Option<ItemStack>],
    output: &mut [Option<ItemStack>],
    pos: [i32; 3],
    depth: i32,
) {
    let table = recipes(&mut *host);
    let roll = host
        .level(host.dimension())
        .and_then(|level| level.with_random(host, |random| random.next_double()))
        .unwrap_or(0.0);
    let lenses: Vec<Option<String>> = lens
        .iter()
        .map(|slot| slot.as_ref().map(|stack| stack.item().id(&*host)))
        .collect();
    let dimension = host.dimension();
    let level = host.level(dimension);
    let biome = level
        .and_then(|level| level.biome_at(&*host, pos))
        .unwrap_or_default();
    let in_tag = |tag: &str| {
        level
            .and_then(|level| level.biome_in_tag(&*host, &biome, tag))
            .unwrap_or(false)
    };
    let found = candidates(&table.rules, dimension, depth, &in_tag, &lenses);
    let Some(chosen) = pick(&found, roll) else {
        return;
    };
    let recipe = &table.rules[chosen.recipe];
    let Some(id) = recipe.output_item() else {
        return;
    };
    let Some(key) = mod_api::Identifier::parse(id)
        .ok()
        .and_then(|id| host.item_key(&id))
    else {
        return;
    };
    let count = u32::try_from(recipe.count.max(1)).unwrap_or(1);
    let Some(stack) = ItemStack::new(key, count) else {
        return;
    };
    if !output_room(&*host, output, &stack) {
        return;
    }
    crate::dissolution::put(&*host, output, stack);
}

/// Whether the output slots can take `product` (`insertItem(…, false).isEmpty()`).
fn output_room(host: &dyn Host, output: &[Option<ItemStack>], product: &ItemStack) -> bool {
    let max = host.max_stack_size(product.item()).unwrap_or(64);
    output.iter().any(|slot| match slot {
        None => true,
        Some(stack) => stack.item() == product.item() && stack.count() + product.count() <= max,
    })
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

/// A machine saved before this slice (or just placed) has no bar: the drill's starts full.
fn read_progress(nbt: &Compound) -> Option<i32> {
    match nbt.get(&KEY_PROGRESS.into()) {
        Some(Tag::Int(value)) => Some(*value),
        _ => None,
    }
}

/// The base's `miningDepth`: its saved value, its own Y when it has none, and its own Y for
/// the GUI's `-100` sentinel (`setChanged` resets it there).
#[must_use]
pub fn read_depth(nbt: &Compound, pos: [i32; 3]) -> i32 {
    let Some(Tag::Int(depth)) = nbt.get(&KEY_DEPTH.into()) else {
        return pos[1];
    };
    if *depth == -100 { pos[1] } else { *depth }
}

/// Whether a machine holds anything worth carrying on its item.
pub(crate) fn has_contents(block: &str, saved: &Compound) -> bool {
    if block == DRILL_BLOCK {
        return read_int(saved, KEY_ENERGY) > 0
            || read_progress(saved).unwrap_or(DRILL_MAX_PROGRESS) != DRILL_MAX_PROGRESS;
    }
    // The engine drops a mod container's slots itself (`preRemoveSideEffects` →
    // `Containers.dropContents`, measured: a base broken with two ores in it spawns both and
    // the machine item), so carrying `Items` on the item too would duplicate them.
    read_int(saved, KEY_PROGRESS) > 0 || crate::has_seated_augment(saved)
}

/// The `minecraft:block_entity_data` payload a broken machine's item carries.
pub(crate) fn carried_patch(block: &str, saved: &Compound) -> Compound {
    let id = if block == DRILL_BLOCK {
        DRILL_BLOCK
    } else {
        BASE_BLOCK
    };
    let mut payload = saved.clone();
    // The contents are the engine's own drops, never a second copy on the item.
    payload.remove(&"Items".into());
    payload.insert("id".into(), Tag::String(id.into()));
    let mut patch = Compound::new();
    patch.insert("minecraft:block_entity_data".into(), Tag::Compound(payload));
    patch
}

/// The corrections a placed machine needs when its item carried values outside the
/// capacities. `None` when the saved compound is already within range.
pub(crate) fn clamp_patch(block: &str, saved: &Compound) -> Option<Compound> {
    let mut patch = Compound::new();
    let mut changed = false;
    if block == DRILL_BLOCK {
        let energy = read_int(saved, KEY_ENERGY);
        let energy = energy.clamp(0, DRILL_CAPACITY_FE);
        if energy != read_int(saved, KEY_ENERGY) {
            write_int(&mut patch, KEY_ENERGY, energy);
            changed = true;
        }
        let progress = read_progress(saved).unwrap_or(DRILL_MAX_PROGRESS);
        let clamped = progress.clamp(0, DRILL_MAX_PROGRESS);
        if clamped != progress {
            write_int(&mut patch, KEY_PROGRESS, clamped);
            changed = true;
        }
    } else {
        let progress = read_int(saved, KEY_PROGRESS);
        if progress < 0 {
            write_int(&mut patch, KEY_PROGRESS, 0);
            changed = true;
        }
    }
    changed.then_some(patch)
}

// ---------------------------------------------------------------------------
// The commands
// ---------------------------------------------------------------------------

/// `/industrialforegoing energy`/`progress`/`depth` on either machine: one int of its saved
/// compound.
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

/// `/industrialforegoing depth`: the base's `miningDepth`.
pub(crate) fn set_depth(host: &mut dyn ServerHost, pos: [i32; 3], block: &str, value: i32) -> bool {
    set_int(host, pos, block, KEY_DEPTH, value)
}

/// `/industrialforegoing info` for the drill: buffer, bar, the base it found, augments, facing
/// and game time.
pub(crate) fn drill_readout(host: &mut dyn ServerHost, pos: [i32; 3]) -> String {
    let Some(entity) = crate::our_entity(host, pos, DRILL_BLOCK) else {
        return format!("{pos:?} is not a laser drill.");
    };
    let Some(data) = entity.data(&*host) else {
        return format!("{pos:?} is not a mod block entity.");
    };
    let drill = Drill::from_saved(read_int(&data, KEY_ENERGY), read_progress(&data));
    let augments = crate::read_augments(&data);
    let target = target_base(host, pos);
    let facing = entity
        .level(&*host)
        .and_then(|level| level.block_property(host, pos, FACING))
        .unwrap_or_else(|| "unknown".to_owned());
    let time = entity
        .level(&*host)
        .and_then(|level| level.game_time(host))
        .unwrap_or(0);
    format!(
        "Laser Drill at {pos:?}: {}/{} FE, progress {}/{}, target {}, augments speed {} efficiency {:.2}, facing {facing}, t={time}",
        drill.energy,
        DRILL_CAPACITY_FE,
        drill.progress,
        Drill::bar_length(augments),
        target.map_or_else(|| "none".to_owned(), |target| format!("{target:?}")),
        augments.speed,
        augments.efficiency,
    )
}

/// `/industrialforegoing info` for the base: its bar, depth, biome, dimension, the recipe its
/// filters resolve (if exactly one, else the first and a count), its weight, the table's size,
/// the lens and output slots, augments and game time.
pub(crate) fn base_readout(host: &mut dyn ServerHost, pos: [i32; 3]) -> String {
    let Some(entity) = crate::our_entity(host, pos, BASE_BLOCK) else {
        return format!("{pos:?} is not an ore laser base.");
    };
    let Some(data) = entity.data(&*host) else {
        return format!("{pos:?} is not a mod block entity.");
    };
    let base = Base::from_saved(read_int(&data, KEY_PROGRESS));
    let augments = crate::read_augments(&data);
    let depth = read_depth(&data, pos);
    let table = recipes(&mut *host);
    let dimension = host.dimension();
    let level = host.level(dimension);
    let biome = level
        .and_then(|level| level.biome_at(&*host, pos))
        .unwrap_or_else(|| "unknown".to_owned());
    let in_tag = |tag: &str| {
        level
            .and_then(|level| level.biome_in_tag(&*host, &biome, tag))
            .unwrap_or(false)
    };
    let lens: Vec<Option<String>> = (0..LENS_SLOTS)
        .map(|slot| {
            entity
                .item_slot(&*host, slot as u32)
                .ok()
                .flatten()
                .map(|stack| stack.item().id(&*host))
        })
        .collect();
    let output: Vec<String> = (LENS_SLOTS..BASE_SLOTS)
        .filter_map(|slot| {
            entity
                .item_slot(&*host, slot as u32)
                .ok()
                .flatten()
                .map(|stack| format!("{} x{}", stack.item().id(&*host), stack.count()))
        })
        .collect();
    let found = candidates(&table.rules, dimension, depth, &in_tag, &lens);
    let (recipe, weight) = match found.split_first() {
        None => ("none".to_owned(), 0),
        Some((first, rest)) => {
            let mut id = table.rules[first.recipe].id.clone();
            if !rest.is_empty() {
                id.push_str(&format!(" (+{} more)", rest.len()));
            }
            (id, first.weight)
        }
    };
    let lenses = lens
        .iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    let time = level.and_then(|level| level.game_time(host)).unwrap_or(0);
    format!(
        "Ore Laser Base at {pos:?}: progress {}/{}, depth {depth}, biome {biome}, dimension {dimension}, recipe {recipe}, weight {weight}, recipes {}/{}, lens {}, output {}, augments speed {} efficiency {:.2}, t={time}",
        base.progress,
        Base::bar_length(augments),
        table.rules.len(),
        table.fingerprint.len(),
        if lenses.is_empty() {
            "empty".to_owned()
        } else {
            lenses
        },
        if output.is_empty() {
            "empty".to_owned()
        } else {
            output.join(", ")
        },
        augments.speed,
        augments.efficiency,
    )
}

/// The block entity type the declarations made, for the augment seat.
pub(crate) fn kind_of(entity: mod_api::BlockEntityRef, host: &dyn Host) -> bool {
    let Some(kind) = entity.kind(host) else {
        return false;
    };
    DRILL_KIND.get() == Some(&kind) || BASE_KIND.get() == Some(&kind)
}

/// The item `block_drops` may write to, by block id.
pub(crate) fn item(block: &str) -> Option<ItemKey> {
    if block == DRILL_BLOCK {
        DRILL_ITEM.get().copied()
    } else {
        BASE_ITEM.get().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_base_declares_six_lens_slots_and_fifteen_output_slots() {
        let container = base_container();
        assert_eq!(container.slots, 21);
        assert_eq!(container.inputs, (0..LENS_SLOTS).collect::<Vec<usize>>());
        assert_eq!(
            container.outputs,
            (LENS_SLOTS..BASE_SLOTS).collect::<Vec<usize>>()
        );
    }

    #[test]
    fn the_lens_slots_are_not_reachable_from_any_face() {
        let container = base_container();
        for face in &container.faces {
            assert_eq!(
                face,
                &(LENS_SLOTS..BASE_SLOTS).collect::<Vec<usize>>(),
                "upstream's lens inventory has every face mode NONE"
            );
        }
    }

    #[test]
    fn the_drill_buffer_is_ten_thousand_fe() {
        let battery = battery();
        assert_eq!(battery.capacity, 10_000);
        assert_eq!(battery.max_insert, 10_000);
        assert_eq!(battery.max_extract, 10_000);
        assert_eq!(battery.insert_faces, [true; 6]);
        assert_eq!(battery.extract_faces, [true; 6]);
    }

    #[test]
    fn the_working_area_is_two_blocks_the_opposite_of_the_facing() {
        assert_eq!(
            working_area([10, 20, 30], "up"),
            Some(([9, 17, 29], [11, 19, 31])),
            "a drill facing up works the three blocks below it"
        );
        assert_eq!(
            working_area([10, 20, 30], "down"),
            Some(([9, 21, 29], [11, 23, 31])),
            "facing down, the three above"
        );
        assert_eq!(
            working_area([10, 20, 30], "north"),
            Some(([9, 19, 31], [11, 21, 33])),
            "facing north, the box to the south"
        );
        assert_eq!(working_area([10, 20, 30], "subfacing"), None);
    }

    #[test]
    fn a_base_without_a_saved_depth_mines_at_its_own_y() {
        let mut nbt = Compound::new();
        assert_eq!(read_depth(&nbt, [202, 101, 0]), 101);
        write_int(&mut nbt, KEY_DEPTH, 64);
        assert_eq!(read_depth(&nbt, [202, 101, 0]), 64);
        write_int(&mut nbt, KEY_DEPTH, -100);
        assert_eq!(
            read_depth(&nbt, [202, 101, 0]),
            101,
            "the GUI's -100 sentinel is the base's own Y"
        );
    }

    #[test]
    fn the_queued_advances_are_per_position_and_are_taken_once() {
        queue_advance("minecraft:the_end", [202, 101, 0]);
        queue_advance("minecraft:the_end", [202, 101, 0]);
        assert_eq!(take_advances("minecraft:the_end", [202, 101, 0]), 2);
        assert_eq!(
            take_advances("minecraft:the_end", [202, 101, 0]),
            0,
            "a taken advance is spent"
        );
        assert_eq!(take_advances("minecraft:the_end", [203, 101, 0]), 0);
        queue_advance("minecraft:overworld", [202, 101, 0]);
        assert_eq!(
            take_advances("minecraft:the_end", [202, 101, 0]),
            0,
            "another dimension's advance does not leak"
        );
        assert_eq!(take_advances("minecraft:overworld", [202, 101, 0]), 1);
    }

    #[test]
    fn a_broken_drill_carries_its_buffer_and_bar() {
        let mut nbt = Compound::new();
        assert!(
            !has_contents(DRILL_BLOCK, &nbt),
            "a fresh drill carries nothing"
        );
        write_int(&mut nbt, KEY_ENERGY, 4_000);
        assert!(has_contents(DRILL_BLOCK, &nbt));
        let mut nbt = Compound::new();
        write_int(&mut nbt, KEY_PROGRESS, 17);
        assert!(has_contents(DRILL_BLOCK, &nbt));
        let patch = carried_patch(DRILL_BLOCK, &nbt);
        let Some(Tag::Compound(payload)) = patch.get(&"minecraft:block_entity_data".into()) else {
            panic!("no block entity data in the patch");
        };
        assert_eq!(
            payload.get(&"id".into()),
            Some(&Tag::String(DRILL_BLOCK.into()))
        );
    }

    #[test]
    fn a_broken_base_carries_its_bar_and_not_its_slots() {
        let mut nbt = Compound::new();
        assert!(
            !has_contents(BASE_BLOCK, &nbt),
            "a fresh base carries nothing"
        );
        write_int(&mut nbt, KEY_PROGRESS, 3);
        assert!(has_contents(BASE_BLOCK, &nbt), "a part-done bar travels");
        let mut nbt = Compound::new();
        let mut item = Compound::new();
        item.insert("Slot".into(), Tag::Byte(6));
        item.insert("id".into(), Tag::String("minecraft:emerald_ore".into()));
        item.insert("count".into(), Tag::Int(1));
        nbt.insert("Items".into(), Tag::List(vec![Tag::Compound(item)]));
        assert!(
            !has_contents(BASE_BLOCK, &nbt),
            "the engine drops a mod container's slots itself, so the item must not carry them"
        );
        write_int(&mut nbt, KEY_PROGRESS, 3);
        let patch = carried_patch(BASE_BLOCK, &nbt);
        let Some(Tag::Compound(payload)) = patch.get(&"minecraft:block_entity_data".into()) else {
            panic!("no block entity data in the patch");
        };
        assert_eq!(
            payload.get(&"id".into()),
            Some(&Tag::String(BASE_BLOCK.into()))
        );
        assert!(
            payload.get(&"Items".into()).is_none(),
            "the payload has no Items, or the contents would be duplicated"
        );
    }

    #[test]
    fn a_placed_machine_clamps_what_its_item_carried() {
        let mut drill = Compound::new();
        write_int(&mut drill, KEY_ENERGY, 50_000);
        write_int(&mut drill, KEY_PROGRESS, 999);
        let patch = clamp_patch(DRILL_BLOCK, &drill).expect("out-of-range values are corrected");
        assert_eq!(read_int(&patch, KEY_ENERGY), DRILL_CAPACITY_FE);
        assert_eq!(read_progress(&patch), Some(DRILL_MAX_PROGRESS));
        let mut fine = Compound::new();
        write_int(&mut fine, KEY_ENERGY, 4_000);
        write_int(&mut fine, KEY_PROGRESS, 20);
        assert!(clamp_patch(DRILL_BLOCK, &fine).is_none());
        let mut base = Compound::new();
        write_int(&mut base, KEY_PROGRESS, -4);
        let patch = clamp_patch(BASE_BLOCK, &base).expect("a negative bar is corrected");
        assert_eq!(read_int(&patch, KEY_PROGRESS), 0);
        assert!(
            clamp_patch(BASE_BLOCK, &Compound::new()).is_none(),
            "a fresh base needs no correction"
        );
    }

    /// The jar writes `{"item": …}` and `{"tag": …}`; the pin's ingredient parser reads a
    /// bare id or a `#tag` string for those two shapes, so the decode must flatten them.
    /// This is the bug the first in-game run found: `recipes 0/10` with 10 files kept.
    #[test]
    fn the_ingredient_forms_the_recipe_files_use_are_all_read() {
        use mod_api::serde_json::json;

        assert_eq!(
            ingredient_spec(&json!({"item": "minecraft:coal_ore"})).as_deref(),
            Some("minecraft:coal_ore")
        );
        assert_eq!(
            ingredient_spec(&json!({"tag": "c:raw_materials/iron"})).as_deref(),
            Some("#c:raw_materials/iron")
        );
        assert_eq!(
            ingredient_spec(&json!("minecraft:redstone")).as_deref(),
            Some("minecraft:redstone")
        );
        assert_eq!(ingredient_spec(&json!({"count": 1})), None);
    }

    #[test]
    fn the_six_way_facing_turns_toward_the_placer() {
        assert_eq!(crate::facing_for_placement(0.0, 0.0), "north");
        assert_eq!(crate::facing_for_placement(90.0, 0.0), "east");
        assert_eq!(crate::facing_for_placement(180.0, 0.0), "south");
        assert_eq!(crate::facing_for_placement(270.0, 0.0), "west");
        assert_eq!(crate::facing_for_placement(0.0, 50.0), "up");
        assert_eq!(crate::facing_for_placement(0.0, -50.0), "down");
        assert_eq!(
            crate::facing_for_placement(0.0, 39.0),
            "north",
            "under 40 degrees is still horizontal"
        );
    }

    #[test]
    fn the_table_is_the_ten_recipes_when_the_package_is_installed() {
        // The content test in `lib.rs` proves the ten files; this pins the constants the
        // table and the container are read with.
        assert_eq!(crate::laser::CATALYST_MODIFIER, 8);
        assert_eq!(crate::laser::BASE_MAX_PROGRESS, 100);
        assert_eq!(crate::laser::DRILL_POWER_FE, 1_000);
    }
}
