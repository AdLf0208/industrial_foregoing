//! The game side of the latex chain: the **fluid extractor** and the **latex processing unit**
//! (`ModuleCore.FLUID_EXTRACTOR` and `ModuleCore.LATEX_PROCESSING`).
//!
//! Upstream's `FluidExtractorTile` is an `IndustrialAreaWorkingTile(BEHIND)` — a bar counting down
//! over the block *behind* the machine — and `LatexProcessingUnitTile` is an
//! `IndustrialProcessingTile` — a bar counting up while latex and water are in its tanks. The
//! arithmetic is [`crate::latex`]; this file reads and writes the game.
//!
//! # The extractor's recipe type
//!
//! Its recipes are files of its own type (`industrialforegoing:fluid_extractor`) whose products
//! this package ships. The type is declared with `Registrar::register_recipe_type` and its files
//! are kept by a read-only serializer, exactly as the dissolution chamber's are; this module
//! decodes them once per table identity and matches the block behind the machine against them.
//!
//! # The four fluids and the one that cannot be inserted
//!
//! `industrialforegoing:latex` is a declared fluid key. `native_fluid_insert` — and so the SDK's
//! `fluid_tank_insert` and the `.hnmod` era's `native_move_fluids`, which calls it — takes only
//! `minecraft:water` and `minecraft:lava`, so a mod fluid has no capability fill path at this pin:
//! the latex tank is written through the entity's own `fluids` compound by `/industrialforegoing
//! fluid`, and `/industrialforegoing fill` proves the refusal through the declared capability.
//! Water **is** inserted through the capability, which is why the unit's latex tank declares no
//! fill face: the pin cannot filter a tank by fluid, and a fill of water must not land in it.
//!
//! # The hit counter
//!
//! `FluidExtractorTile.EXTRACTION` is a static map of position to progress, shared by every
//! extractor on that block, and it is not saved. The port keeps the same map here. Upstream rolls
//! `level.random.nextDouble()`; the pin's host has no random source, so the roll is a uniform mix
//! of the game clock and the position ([`roll_at`]), and `/industrialforegoing roll` can pin the
//! next one so the eighth-hit path is observable in bounded time.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

use mod_api::{
    self, BlockEntityPart, BlockEntityTickEvent, BlockEntityTicksEvent, BlockPart, BlockState,
    BlockUpdateFlags, Compound, Direction, FLUIDS, FluidResource, FluidStack, Host,
    HostTransaction, InteractionResult, ItemKey, ItemPart, ItemStack, LevelHost, ModContainer,
    ModEnergyBuffer, ModFluidTanks, OnBlockPlacedByEvent, RandomSource, Registrar, ServerHost,
    StateForPlacementEvent, Tag, UseItemOnEvent,
};

use crate::latex::{
    EXTRACTOR_CAPACITY_FE, EXTRACTOR_HITS, EXTRACTOR_MAX_PROGRESS, EXTRACTOR_TANK_MB, Extractor,
    ExtractorRecipe, ExtractorStep, UNIT_CAPACITY_FE, UNIT_SLOTS, UNIT_TANK_MB, Unit, UnitStep,
    recipe_for,
};

/// The fluid extractor (`FluidExtractorBlock`).
pub const EXTRACTOR_BLOCK: &str = "industrialforegoing:fluid_extractor";
/// Its block entity type, a bare name so the mod's namespace qualifies it.
pub const EXTRACTOR_ENTITY: &str = "fluid_extractor";
/// The latex processing unit (`LatexProcessingUnitBlock`).
pub const UNIT_BLOCK: &str = "industrialforegoing:latex_processing_unit";
/// Its block entity type.
pub const UNIT_ENTITY: &str = "latex_processing_unit";
/// The extractor's own recipe type (`ModuleCore.FLUID_EXTRACTOR_TYPE`).
pub const RECIPE_TYPE: &str = "industrialforegoing:fluid_extractor";
/// The unit's product (`ModuleCore.DRY_RUBBER`).
pub const RUBBER: &str = "industrialforegoing:dryrubber";
/// The mod's own fluid, declared by slice 5 and produced here.
pub const LATEX: &str = "industrialforegoing:latex";
/// Water, the unit's second fluid — the one the pin's capability does accept.
pub const WATER: &str = "minecraft:water";
/// `IndustrialBlock.RotationType.FOUR_WAY`'s state property.
pub const SUBFACING: &str = "subfacing";
/// The saved keys.
const KEY_ENERGY: &str = "energy";
const KEY_PROGRESS: &str = "progress";
/// The side the tanks are read, drained and filled through (all six reach them).
const READ_FACE: Direction = Direction::Up;
/// The tank indices, in the order the declarations list them.
const LATEX_TANK: usize = 0;
const WATER_TANK: usize = 1;

/// The block entity types and the items, resolved once in `register`: `block_drops` and the
/// command path run with a read-only level host, which cannot name either.
static EXTRACTOR_KIND: OnceLock<mod_api::BlockEntityTypeKey> = OnceLock::new();
static UNIT_KIND: OnceLock<mod_api::BlockEntityTypeKey> = OnceLock::new();
static EXTRACTOR_ITEM: OnceLock<ItemKey> = OnceLock::new();
static UNIT_ITEM: OnceLock<ItemKey> = OnceLock::new();

/// A log a machine works on: its position and the dimension it stands in.
type LogKey = ([i32; 3], &'static str);

/// The hit counter of every log a machine works on, as `FluidExtractorTile.EXTRACTION`: by
/// dimension and position, shared between extractors, and not saved (upstream does not save it).
static HITS: Mutex<BTreeMap<LogKey, i32>> = Mutex::new(BTreeMap::new());

/// The rolls `/industrialforegoing roll` pinned, one per position, spent by the next operation.
static PINNED_ROLLS: Mutex<BTreeMap<LogKey, f32>> = Mutex::new(BTreeMap::new());

/// The decoded extractor recipes and the identity of the table they came from.
#[derive(Debug)]
pub(crate) struct Table {
    /// `(id, the file's own allocation)`: the identity of the recipe table, so a `/reload` that
    /// changed it rebuilds the decode and every tick in between reads this instead of crossing
    /// into the game.
    fingerprint: Vec<(String, usize)>,
    /// The decoded rules, in file order.
    pub(crate) rules: Vec<ExtractorRecipe>,
}

/// The recipe table, as last decoded.
static TABLE: Mutex<Option<Arc<Table>>> = Mutex::new(None);

// ---------------------------------------------------------------------------
// The declarations
// ---------------------------------------------------------------------------

/// Declare both machines: their block entity types, their FE buffers, their tanks, the unit's nine
/// output slots, the Parts that tick and rotate them, and the Parts on their items.
///
/// Called from [`crate::Mod::register`]; the extractor's recipe type and serializer are declared
/// there beside it, with the chamber's.
pub(crate) fn declare(r: &mut dyn Registrar) {
    let block = r.block_key(EXTRACTOR_BLOCK);
    let item = r.item_key(EXTRACTOR_BLOCK);
    let _ = EXTRACTOR_ITEM.set(item);
    r.item_part(item, ExtractorItem);
    let kind = r
        .block_entity_type(EXTRACTOR_ENTITY)
        .blocks(&[EXTRACTOR_ENTITY])
        .build();
    let _ = EXTRACTOR_KIND.set(kind);
    r.block_entity_part(&kind, ExtractorPart)
        .expect("the extractor's Part attaches to the type this mod just declared");
    r.block_energy(&block, buffer(EXTRACTOR_CAPACITY_FE));
    r.block_fluids(&block, extractor_tanks());

    let block = r.block_key(UNIT_BLOCK);
    let item = r.item_key(UNIT_BLOCK);
    let _ = UNIT_ITEM.set(item);
    r.item_part(item, UnitItem);
    let kind = r
        .block_entity_type(UNIT_ENTITY)
        .blocks(&[UNIT_ENTITY])
        .build();
    let _ = UNIT_KIND.set(kind);
    r.block_entity_part(&kind, UnitPart)
        .expect("the unit's Part attaches to the type this mod just declared");
    r.block_energy(&block, buffer(UNIT_CAPACITY_FE));
    r.block_fluids(&block, unit_tanks());
    r.block_container(&block, unit_container());
}

/// One `EnergyStorageComponent(capacity, 10, 20)`: unlimited IO up to capacity.
fn buffer(capacity: i32) -> ModEnergyBuffer {
    ModEnergyBuffer {
        capacity,
        max_insert: capacity,
        max_extract: capacity,
        insert_faces: [true; 6],
        extract_faces: [true; 6],
    }
}

/// The extractor's tank: 1,000 mB, `Action.DRAIN` — every face reads and drains it, none fills it
/// (the machine fills it itself, and the pin could not insert a mod fluid anyway).
#[must_use]
pub fn extractor_tanks() -> ModFluidTanks {
    let all = vec![LATEX_TANK];
    let mut tanks = ModFluidTanks::default();
    tanks.tanks = vec![EXTRACTOR_TANK_MB];
    tanks.faces = std::array::from_fn(|_| all.clone());
    tanks.outputs = vec![LATEX_TANK];
    tanks
}

/// The unit's two tanks, 16,000 mB each: latex (drainable, **not** fillable — the pin's fill path
/// cannot tell a tank by fluid, and latex has no fill path at all) and water (fillable and
/// drainable, which is the one real capability insert this machine has).
#[must_use]
pub fn unit_tanks() -> ModFluidTanks {
    let both = vec![LATEX_TANK, WATER_TANK];
    let mut tanks = ModFluidTanks::default();
    tanks.tanks = vec![UNIT_TANK_MB, UNIT_TANK_MB];
    tanks.faces = std::array::from_fn(|_| both.clone());
    tanks.inputs = vec![WATER_TANK];
    tanks.outputs = vec![LATEX_TANK, WATER_TANK];
    tanks
}

/// The unit's nine output slots (`SidedInventoryComponent("output", …, 9, 2)`): every face reaches
/// them, a pipe may take from them.
#[must_use]
pub fn unit_container() -> ModContainer {
    let all: Vec<usize> = (0..UNIT_SLOTS).collect();
    let mut container = ModContainer {
        slots: UNIT_SLOTS,
        ..ModContainer::default()
    };
    container.faces = std::array::from_fn(|_| all.clone());
    container.outputs = all;
    container
}

// ---------------------------------------------------------------------------
// The extractor's recipe table
// ---------------------------------------------------------------------------

/// The extractor recipes in force, decoded once per table.
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

/// One recipe file (`FluidExtractorRecipe.CODEC`): `input` (one 26.3 ingredient), `result` (the
/// `/setblock` string the port re-spells it to), `breakChance`, `output` (`FluidStack`: `id` +
/// `amount`) and `defaultRecipe`. A file whose input resolves to nothing could never match, so it
/// is left out — the same outcome as the game dropping a recipe it cannot read.
fn one(host: &dyn Host, id: &str, value: &mod_api::serde_json::Value) -> Option<ExtractorRecipe> {
    use mod_api::serde_json::Value;

    let text = mod_api::serde_json::to_string(value.get("input")?).ok()?;
    let ingredient = host.recipe_ingredient(&text).ok().flatten()?;
    let keys = ingredient.item_keys(host).ok()?;
    let input: Vec<String> = keys.iter().map(|key| key.id(host)).collect();
    if input.is_empty() {
        return None;
    }
    let result = match value.get("result")? {
        Value::String(text) => text.clone(),
        // The jar writes a block state object; `assemble_content.py` re-spells it as the
        // `/setblock` string `block_state_id` reads. A file from another pack may still carry
        // the object, so read its `Name` and leave the properties at their defaults.
        other => other.get("Name")?.as_str()?.to_owned(),
    };
    let break_chance = value.get("breakChance")?.as_f64()? as f32;
    let mb = value
        .get("output")?
        .get("amount")
        .and_then(Value::as_i64)
        .and_then(|mb| i32::try_from(mb).ok())?;
    Some(ExtractorRecipe {
        id: id.to_owned(),
        input,
        result,
        break_chance,
        output_mb: mb,
        default_recipe: value
            .get("defaultRecipe")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

// ---------------------------------------------------------------------------
// The Parts
// ---------------------------------------------------------------------------

/// The fluid extractor's block entity: its tick, and the placement rotation every `FOUR_WAY` block
/// takes.
pub(crate) struct ExtractorPart;

#[mod_api::horizon_part]
impl BlockPart for ExtractorPart {
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

    /// `AddonItem.useOn`: the two augment slots are the base tile's, seated by a right-click.
    fn use_item_on(
        &self,
        host: &mut dyn ServerHost,
        event: &mut UseItemOnEvent<'_>,
    ) -> Option<InteractionResult> {
        crate::seat_from_click(host, event)
    }
}

#[mod_api::horizon_part]
impl BlockEntityPart for ExtractorPart {
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    /// One server tick: fall the bar, and at zero match the block **behind** the machine and run
    /// one operation (`work()`), then write the machine back.
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let dimension = host.dimension();
        // The hit counter belongs to the **log**, which every extractor on that block shares, so
        // both the saved read and the write are keyed by the position behind the machine.
        let target = pointed(host, pos);
        let key = target.unwrap_or(pos);
        let entity = &mut *event.entity;
        let mut machine = Extractor::from_saved(
            read_int(&entity.nbt, KEY_ENERGY),
            read_tank(&entity.nbt, LATEX_TANK),
            read_progress(&entity.nbt),
            hits(dimension, key),
        );
        let behind = target.and_then(|target| block_id(host, target));
        let table = recipes(&mut *host);
        let recipe = behind
            .as_deref()
            .and_then(|id| recipe_for(&table.rules, id));
        // `work()` rolls only when the bar completes (`if (this.level.random.nextDouble() <= …)`
        // inside it), so the roll is drawn on that tick alone: a pin set for the next operation
        // must not be spent by a tick that merely falls the bar.
        let roll = if machine.progress <= 1 {
            next_roll(host, dimension, key)
        } else {
            1.0
        };
        let step = machine.tick(recipe, roll);
        write_int(&mut entity.nbt, KEY_ENERGY, machine.energy);
        write_tank(
            &mut entity.nbt,
            LATEX_TANK,
            machine.latex_mb,
            EXTRACTOR_TANK_MB,
            LATEX,
        );
        write_int(&mut entity.nbt, KEY_PROGRESS, machine.progress);
        set_hits(dimension, key, machine.hits);
        if let ExtractorStep::Extracted(extraction) = step
            && let (Some(target), Some(result)) = (target, extraction.result.as_deref())
        {
            let _ = replace_block(host, target, result);
        }
    }
}

/// The latex processing unit's block entity.
pub(crate) struct UnitPart;

#[mod_api::horizon_part]
impl BlockPart for UnitPart {
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
impl BlockEntityPart for UnitPart {
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    /// One server tick: rise the bar while both fluids, the buffer and the output slots allow it,
    /// and at the end of the bar take 750 mB of latex, 500 mB of water and give one rubber.
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let entity = &mut *event.entity;
        if entity.items.len() < UNIT_SLOTS {
            return;
        }
        let mut machine = Unit::from_saved(
            read_int(&entity.nbt, KEY_ENERGY),
            read_tank(&entity.nbt, LATEX_TANK),
            read_tank(&entity.nbt, WATER_TANK),
            read_int(&entity.nbt, KEY_PROGRESS),
        );
        let room = rubber_room(&*host, &entity.items[..]);
        let step = machine.tick(room, crate::read_augments(&entity.nbt));
        write_int(&mut entity.nbt, KEY_ENERGY, machine.energy);
        write_int(&mut entity.nbt, KEY_PROGRESS, machine.progress);
        write_tank(
            &mut entity.nbt,
            LATEX_TANK,
            machine.latex_mb,
            UNIT_TANK_MB,
            LATEX,
        );
        write_tank(
            &mut entity.nbt,
            WATER_TANK,
            machine.water_mb,
            UNIT_TANK_MB,
            WATER,
        );
        if step == UnitStep::Completed
            && let Some(rubber) = rubber(&*host)
        {
            crate::dissolution::put(&*host, &mut entity.items[..], rubber);
        }
    }
}

/// Whether the unit's output slots can take one more rubber (`insertItem(…, true).isEmpty()`).
fn rubber_room(host: &dyn Host, outputs: &[Option<ItemStack>]) -> bool {
    let Some(rubber) = rubber(host) else {
        return false;
    };
    let max = host.max_stack_size(rubber.item()).unwrap_or(64);
    let id = rubber.item();
    outputs.iter().any(|slot| match slot {
        None => true,
        Some(stack) => stack.item() == id && stack.count() < max,
    })
}

/// One `industrialforegoing:dryrubber` stack.
fn rubber(host: &dyn Host) -> Option<ItemStack> {
    let key = host.item_key(&mod_api::Identifier::parse(RUBBER).ok()?)?;
    ItemStack::new(key, 1)
}

/// Both machines' items: a broken machine's drop carries its buffer, fluids, bar and augments,
/// and placing it clamps them back to the capacities (slice 2's route).
pub(crate) struct ExtractorItem;

#[mod_api::horizon_part]
impl ItemPart for ExtractorItem {
    fn on_block_placed_by(&self, host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
        placed(host, event, EXTRACTOR_BLOCK);
    }
}

/// The unit's item.
pub(crate) struct UnitItem;

#[mod_api::horizon_part]
impl ItemPart for UnitItem {
    fn on_block_placed_by(&self, host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
        placed(host, event, UNIT_BLOCK);
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
// The block behind the extractor, the roll, and the hit counter
// ---------------------------------------------------------------------------

/// The position the extractor works on: `RangeManager.RangeType.BEHIND` — the block opposite its
/// `subfacing`.
fn pointed(host: &mut dyn ServerHost, pos: [i32; 3]) -> Option<[i32; 3]> {
    let level = host.level(host.dimension())?;
    let facing = level.block_property(host, pos, SUBFACING)?;
    let [dx, dy, dz] = behind(&facing)?;
    Some([pos[0] + dx, pos[1] + dy, pos[2] + dz])
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

/// The id of the block at `pos`: the block's item identity, which is what
/// `FluidExtractorRecipe.matches` tests (`input.test(new ItemStack(state.getBlock()))`).
fn block_id(host: &mut dyn ServerHost, pos: [i32; 3]) -> Option<String> {
    let level = host.level(host.dimension())?;
    let block = level.block(host, pos)?;
    Some(block.id(&*host))
}

/// Replace the block at `pos` with the recipe's `result`, as `setBlockAndUpdate` does.
fn replace_block(host: &mut dyn ServerHost, pos: [i32; 3], result: &str) -> bool {
    let Some(level) = host.level(host.dimension()) else {
        return false;
    };
    let Some(state) = host.block_state_id(result) else {
        return false;
    };
    level
        .set_block(
            host,
            pos,
            state,
            BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS,
        )
        .unwrap_or(false)
}

/// This operation's `breakChance` roll: the one `/industrialforegoing roll` pinned, else a draw
/// from the level's own random stream — `level.random.nextDouble()`, which is what upstream reads
/// (`LevelRef::with_random` keeps the stream in the level after the draw). A level the handle
/// cannot reach draws no hit (`1.0`).
fn next_roll(host: &mut dyn ServerHost, dimension: &'static str, pos: [i32; 3]) -> f32 {
    if let Ok(mut pinned) = PINNED_ROLLS.lock()
        && let Some(roll) = pinned.remove(&(pos, dimension))
    {
        return roll;
    }
    let Some(level) = host.level(host.dimension()) else {
        return 1.0;
    };
    level
        .with_random(host, |random| random.next_double() as f32)
        .unwrap_or(1.0)
}

/// Pin this position's next `breakChance` roll.
pub(crate) fn pin_roll(dimension: &'static str, pos: [i32; 3], roll: f32) {
    if let Ok(mut pinned) = PINNED_ROLLS.lock() {
        pinned.insert((pos, dimension), roll.clamp(0.0, 1.0));
    }
}

/// The hits the log at `pos` has taken.
fn hits(dimension: &'static str, pos: [i32; 3]) -> i32 {
    HITS.lock()
        .ok()
        .and_then(|hits| hits.get(&(pos, dimension)).copied())
        .unwrap_or(0)
}

/// Set the hits the log at `pos` has taken.
fn set_hits(dimension: &'static str, pos: [i32; 3], value: i32) {
    if let Ok(mut hits) = HITS.lock() {
        hits.insert((pos, dimension), value);
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

/// A machine saved before this slice (or just placed) has no bar: the extractor's starts full and
/// the unit's empty, as their bars do.
fn read_progress(nbt: &Compound) -> Option<i32> {
    match nbt.get(&KEY_PROGRESS.into()) {
        Some(Tag::Int(value)) => Some(*value),
        _ => None,
    }
}

/// One tank of the entity's `fluids` list (the pin's shape: one `{Fluid, Amount}` per tank).
fn read_tank(nbt: &Compound, tank: usize) -> i32 {
    let Some(Tag::List(entries)) = nbt.get(&"fluids".into()) else {
        return 0;
    };
    let Some(Tag::Compound(entry)) = entries.get(tank) else {
        return 0;
    };
    match entry.get(&"Amount".into()) {
        Some(Tag::Int(amount)) => *amount,
        _ => 0,
    }
}

/// Write one tank back in the same shape, keeping the other entries where they are.
fn write_tank(nbt: &mut Compound, tank: usize, mb: i32, capacity: i32, fluid: &str) {
    let mut entries: Vec<Tag> = match nbt.get(&"fluids".into()) {
        Some(Tag::List(entries)) => entries.clone(),
        _ => Vec::new(),
    };
    while entries.len() <= tank {
        entries.push(Tag::Compound(Compound::new()));
    }
    let mut entry = Compound::new();
    let mb = mb.clamp(0, capacity);
    entry.insert(
        "Fluid".into(),
        Tag::String(if mb > 0 { fluid } else { "minecraft:empty" }.into()),
    );
    entry.insert("Amount".into(), Tag::Int(mb));
    entries[tank] = Tag::Compound(entry);
    nbt.insert("fluids".into(), Tag::List(entries));
}

/// The tank's fluid id, for the readout and the command's answer.
fn tank_fluid(nbt: &Compound, tank: usize) -> String {
    let Some(Tag::List(entries)) = nbt.get(&"fluids".into()) else {
        return "empty".to_owned();
    };
    let Some(Tag::Compound(entry)) = entries.get(tank) else {
        return "empty".to_owned();
    };
    match entry.get(&"Fluid".into()) {
        Some(Tag::String(fluid)) => fluid.to_string_lossy(),
        _ => "empty".to_owned(),
    }
}

/// Whether a machine holds anything worth carrying on its item.
pub(crate) fn has_contents(block: &str, saved: &Compound) -> bool {
    if crate::has_seated_augment(saved) {
        return true;
    }
    if block == EXTRACTOR_BLOCK {
        return read_int(saved, KEY_ENERGY) > 0
            || read_tank(saved, LATEX_TANK) > 0
            || read_progress(saved).unwrap_or(EXTRACTOR_MAX_PROGRESS) != EXTRACTOR_MAX_PROGRESS;
    }
    read_int(saved, KEY_ENERGY) > 0
        || read_tank(saved, LATEX_TANK) > 0
        || read_tank(saved, WATER_TANK) > 0
        || read_int(saved, KEY_PROGRESS) > 0
}

/// One tank's entry in the pin's shape (`{Fluid, Amount}`), `minecraft:empty` when it holds none.
fn tank_entry(fluid: &str, mb: i32, capacity: i32) -> Tag {
    let mb = mb.clamp(0, capacity);
    let mut entry = Compound::new();
    entry.insert(
        "Fluid".into(),
        Tag::String(if mb > 0 { fluid } else { "minecraft:empty" }.into()),
    );
    entry.insert("Amount".into(), Tag::Int(mb));
    Tag::Compound(entry)
}

/// The `minecraft:block_entity_data` payload a broken machine's item carries: a **fresh
/// allowlist** of the machine's own keys, never the snapshot. `§9m`: the engine spills a mod
/// container's slots itself, so a payload that also held `Items` would duplicate every stack when
/// the item is placed back.
pub(crate) fn carried_patch(block: &str, saved: &Compound) -> Compound {
    let mut payload = Compound::new();
    let mut fluids = Vec::new();
    if block == EXTRACTOR_BLOCK {
        payload.insert("id".into(), Tag::String(EXTRACTOR_BLOCK.into()));
        payload.insert(
            KEY_ENERGY.into(),
            Tag::Int(read_int(saved, KEY_ENERGY).clamp(0, EXTRACTOR_CAPACITY_FE)),
        );
        payload.insert(
            KEY_PROGRESS.into(),
            Tag::Int(
                read_progress(saved)
                    .unwrap_or(EXTRACTOR_MAX_PROGRESS)
                    .clamp(0, EXTRACTOR_MAX_PROGRESS),
            ),
        );
        fluids.push(tank_entry(
            LATEX,
            read_tank(saved, LATEX_TANK),
            EXTRACTOR_TANK_MB,
        ));
    } else {
        payload.insert("id".into(), Tag::String(UNIT_BLOCK.into()));
        payload.insert(
            KEY_ENERGY.into(),
            Tag::Int(read_int(saved, KEY_ENERGY).clamp(0, UNIT_CAPACITY_FE)),
        );
        payload.insert(
            KEY_PROGRESS.into(),
            Tag::Int(read_int(saved, KEY_PROGRESS).max(0)),
        );
        fluids.push(tank_entry(
            LATEX,
            read_tank(saved, LATEX_TANK),
            UNIT_TANK_MB,
        ));
        fluids.push(tank_entry(
            WATER,
            read_tank(saved, WATER_TANK),
            UNIT_TANK_MB,
        ));
    }
    payload.insert("fluids".into(), Tag::List(fluids));
    if let Some(augments) = crate::carried_augment_slots(saved) {
        payload.insert("augments".into(), augments);
    }
    let mut patch = Compound::new();
    patch.insert("minecraft:block_entity_data".into(), Tag::Compound(payload));
    patch
}

/// The corrections a placed machine needs when its item carried values outside the capacities.
pub(crate) fn clamp_patch(block: &str, saved: &Compound) -> Option<Compound> {
    let mut patch = Compound::new();
    let mut changed = false;
    let energy = read_int(saved, KEY_ENERGY);
    let energy = energy.clamp(
        0,
        if block == EXTRACTOR_BLOCK {
            EXTRACTOR_CAPACITY_FE
        } else {
            UNIT_CAPACITY_FE
        },
    );
    if energy != read_int(saved, KEY_ENERGY) {
        write_int(&mut patch, KEY_ENERGY, energy);
        changed = true;
    }
    let latex = read_tank(saved, LATEX_TANK);
    let latex_capacity = if block == EXTRACTOR_BLOCK {
        EXTRACTOR_TANK_MB
    } else {
        UNIT_TANK_MB
    };
    if latex != latex.clamp(0, latex_capacity) {
        write_tank(&mut patch, LATEX_TANK, latex, latex_capacity, LATEX);
        changed = true;
    }
    if block == EXTRACTOR_BLOCK {
        let progress = read_progress(saved).unwrap_or(EXTRACTOR_MAX_PROGRESS);
        if progress.clamp(0, EXTRACTOR_MAX_PROGRESS) != progress {
            write_int(
                &mut patch,
                KEY_PROGRESS,
                progress.clamp(0, EXTRACTOR_MAX_PROGRESS),
            );
            changed = true;
        }
    } else {
        let water = read_tank(saved, WATER_TANK);
        if water != water.clamp(0, UNIT_TANK_MB) {
            write_tank(&mut patch, WATER_TANK, water, UNIT_TANK_MB, WATER);
            changed = true;
        }
        let progress = read_int(saved, KEY_PROGRESS).max(0);
        if progress != read_int(saved, KEY_PROGRESS) {
            write_int(&mut patch, KEY_PROGRESS, progress);
            changed = true;
        }
    }
    changed.then_some(patch)
}

// ---------------------------------------------------------------------------
// The commands
// ---------------------------------------------------------------------------

/// `/industrialforegoing energy`/`progress` on either machine: one int of its saved compound.
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

/// `/industrialforegoing info` for the extractor: buffer, tank, bar, the block behind, the recipe
/// it answers, the hits it has taken, the recipe table's size, the facing and the game time.
pub(crate) fn extractor_readout(host: &mut dyn ServerHost, pos: [i32; 3]) -> String {
    let Some(entity) = crate::our_entity(host, pos, EXTRACTOR_BLOCK) else {
        return format!("{pos:?} is not a fluid extractor.");
    };
    let Some(data) = entity.data(&*host) else {
        return format!("{pos:?} is not a mod block entity.");
    };
    let target = pointed(host, pos);
    let doc = entity_dimension(&*host, pos);
    let machine = Extractor::from_saved(
        read_int(&data, KEY_ENERGY),
        read_tank(&data, LATEX_TANK),
        read_progress(&data),
        hits(doc, target.unwrap_or(pos)),
    );
    let behind = target.and_then(|target| block_id(host, target));
    let table = recipes(host);
    let recipe = behind
        .as_deref()
        .and_then(|id| recipe_for(&table.rules, id))
        .map(|recipe| recipe.id.clone());
    let augments = crate::read_augments(&data);
    let facing = entity
        .level(&*host)
        .and_then(|level| level.block_property(host, pos, SUBFACING))
        .unwrap_or_else(|| "unknown".to_owned());
    let time = entity
        .level(&*host)
        .and_then(|level| level.game_time(host))
        .unwrap_or(0);
    format!(
        "Fluid Extractor at {pos:?}: {}/{} FE, latex {} {}/{} mB, progress {}/{}, log {}, recipe {}, hits {}/{}, recipes {}/{}, augments speed {} efficiency {:.2}, facing {facing}, t={time}",
        machine.energy,
        EXTRACTOR_CAPACITY_FE,
        tank_fluid(&data, LATEX_TANK),
        machine.latex_mb,
        EXTRACTOR_TANK_MB,
        machine.progress,
        EXTRACTOR_MAX_PROGRESS,
        behind.as_deref().unwrap_or("none"),
        recipe.as_deref().unwrap_or("none"),
        machine.hits,
        EXTRACTOR_HITS,
        table.rules.len(),
        table.fingerprint.len(),
        augments.speed,
        augments.efficiency,
    )
}

/// The dimension a machine stands in, as the static maps key it.
fn entity_dimension(host: &dyn ServerHost, _pos: [i32; 3]) -> &'static str {
    host.dimension()
}

/// `/industrialforegoing info` for the unit: buffer, both tanks, the bar, its output slots, the
/// latex tank as the declared capability reports it, the augments, the facing and the game time.
pub(crate) fn unit_readout(host: &mut dyn ServerHost, pos: [i32; 3]) -> String {
    let Some(entity) = crate::our_entity(host, pos, UNIT_BLOCK) else {
        return format!("{pos:?} is not a latex processing unit.");
    };
    let Some(data) = entity.data(&*host) else {
        return format!("{pos:?} is not a mod block entity.");
    };
    let machine = Unit::from_saved(
        read_int(&data, KEY_ENERGY),
        read_tank(&data, LATEX_TANK),
        read_tank(&data, WATER_TANK),
        read_int(&data, KEY_PROGRESS),
    );
    let augments = crate::read_augments(&data);
    let items: Vec<String> = (0..UNIT_SLOTS)
        .filter_map(|slot| {
            entity
                .item_slot(&*host, slot as u32)
                .ok()
                .flatten()
                .map(|stack| format!("{} x{}", stack.item().id(&*host), stack.count()))
        })
        .collect();
    let (capacity_fluid, capacity_mb) = capability_tank(host, pos, LATEX_TANK);
    let (water_fluid, water_mb) = capability_tank(host, pos, WATER_TANK);
    let facing = entity
        .level(&*host)
        .and_then(|level| level.block_property(host, pos, SUBFACING))
        .unwrap_or_else(|| "unknown".to_owned());
    let time = entity
        .level(&*host)
        .and_then(|level| level.game_time(host))
        .unwrap_or(0);
    format!(
        "Latex Processing Unit at {pos:?}: {}/{} FE, latex {} {}/{} mB, water {} {}/{} mB, progress {}/{}, output {}, capability {capacity_fluid} {capacity_mb} mB, water capability {water_fluid} {water_mb} mB, augments speed {} efficiency {:.2}, facing {facing}, t={time}",
        machine.energy,
        UNIT_CAPACITY_FE,
        tank_fluid(&data, LATEX_TANK),
        machine.latex_mb,
        UNIT_TANK_MB,
        tank_fluid(&data, WATER_TANK),
        machine.water_mb,
        UNIT_TANK_MB,
        machine.progress,
        Unit::bar_length(augments),
        if items.is_empty() {
            "empty".to_owned()
        } else {
            items.join(", ")
        },
        augments.speed,
        augments.efficiency,
    )
}

/// One tank as the declared fluid capability reports it.
fn capability_tank(host: &mut dyn ServerHost, pos: [i32; 3], tank: usize) -> (String, i32) {
    let Some(level) = host.level(host.dimension()) else {
        return ("none".to_owned(), 0);
    };
    let Some(tanks) = level.capability(host, &FLUIDS, pos, Some(READ_FACE)) else {
        return ("none".to_owned(), 0);
    };
    let Some(resource) = tanks.resource(host, tank) else {
        return ("empty".to_owned(), 0);
    };
    let name = match resource.fluid() {
        mod_api::FluidIdentity::Resolved(fluid) => fluid.id(host),
        mod_api::FluidIdentity::Unresolved(fluid) => fluid.id(host),
        _ => "unknown".to_owned(),
    };
    let amount = i32::try_from(tanks.amount(host, tank)).unwrap_or(i32::MAX);
    (name, amount)
}

/// `/industrialforegoing drain <pos> <amount>` on either machine: take the fluid out through the
/// declared tank capability (tank 0), which is what makes the tank a handler rather than a counter.
pub(crate) fn drain(host: &mut dyn ServerHost, pos: [i32; 3], block: &str, amount: i32) {
    let want = u32::try_from(amount.max(0)).unwrap_or(u32::MAX);
    let label = if block == EXTRACTOR_BLOCK {
        "Fluid Extractor"
    } else {
        "Latex Processing Unit"
    };
    let answer = host
        .transaction::<String, ()>(|host, tx| {
            let Some(level) = host.level(host.dimension()) else {
                return Ok(format!("{label} at {pos:?}: no drainable fluid."));
            };
            let Some(tanks) = level.capability(host, &FLUIDS, pos, Some(READ_FACE)) else {
                return Ok(format!("{label} at {pos:?}: no drainable fluid."));
            };
            let Some(resource) = tanks.resource(host, LATEX_TANK) else {
                return Ok(format!("{label} at {pos:?}: no drainable fluid."));
            };
            let taken = tanks.extract(host, tx, LATEX_TANK, &resource, want);
            if taken == 0 {
                return Ok(format!("{label} at {pos:?}: no drainable fluid."));
            }
            let name = match resource.fluid() {
                mod_api::FluidIdentity::Resolved(fluid) => fluid.id(host),
                mod_api::FluidIdentity::Unresolved(fluid) => fluid.id(host),
                _ => "an unknown fluid".to_owned(),
            };
            Ok(format!("{label} at {pos:?}: drained {taken} mB of {name}."))
        })
        .unwrap_or_else(|()| format!("{pos:?} is not one of this mod's machines."));
    crate::say(host, &answer);
}

/// `/industrialforegoing fill <pos> <fluid> <amount>`: insert through the declared tank
/// capability, tank by tank, and report what it took. This is the pin's own fill path — the one
/// `native_fluid_insert` (and `native_move_fluids`) uses — so a mod fluid is refused here, which
/// is what the report says about a fluid move.
pub(crate) fn fill(
    host: &mut dyn ServerHost,
    pos: [i32; 3],
    block: &str,
    fluid: &str,
    amount: i32,
) {
    let want = u32::try_from(amount.max(0)).unwrap_or(u32::MAX);
    let label = if block == EXTRACTOR_BLOCK {
        "Fluid Extractor"
    } else {
        "Latex Processing Unit"
    };
    let answer = host
        .transaction::<String, ()>(|host, tx| {
            let Some(level) = host.level(host.dimension()) else {
                return Ok(format!("{pos:?} is not one of this mod's machines."));
            };
            let Some(tanks) = level.capability(host, &FLUIDS, pos, Some(READ_FACE)) else {
                return Ok(format!("{label} at {pos:?}: no fillable tank."));
            };
            let Some(key) = host.fluid_key(fluid) else {
                return Ok(format!("{label} at {pos:?}: unknown fluid {fluid}."));
            };
            let stack = FluidStack::new(key, 0);
            let resource = FluidResource::of(&stack);
            let mut accepted = 0u32;
            let mut tried = Vec::new();
            for tank in 0..tanks.size(host) {
                let taken = tanks.insert(host, tx, tank, &resource, want - accepted);
                if taken > 0 {
                    accepted += taken;
                    tried.push(format!("tank {tank} took {taken} mB"));
                }
                if accepted >= want {
                    break;
                }
            }
            if accepted == 0 {
                return Ok(format!(
                    "{label} at {pos:?}: accepted 0 mB of {fluid} (the pin's fill takes water and lava only)."
                ));
            }
            Ok(format!(
                "{label} at {pos:?}: accepted {accepted} mB of {fluid} ({})",
                tried.join(", ")
            ))
        })
        .unwrap_or_else(|()| format!("{pos:?} is not one of this mod's machines."));
    crate::say(host, &answer);
}

/// `/industrialforegoing fluid <pos> <fluid> <amount>` on either machine: write the tank that
/// fluid belongs in (`_latex_`, `water`), through the same `merge_data` path as
/// `/data merge block` — the only route a mod fluid has at this pin.
pub(crate) fn set_fluid(
    host: &mut dyn ServerHost,
    pos: [i32; 3],
    block: &str,
    fluid: &str,
    amount: i32,
) {
    let Some(entity) = crate::our_entity(host, pos, block) else {
        crate::say(host, &format!("{pos:?} is not one of this mod's machines."));
        return;
    };
    let Some(saved) = entity.data(&*host) else {
        crate::say(host, &format!("{pos:?} is not a mod block entity."));
        return;
    };
    let (tank, capacity) = if block == EXTRACTOR_BLOCK {
        if fluid != LATEX {
            crate::say(
                host,
                &format!("Fluid Extractor at {pos:?}: its only tank holds {LATEX}."),
            );
            return;
        }
        (LATEX_TANK, EXTRACTOR_TANK_MB)
    } else if fluid == LATEX {
        (LATEX_TANK, UNIT_TANK_MB)
    } else if fluid == WATER {
        (WATER_TANK, UNIT_TANK_MB)
    } else {
        crate::say(
            host,
            &format!("Latex Processing Unit at {pos:?}: its tanks hold {LATEX} and {WATER}."),
        );
        return;
    };
    // Seed the patch with the entity's own list: a merge replaces a list, so writing one tank
    // from an empty compound would drop the other (the unit has two).
    let mut patch = Compound::new();
    if let Some(fluids) = saved.get(&"fluids".into()) {
        patch.insert("fluids".into(), fluids.clone());
    }
    write_tank(&mut patch, tank, amount, capacity, fluid);
    let changed = entity.merge_data(host, patch).unwrap_or(false);
    let amount = amount.clamp(0, capacity);
    crate::say(
        host,
        &if changed {
            format!("{block} at {pos:?}: tank {tank} {fluid} {amount}/{capacity} mB.")
        } else {
            format!("{pos:?} is not a mod block entity.")
        },
    );
}

/// `/industrialforegoing extraction <pos>` (read) or `… <hits>` (set): the shared hit counter of
/// the log the machine works on, so the eighth-hit path is reachable in a bounded run.
pub(crate) fn extraction(host: &mut dyn ServerHost, pos: [i32; 3], block: &str, hits: Option<i32>) {
    let label = if block == EXTRACTOR_BLOCK {
        "Fluid Extractor"
    } else {
        "Latex Processing Unit"
    };
    let Some(target) = (block == EXTRACTOR_BLOCK)
        .then(|| pointed(host, pos))
        .flatten()
    else {
        crate::say(host, &format!("{label} at {pos:?}: no block behind it."));
        return;
    };
    let dimension = host.dimension();
    if let Some(value) = hits {
        let value = value.clamp(0, EXTRACTOR_HITS);
        set_hits(dimension, target, value);
    }
    crate::say(
        host,
        &format!(
            "{label} at {pos:?}: the log at {target:?} in {dimension} has taken {}/{} hits.",
            hits_of(dimension, target),
            EXTRACTOR_HITS
        ),
    );
}

/// [`hits`] under its command name.
fn hits_of(dimension: &'static str, pos: [i32; 3]) -> i32 {
    hits(dimension, pos)
}

/// `/industrialforegoing roll <pos> <value>`: pin this machine's next `breakChance` roll (`0` always
/// hits, `1` never does), so the eighth-hit path is observable without 800 operations.
pub(crate) fn roll(host: &mut dyn ServerHost, pos: [i32; 3], block: &str, value: f32) {
    let label = if block == EXTRACTOR_BLOCK {
        "Fluid Extractor"
    } else {
        "Latex Processing Unit"
    };
    let Some(target) = (block == EXTRACTOR_BLOCK)
        .then(|| pointed(host, pos))
        .flatten()
    else {
        crate::say(host, &format!("{label} at {pos:?}: no block behind it."));
        return;
    };
    let dimension = host.dimension();
    pin_roll(dimension, target, value);
    crate::say(
        host,
        &format!(
            "{label} at {pos:?}: the next operation's roll on the log at {target:?} in {dimension} is pinned to {:.3}.",
            value.clamp(0.0, 1.0)
        ),
    );
}

/// The block entity type the declarations made, for the augment seat.
pub(crate) fn kind_of(entity: mod_api::BlockEntityRef, host: &dyn Host) -> bool {
    let Some(kind) = entity.kind(host) else {
        return false;
    };
    EXTRACTOR_KIND.get() == Some(&kind) || UNIT_KIND.get() == Some(&kind)
}

/// The item `block_drops` may write to, by block id.
pub(crate) fn item(block: &str) -> Option<ItemKey> {
    if block == EXTRACTOR_BLOCK {
        EXTRACTOR_ITEM.get().copied()
    } else {
        UNIT_ITEM.get().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_extractor_tank_is_drain_only_and_a_thousand_millibuckets() {
        let tanks = extractor_tanks();
        assert_eq!(tanks.tanks, vec![1_000]);
        assert!(tanks.inputs.is_empty(), "the machine fills its own tank");
        assert_eq!(tanks.outputs, vec![0]);
    }

    #[test]
    fn the_unit_fills_only_its_water_tank() {
        let tanks = unit_tanks();
        assert_eq!(tanks.tanks, vec![16_000, 16_000]);
        assert_eq!(
            tanks.inputs,
            vec![WATER_TANK],
            "a mod fluid has no fill path, and the pin cannot filter a tank by fluid"
        );
        assert_eq!(tanks.outputs, vec![0, 1]);
    }

    #[test]
    fn the_unit_declares_nine_output_slots() {
        let container = unit_container();
        assert_eq!(container.slots, 9);
        assert_eq!(container.outputs, (0..9).collect::<Vec<usize>>());
        assert!(
            container.inputs.is_empty(),
            "nothing goes in but the fluids"
        );
        assert!(container.faces.iter().all(|face| face.len() == 9));
    }

    #[test]
    fn the_block_behind_a_machine_is_opposite_its_facing() {
        assert_eq!(behind("north"), Some([0, 0, 1]));
        assert_eq!(behind("south"), Some([0, 0, -1]));
        assert_eq!(behind("east"), Some([-1, 0, 0]));
        assert_eq!(behind("west"), Some([1, 0, 0]));
        assert_eq!(behind("up"), None);
    }

    #[test]
    fn a_tank_round_trips_through_the_saved_compound() {
        let mut nbt = Compound::new();
        assert_eq!(read_tank(&nbt, LATEX_TANK), 0);
        write_tank(&mut nbt, LATEX_TANK, 750, UNIT_TANK_MB, LATEX);
        write_tank(&mut nbt, WATER_TANK, 500, UNIT_TANK_MB, WATER);
        assert_eq!(read_tank(&nbt, LATEX_TANK), 750);
        assert_eq!(read_tank(&nbt, WATER_TANK), 500);
        assert_eq!(tank_fluid(&nbt, LATEX_TANK), LATEX);
        assert_eq!(tank_fluid(&nbt, WATER_TANK), WATER);
        write_tank(&mut nbt, LATEX_TANK, 0, UNIT_TANK_MB, LATEX);
        assert_eq!(read_tank(&nbt, LATEX_TANK), 0);
        assert_eq!(
            tank_fluid(&nbt, LATEX_TANK),
            "minecraft:empty",
            "the water tank keeps its place"
        );
        assert_eq!(read_tank(&nbt, WATER_TANK), 500);
    }

    #[test]
    fn a_tank_never_saves_more_than_it_holds() {
        let mut nbt = Compound::new();
        write_tank(&mut nbt, LATEX_TANK, 99_999, EXTRACTOR_TANK_MB, LATEX);
        assert_eq!(read_tank(&nbt, LATEX_TANK), EXTRACTOR_TANK_MB);
    }

    #[test]
    fn the_extractor_carries_its_tank_and_the_unit_its_fluids() {
        let mut nbt = Compound::new();
        assert!(!has_contents(EXTRACTOR_BLOCK, &nbt));
        write_tank(&mut nbt, LATEX_TANK, 6, EXTRACTOR_TANK_MB, LATEX);
        assert!(has_contents(EXTRACTOR_BLOCK, &nbt));
        let mut nbt = Compound::new();
        write_tank(&mut nbt, WATER_TANK, 500, UNIT_TANK_MB, WATER);
        assert!(has_contents(UNIT_BLOCK, &nbt));
        assert!(
            !has_contents(EXTRACTOR_BLOCK, &nbt),
            "the extractor has no water tank"
        );
    }

    #[test]
    fn a_placed_machine_clamps_what_its_item_carried() {
        let mut nbt = Compound::new();
        write_int(&mut nbt, KEY_ENERGY, 50_000);
        // A hand-written amount, not `write_tank`: that one clamps as it saves.
        let mut entry = Compound::new();
        entry.insert("Fluid".into(), Tag::String(LATEX.into()));
        entry.insert("Amount".into(), Tag::Int(99_999));
        nbt.insert("fluids".into(), Tag::List(vec![Tag::Compound(entry)]));
        write_int(&mut nbt, KEY_PROGRESS, 999);
        let patch = clamp_patch(EXTRACTOR_BLOCK, &nbt).expect("out-of-range values are corrected");
        assert_eq!(read_int(&patch, KEY_ENERGY), EXTRACTOR_CAPACITY_FE);
        assert_eq!(read_tank(&patch, LATEX_TANK), EXTRACTOR_TANK_MB);
        assert_eq!(read_progress(&patch), Some(EXTRACTOR_MAX_PROGRESS));
        let mut fine = Compound::new();
        write_int(&mut fine, KEY_ENERGY, 4_000);
        write_tank(&mut fine, LATEX_TANK, 6, EXTRACTOR_TANK_MB, LATEX);
        write_int(&mut fine, KEY_PROGRESS, 40);
        assert!(
            clamp_patch(EXTRACTOR_BLOCK, &fine).is_none(),
            "a machine in range is left alone"
        );
        let mut unit = Compound::new();
        let mut water = Compound::new();
        water.insert("Fluid".into(), Tag::String(WATER.into()));
        water.insert("Amount".into(), Tag::Int(99_999));
        unit.insert(
            "fluids".into(),
            Tag::List(vec![Tag::Compound(Compound::new()), Tag::Compound(water)]),
        );
        let patch = clamp_patch(UNIT_BLOCK, &unit).expect("the water tank is clamped");
        assert_eq!(read_tank(&patch, WATER_TANK), UNIT_TANK_MB);
    }

    #[test]
    fn the_carried_patch_names_each_entity_type() {
        let mut nbt = Compound::new();
        write_int(&mut nbt, KEY_ENERGY, 500);
        let patch = carried_patch(EXTRACTOR_BLOCK, &nbt);
        let Some(Tag::Compound(payload)) = patch.get(&"minecraft:block_entity_data".into()) else {
            panic!("no block entity data in the patch");
        };
        assert_eq!(
            payload.get(&"id".into()),
            Some(&Tag::String(EXTRACTOR_BLOCK.into()))
        );
        let patch = carried_patch(UNIT_BLOCK, &nbt);
        let Some(Tag::Compound(payload)) = patch.get(&"minecraft:block_entity_data".into()) else {
            panic!("no block entity data in the patch");
        };
        assert_eq!(
            payload.get(&"id".into()),
            Some(&Tag::String(UNIT_BLOCK.into()))
        );
    }

    #[test]
    fn the_hit_counter_is_shared_by_position() {
        set_hits("minecraft:the_end", [200, 101, 0], 3);
        assert_eq!(hits("minecraft:the_end", [200, 101, 0]), 3);
        assert_eq!(hits("minecraft:the_end", [201, 101, 0]), 0);
        set_hits("minecraft:the_end", [200, 101, 0], 0);
        assert_eq!(hits("minecraft:the_end", [200, 101, 0]), 0);
    }

    /// `§9m`: the engine spills a mod container's slots itself, so a carried payload that also
    /// held `Items` would duplicate every stack when the item is placed back.
    #[test]
    fn a_carried_payload_is_an_allowlist_without_its_container() {
        let mut nbt = Compound::new();
        write_int(&mut nbt, KEY_ENERGY, 4_000);
        write_int(&mut nbt, KEY_PROGRESS, 40);
        write_tank(&mut nbt, LATEX_TANK, 750, UNIT_TANK_MB, LATEX);
        write_tank(&mut nbt, WATER_TANK, 500, UNIT_TANK_MB, WATER);
        nbt.insert("Items".into(), Tag::List(Vec::new()));
        nbt.insert("Stray".into(), Tag::Int(9));
        for block in [UNIT_BLOCK, EXTRACTOR_BLOCK] {
            let patch = carried_patch(block, &nbt);
            let Some(Tag::Compound(payload)) = patch.get(&"minecraft:block_entity_data".into())
            else {
                panic!("no block entity data in the patch");
            };
            assert!(
                payload.get(&"Items".into()).is_none(),
                "{block}: the container's contents ride on the engine's own spill, never twice"
            );
            assert!(
                payload.get(&"Stray".into()).is_none(),
                "{block}: an allowlist carries the machine's own keys, not the snapshot's"
            );
            assert_eq!(read_int(payload, KEY_ENERGY), 4_000);
            assert_eq!(read_tank(payload, LATEX_TANK), 750);
        }
    }
}
