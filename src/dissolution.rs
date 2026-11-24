//! The game side of the dissolution chamber (`ModuleCore.DISSOLUTION_CHAMBER`): its
//! declarations, its recipes read back from this package's own data, and the block entity
//! Part that runs them.
//!
//! Upstream's `DissolutionChamberTile` is an `IndustrialProcessingTile` over Titanium's
//! `ProgressBarComponent`: eight one-item input slots, three output slots, an 8,000 mB input
//! tank, a 16,000 mB output tank and a 10,000 FE buffer, 90 FE a tick. The rules are
//! [`crate::chamber`]; this file reads and writes the game.
//!
//! # The recipe type
//!
//! The machine's recipes are files of its own type (`industrialforegoing:dissolution_chamber`)
//! under `data/<namespace>/recipe/`, which this package carries. The type is declared with
//! `Registrar::register_recipe_type` and the files are read back through
//! [`Host::recipes_of_type`](mod_api::Host::recipes_of_type), so a data pack can add, override
//! or remove an operation, as upstream's `RecipeManager` allows. The pin's
//! `CraftingRecipeKind`/`register_crafting_recipe_serializer` is the **3×3 grid** kind (it
//! registers under `minecraft:crafting`); a machine's own type is API 36's read-only one.
//!
//! # The fluids
//!
//! `industrialforegoing:latex`, `:pink_slime` and `:ether_gas` are declared fluid keys, and the
//! two tanks' contents live in the block entity's own `fluids` compound — the same shape the
//! pin reads for a declared tank. `native_fluid_insert` takes only water and lava
//! (`machines/native_fluids.rs`), so the machine is filled through
//! `/industrialforegoing fluid`, which merges the compound as `/data merge block` does.

use std::sync::{Arc, Mutex, OnceLock};

use mod_api::{
    self, BlockEntityPart, BlockEntityTickEvent, BlockEntityTicksEvent, BlockPart, BlockState,
    Compound, Direction, FLUIDS, FluidIdentity, Host, ItemKey, ItemPart, ItemStack, LevelHost,
    ModContainer, ModEnergyBuffer, ModFluidTanks, OnBlockPlacedByEvent, Registrar, ServerHost,
    StateForPlacementEvent, Tag, UseItemOnEvent,
};

use crate::chamber::{
    CAPACITY_FE, ChamberRecipe, INPUT_SLOTS, INPUT_TANK_MB, Ingredient, Machine, OUTPUT_TANK_MB,
    RECIPE_TYPE, SLOTS, Step, Tank, recipe_index,
};

/// The block's full id (`ModuleCore.DISSOLUTION_CHAMBER`), as `Level::block` names it.
pub const BLOCK: &str = "industrialforegoing:dissolution_chamber";
/// The block entity type, a bare name so the mod's namespace qualifies it.
pub const ENTITY: &str = "dissolution_chamber";
/// The block entity type's registry id, which `minecraft:block_entity_data` tags its payload
/// with.
pub const ENTITY_ID: &str = "industrialforegoing:dissolution_chamber";
/// The state property Titanium's `RotationType.FOUR_WAY` sets (`DissolutionChamberBlock`).
pub const SUBFACING: &str = "subfacing";
/// The saved keys, in the same shape the water condensator uses: the buffer's amount (the
/// energy declaration's own), the bar and the two augments.
const KEY_ENERGY: &str = "energy";
const KEY_PROGRESS: &str = "progress";
/// The fluids this machine's carried recipes name, declared as fluid keys
/// (`ModuleCore.LATEX`/`PINK_SLIME`/`ETHER_GAS`/`MEAT`).
pub const DECLARED_FLUIDS: [&str; 7] = [
    "industrialforegoing:latex",
    "industrialforegoing:pink_slime",
    "industrialforegoing:ether_gas",
    "industrialforegoing:meat",
    "industrialforegoing:essence",
    // The sewer/composter's fluid (slice 15): declared so a tank can hold it and the capability
    // can name it back (an undeclared id reads `empty` with "unrecognized live fluid id").
    "industrialforegoing:sewage",
    // The Animal Rancher's milk (slice 13): 26.3's registry has empty/water/lava only, where the
    // Java pack drains NeoForge's `minecraft:milk`.
    "industrialforegoing:milk",
];
/// The side the tanks are read through (all six reach them).
const READ_FACE: Direction = Direction::Up;

/// The block entity type and the item, resolved once in `register`: `block_drops` and the
/// command path run with a read-only level host, which cannot name either.
static KIND: OnceLock<mod_api::BlockEntityTypeKey> = OnceLock::new();
static ITEM: OnceLock<ItemKey> = OnceLock::new();

/// The decoded recipes and the identity of the table they came from.
#[derive(Debug)]
pub(crate) struct Table {
    /// `(id, the file's own allocation)`: the identity of the recipe table, so a `/reload`
    /// that changed it rebuilds the decode, and every tick in between reads this instead of
    /// crossing into the game.
    fingerprint: Vec<(String, usize)>,
    /// The decoded rules, in file order.
    pub(crate) rules: Vec<ChamberRecipe>,
    /// The product each rule makes, resolved by the game (components included).
    products: Vec<Option<ItemStack>>,
}

/// The recipe table, as last decoded.
static TABLE: Mutex<Option<Arc<Table>>> = Mutex::new(None);

// ---------------------------------------------------------------------------
// The declarations (`ModuleCore` + `DissolutionChamberTile`'s components)
// ---------------------------------------------------------------------------

/// Declare everything the game answers for the chamber: the block entity type, its eight
/// input and three output slots, its two tanks, its 10,000 FE buffer, its Part and the Part on
/// its item.
///
/// Called from [`crate::Mod::register`]; the fluid keys and the recipe type are declared there
/// beside it.
pub(crate) fn declare(r: &mut dyn Registrar) {
    let block = r.block_key(BLOCK);
    let item = r.item_key(BLOCK);
    let _ = ITEM.set(item);
    r.item_part(item, ChamberItem);
    let kind = r
        .block_entity_type(ENTITY)
        .blocks(&["dissolution_chamber"])
        .build();
    let _ = KIND.set(kind);
    r.block_entity_part(&kind, Chamber)
        .expect("the chamber's Part attaches to the type this mod just declared");
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

/// The two tanks: the input (8,000 mB, fillable and drainable as upstream's sided tank is) and
/// the output (16,000 mB, `Action.DRAIN` — the machine fills it itself). Every face reaches
/// both, and their contents are the entity's own `fluids` compound.
#[must_use]
pub fn tanks() -> ModFluidTanks {
    let both = vec![0usize, 1];
    let mut tanks = ModFluidTanks::default();
    tanks.tanks = vec![INPUT_TANK_MB, OUTPUT_TANK_MB];
    tanks.faces = [
        both.clone(),
        both.clone(),
        both.clone(),
        both.clone(),
        both.clone(),
        both,
    ];
    tanks.inputs = vec![0];
    tanks.outputs = vec![0, 1];
    tanks
}

/// The eleven slots: `SidedInventoryComponent("input", …, 8, 0)` (one item per slot, no
/// output filter) then `SidedInventoryComponent("output", …, 3, 2)`. Every face reaches them;
/// a pipe may fill the inputs and take from the outputs, as upstream's filters say.
#[must_use]
pub fn container() -> ModContainer {
    let all: Vec<usize> = (0..SLOTS).collect();
    let mut container = ModContainer {
        slots: SLOTS,
        ..ModContainer::default()
    };
    container.faces = [
        all.clone(),
        all.clone(),
        all.clone(),
        all.clone(),
        all.clone(),
        all,
    ];
    container.inputs = (0..INPUT_SLOTS).collect();
    container.outputs = (INPUT_SLOTS..SLOTS).collect();
    container
}

// ---------------------------------------------------------------------------
// The recipe table
// ---------------------------------------------------------------------------

/// The recipes in force, decoded once per table: the page of rules this machine reads.
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
    let table = Arc::new(decode(&*host, &listed, fingerprint));
    if let Ok(mut cached) = TABLE.lock() {
        *cached = Some(Arc::clone(&table));
    }
    table
}

/// Decode every file of the type; a file this machine cannot read is skipped (a pack may add
/// its own operations), and the count is what `/industrialforegoing info` reports.
fn decode(host: &dyn Host, listed: &[mod_api::Recipe], fingerprint: Vec<(String, usize)>) -> Table {
    let mut rules = Vec::new();
    let mut products = Vec::new();
    for recipe in listed {
        let Some((rule, product)) = one(host, &recipe.id, &recipe.json) else {
            continue;
        };
        rules.push(rule);
        products.push(product);
    }
    Table {
        fingerprint,
        rules,
        products,
    }
}

/// One recipe file (`DissolutionChamberRecipe.CODEC`): `input` (0–8 ingredients),
/// `inputFluid` (`SizedFluidIngredient.FLAT_CODEC`: `fluid` + `amount`), `processingTime`,
/// `output` and the optional `outputFluid` (`FluidStack.CODEC`).
fn one(
    host: &dyn Host,
    id: &str,
    value: &mod_api::serde_json::Value,
) -> Option<(ChamberRecipe, Option<ItemStack>)> {
    use mod_api::serde_json::Value;

    let ticks = value
        .get("processingTime")
        .and_then(Value::as_i64)
        .and_then(|ticks| i32::try_from(ticks).ok())?;
    if ticks <= 0 {
        return None;
    }
    let input = value.get("input").and_then(Value::as_array)?;
    if input.len() > INPUT_SLOTS {
        return None;
    }
    let mut ingredients = Vec::with_capacity(input.len());
    for cell in input {
        let text = mod_api::serde_json::to_string(cell).ok()?;
        let ingredient = host.recipe_ingredient(&text).ok().flatten()?;
        let keys = ingredient.item_keys(host).ok()?;
        let items: Vec<String> = keys.iter().map(|key| key.id(host)).collect();
        if items.is_empty() {
            // A tag nobody fills: the recipe could never match, so it is left out of the
            // table (the same outcome as the game dropping a recipe it cannot read).
            return None;
        }
        ingredients.push(Ingredient { items });
    }
    let fluid = fluid_of(value.get("inputFluid")?)?;
    let product = value.get("output").and_then(|output| {
        let text = mod_api::serde_json::to_string(output).ok()?;
        host.recipe_result(&text).ok().flatten()
    });
    let rule = ChamberRecipe {
        id: id.to_owned(),
        input: ingredients,
        fluid,
        ticks,
        output: product.as_ref().map(|stack| crate::chamber::Product {
            id: stack.item().id(host),
            count: stack.count(),
        }),
        output_fluid: value.get("outputFluid").and_then(fluid_of),
    };
    Some((rule, product))
}

/// A `{fluid|id, amount}` object as a tank.
fn fluid_of(value: &mod_api::serde_json::Value) -> Option<Tank> {
    use mod_api::serde_json::Value;

    let id = value
        .get("fluid")
        .or_else(|| value.get("id"))
        .and_then(Value::as_str)?;
    let mb = value
        .get("amount")
        .and_then(Value::as_i64)
        .and_then(|amount| i32::try_from(amount).ok())?;
    Some(Tank::new(id, mb))
}

// ---------------------------------------------------------------------------
// The Part
// ---------------------------------------------------------------------------

/// The chamber's block entity: its tick comes from the declarations in [`declare`].
pub(crate) struct Chamber;

#[mod_api::horizon_part]
impl BlockPart for Chamber {
    /// `RotationType.FOUR_WAY`: a placed chamber faces its placer.
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

    /// `AddonItem.useOn`: right-clicking the chamber with an augment seats it, as on the water
    /// condensator — the write is queued for the level's `level_tick_post`.
    fn use_item_on(
        &self,
        host: &mut dyn ServerHost,
        event: &mut UseItemOnEvent<'_>,
    ) -> Option<mod_api::InteractionResult> {
        let click = event.click;
        let stack = click.stack?;
        let component = crate::augment_component(&*host, stack)?;
        let augment = crate::decode_augment(&component)?;
        crate::queue_seatings(vec![crate::Seating {
            player: click.player,
            dimension: click.dimension.key().id(),
            pos: click.pos,
            hand: click.hand,
            stack: stack.clone(),
            kind: augment.kind,
            component,
        }]);
        Some(mod_api::InteractionResult::Consume)
    }
}

#[mod_api::horizon_part]
impl BlockEntityPart for Chamber {
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    /// One server tick: decode the table if it changed, match the input slots and tank, run the
    /// bar, and write the machine back. Every write lands in the entity view the tick owns, so
    /// the default `Changed` reply keeps it and marks the chunk.
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let entity = &mut *event.entity;
        // The view's slot list follows the saved `Items` list, so a container a break-and-replace
        // or a `/item replace … with air` left shorter than the declaration comes back short: pad
        // it instead of skipping the machine (measured 2026-11-23 — a chamber that had completed
        // one recipe stopped working because `items` held fewer slots than it declares).
        if entity.items.len() < SLOTS {
            entity.items.resize(SLOTS, None);
        }
        let stored = read_tanks(&entity.nbt);
        let mut machine = Machine::from_saved(
            read_int(&entity.nbt, KEY_ENERGY),
            read_int(&entity.nbt, KEY_PROGRESS),
            stored[0].clone(),
            stored[1].clone(),
        );
        let held: Vec<Option<String>> = entity.items[..INPUT_SLOTS]
            .iter()
            .map(|slot| slot.as_ref().map(|stack| stack.item().id(&*host)))
            .collect();
        let table = recipes(&mut *host);
        let index = recipe_index(&table.rules, &held, &machine.input);
        let accepts = index.is_some_and(|index| {
            let rule = &table.rules[index];
            room_for(
                &*host,
                &entity.items[INPUT_SLOTS..],
                table.products[index].as_ref(),
            ) && output_room(&machine, rule)
        });
        let step = machine.tick(
            index.map(|index| &table.rules[index]),
            crate::read_augments(&entity.nbt),
            accepts,
        );
        if matches!(step, Step::Completed(_)) {
            // `onFinish`: one out of every input slot, the product into the outputs.
            for slot in &mut entity.items[..INPUT_SLOTS] {
                if let Some(stack) = slot.as_ref() {
                    *slot = stack.clone().shrink(1);
                }
            }
            if let Some(index) = index
                && let Some(product) = table.products[index].clone()
            {
                put(&*host, &mut entity.items[INPUT_SLOTS..], product);
            }
        }
        write_int(&mut entity.nbt, KEY_ENERGY, machine.energy);
        write_int(&mut entity.nbt, KEY_PROGRESS, machine.progress);
        write_tanks(&mut entity.nbt, &[machine.input, machine.output]);
    }
}

/// The chamber's item: the drop carries the machine's FE, fluids and bar, and placing it
/// clamps them (slice 2's route for a mod block entity).
pub(crate) struct ChamberItem;

#[mod_api::horizon_part]
impl ItemPart for ChamberItem {
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
        let Some(patch) = clamp_patch(&saved) else {
            return;
        };
        let _ = entity.merge_data(host, patch);
    }
}

// ---------------------------------------------------------------------------
// Room, insertion and the saved compound
// ---------------------------------------------------------------------------

/// Whether `outputs` can take `product`, as `ItemHandlerHelper.insertItem(…, true)` asks.
fn room_for(host: &dyn Host, outputs: &[Option<ItemStack>], product: Option<&ItemStack>) -> bool {
    let Some(product) = product else {
        return true;
    };
    let max = host.max_stack_size(product.item()).unwrap_or(64);
    outputs.iter().any(|slot| match slot {
        None => true,
        Some(stack) => stack.item() == product.item() && stack.count() + product.count() <= max,
    })
}

/// Whether the output tank can take the rule's `outputFluid` (`canIncrease`'s second half).
fn output_room(machine: &Machine, rule: &ChamberRecipe) -> bool {
    let Some(fluid) = &rule.output_fluid else {
        return true;
    };
    machine.output.room(OUTPUT_TANK_MB) >= fluid.mb
        && (machine.output.is_empty() || machine.output.fluid == fluid.fluid)
}

/// Insert `product` into the first output slot that holds it, else the first empty one
/// (`ItemHandlerHelper.insertItem`).
pub(crate) fn put(host: &dyn Host, outputs: &mut [Option<ItemStack>], product: ItemStack) -> bool {
    let max = host.max_stack_size(product.item()).unwrap_or(64);
    insert(outputs, product, max).is_none()
}

/// Put a product into the first slot that can take it, and return what did not fit.
///
/// A slot may take the product only when the two stacks are **stackable** — the same item,
/// the same damage and the same component patch — and their counts fit the item's maximum.
/// Growing the slot's own stack (instead of building a new one from the id and the count)
/// is what keeps its damage and components. A product that fits nowhere comes back, so the
/// caller gives it to the world instead of losing it.
#[must_use]
pub(crate) fn insert(
    outputs: &mut [Option<ItemStack>],
    product: ItemStack,
    max: u32,
) -> Option<ItemStack> {
    let same = outputs.iter().position(|slot| {
        slot.as_ref().is_some_and(|stack| {
            stack.is_stackable_with(&product) && stack.count() + product.count() <= max
        })
    });
    if let Some(index) = same
        && let Some(stack) = outputs[index].as_mut()
        && stack.grow(product.count()).is_ok()
    {
        return None;
    }
    if let Some(index) = outputs.iter().position(Option::is_none) {
        outputs[index] = Some(product);
        return None;
    }
    Some(product)
}

/// The two tanks from the entity's `fluids`: the pin's shape (one `{Fluid, Amount}` per tank),
/// padded and read even when the fluid is this mod's own (which `native_fluid_insert` refuses
/// to write, so the port writes the compound itself).
fn read_tanks(nbt: &Compound) -> [Tank; 2] {
    let mut out = [Tank::EMPTY, Tank::EMPTY];
    let Some(Tag::List(entries)) = nbt.get(&"fluids".into()) else {
        return out;
    };
    for (tank, entry) in out.iter_mut().zip(entries) {
        let Tag::Compound(entry) = entry else {
            continue;
        };
        let fluid = match entry.get(&"Fluid".into()) {
            Some(Tag::String(fluid)) if fluid.to_string_lossy() != "minecraft:empty" => {
                fluid.to_string_lossy()
            }
            _ => continue,
        };
        let mb = match entry.get(&"Amount".into()) {
            Some(Tag::Int(amount)) => *amount,
            _ => 0,
        };
        *tank = Tank::new(fluid, mb);
    }
    out
}

/// The tanks back in the pin's shape, both entries kept in place.
fn write_tanks(nbt: &mut Compound, tanks: &[Tank; 2]) {
    let entries = tanks
        .iter()
        .map(|tank| {
            let mut entry = Compound::new();
            entry.insert(
                "Fluid".into(),
                Tag::String(if tank.is_empty() {
                    "minecraft:empty".into()
                } else {
                    tank.fluid.as_str().into()
                }),
            );
            entry.insert("Amount".into(), Tag::Int(tank.mb.max(0)));
            Tag::Compound(entry)
        })
        .collect();
    nbt.insert("fluids".into(), Tag::List(entries));
}

fn read_int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(value)) => *value,
        _ => 0,
    }
}

fn write_int(nbt: &mut Compound, key: &str, value: i32) {
    nbt.insert(key.into(), Tag::Int(value));
}

/// Whether the chamber holds anything worth carrying on its item.
pub(crate) fn has_contents(saved: &Compound) -> bool {
    let [input, output] = read_tanks(saved);
    read_int(saved, KEY_ENERGY) > 0
        || read_int(saved, KEY_PROGRESS) > 0
        || !input.is_empty()
        || !output.is_empty()
}

/// The corrections a placed chamber needs when its item carried values outside the capacities.
pub(crate) fn clamp_patch(saved: &Compound) -> Option<Compound> {
    let machine = Machine::from_saved(
        read_int(saved, KEY_ENERGY),
        read_int(saved, KEY_PROGRESS),
        read_tanks(saved)[0].clone(),
        read_tanks(saved)[1].clone(),
    );
    let [input, output] = read_tanks(saved);
    let mut patch = Compound::new();
    let mut changed = false;
    if machine.energy != read_int(saved, KEY_ENERGY) {
        write_int(&mut patch, KEY_ENERGY, machine.energy);
        changed = true;
    }
    if machine.progress != read_int(saved, KEY_PROGRESS).max(0) {
        write_int(&mut patch, KEY_PROGRESS, machine.progress);
        changed = true;
    }
    if machine.input != input || machine.output != output {
        write_tanks(&mut patch, &[machine.input, machine.output]);
        changed = true;
    }
    changed.then_some(patch)
}

/// The `minecraft:block_entity_data` payload a broken chamber's item carries.
/// The `minecraft:block_entity_data` payload a broken chamber's item carries: a **fresh
/// allowlist** of the machine's own keys, never the snapshot. `§9m`: the engine spills the
/// chamber's slots itself, so a payload holding `Items` would duplicate every stack.
pub(crate) fn carried_patch(saved: &Compound) -> Compound {
    let [input, output] = read_tanks(saved);
    let mut payload = Compound::new();
    payload.insert("id".into(), Tag::String(ENTITY_ID.into()));
    payload.insert(
        KEY_ENERGY.into(),
        Tag::Int(read_int(saved, KEY_ENERGY).clamp(0, CAPACITY_FE)),
    );
    payload.insert(
        KEY_PROGRESS.into(),
        Tag::Int(read_int(saved, KEY_PROGRESS).max(0)),
    );
    let mut body = Compound::new();
    write_tanks(&mut body, &[input, output]);
    if let Some(Tag::List(fluids)) = body.get(&"fluids".into()) {
        payload.insert("fluids".into(), Tag::List(fluids.clone()));
    }
    if let Some(augments) = crate::carried_augment_slots(saved) {
        payload.insert("augments".into(), augments);
    }
    let mut patch = Compound::new();
    patch.insert("minecraft:block_entity_data".into(), Tag::Compound(payload));
    patch
}

// ---------------------------------------------------------------------------
// The commands
// ---------------------------------------------------------------------------

/// `/industrialforegoing info` for the chamber: the buffer, both tanks, the bar, the matched
/// recipe, the table's size, the facing and the game time.
pub(crate) fn readout(host: &mut dyn ServerHost, pos: [i32; 3]) -> String {
    let Some(entity) = crate::our_entity(host, pos, BLOCK) else {
        return format!("{pos:?} is not a dissolution chamber.");
    };
    let Some(data) = entity.data(&*host) else {
        return format!("{pos:?} is not a mod block entity.");
    };
    let [input, output] = read_tanks(&data);
    let machine = Machine::from_saved(
        read_int(&data, KEY_ENERGY),
        read_int(&data, KEY_PROGRESS),
        input,
        output,
    );
    let held: Vec<Option<String>> = (0..INPUT_SLOTS)
        .map(|slot| {
            entity
                .item_slot(&*host, slot as u32)
                .ok()
                .flatten()
                .map(|stack| stack.item().id(&*host))
        })
        .collect();
    let table = recipes(host);
    let index = recipe_index(&table.rules, &held, &machine.input);
    let recipe = index.map_or_else(|| "none".to_owned(), |index| table.rules[index].id.clone());
    let augments = crate::read_augments(&data);
    let length = index.map_or(0, |index| {
        Machine::bar_length(&table.rules[index], augments)
    });
    let (capacity_fluid, capacity_mb) = capability_tank(host, pos);
    let facing = entity
        .level(&*host)
        .and_then(|level| level.block_property(host, pos, SUBFACING))
        .unwrap_or_else(|| "unknown".to_owned());
    let time = entity
        .level(&*host)
        .and_then(|level| level.game_time(host))
        .unwrap_or(0);
    format!(
        "Dissolution Chamber at {pos:?}: {}/{} FE, input {} {}/{} mB, output {} {}/{} mB, progress {}/{}, recipe {recipe}, items {}, recipes {}/{}, capability {capacity_fluid} {capacity_mb} mB, augments speed {} efficiency {:.2}, facing {facing}, t={time}",
        machine.energy,
        CAPACITY_FE,
        fluid_name(&machine.input),
        machine.input.mb,
        INPUT_TANK_MB,
        fluid_name(&machine.output),
        machine.output.mb,
        OUTPUT_TANK_MB,
        machine.progress,
        length,
        held.iter().filter(|slot| slot.is_some()).count(),
        table.rules.len(),
        table.fingerprint.len(),
        augments.speed,
        augments.efficiency,
    )
}

/// The input tank as the declared fluid capability reports it, so the readout proves the tank
/// is a real handler holding this mod's fluid.
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
        FluidIdentity::Resolved(fluid) => fluid.id(host),
        FluidIdentity::Unresolved(fluid) => fluid.id(host),
        _ => "unknown".to_owned(),
    };
    let amount = i32::try_from(tanks.amount(host, 0)).unwrap_or(i32::MAX);
    (name, amount)
}

fn fluid_name(tank: &Tank) -> &str {
    if tank.is_empty() {
        "empty"
    } else {
        tank.fluid.as_str()
    }
}

/// `/industrialforegoing fluid <pos> <fluid> <amount>`: set the input tank. A mod fluid cannot
/// be inserted through the tank capability (`native_fluid_insert` takes water and lava only),
/// so this merges the entity's own `fluids` compound with the same `merge_data` path as
/// `/data merge block`.
pub(crate) fn set_fluid(host: &mut dyn ServerHost, pos: [i32; 3], fluid: &str, amount: i32) {
    let Some(entity) = crate::our_entity(host, pos, BLOCK) else {
        crate::say(host, &format!("{pos:?} is not a dissolution chamber."));
        return;
    };
    let Some(saved) = entity.data(&*host) else {
        crate::say(host, &format!("{pos:?} is not a mod block entity."));
        return;
    };
    let mut tanks = read_tanks(&saved);
    tanks[0] = Tank::new(fluid, amount.clamp(0, INPUT_TANK_MB));
    let mut patch = Compound::new();
    write_tanks(&mut patch, &tanks);
    let changed = entity.merge_data(host, patch).unwrap_or(false);
    let [input, _] = read_tanks(&entity.data(&*host).unwrap_or_default());
    crate::say(
        host,
        &if changed {
            format!(
                "Dissolution Chamber at {pos:?}: input {} {}/{} mB.",
                fluid_name(&input),
                input.mb,
                INPUT_TANK_MB
            )
        } else {
            format!("{pos:?} is not a dissolution chamber.")
        },
    );
}

/// `/industrialforegoing energy`/`progress` on the chamber: one int of its saved compound.
pub(crate) fn set_int(host: &mut dyn ServerHost, pos: [i32; 3], key: &str, value: i32) -> bool {
    let Some(entity) = crate::our_entity(host, pos, BLOCK) else {
        return false;
    };
    let mut patch = Compound::new();
    write_int(&mut patch, key, value);
    entity.merge_data(host, patch).unwrap_or(false)
}

/// The block entity type the declaration made, for the augment seat and the command paths.
pub(crate) fn kind() -> Option<mod_api::BlockEntityTypeKey> {
    KIND.get().copied()
}

/// The chamber's item, the only one `block_drops` may write to.
pub(crate) fn item() -> Option<ItemKey> {
    ITEM.get().copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chamber::OUTPUT_SLOTS;

    #[test]
    fn the_container_declares_eleven_slots_with_eight_inputs() {
        let container = container();
        assert_eq!(container.slots, 11);
        assert_eq!(container.inputs, (0..8).collect::<Vec<usize>>());
        assert_eq!(container.outputs, (8..11).collect::<Vec<usize>>());
        assert!(container.faces.iter().all(|face| face.len() == 11));
    }

    #[test]
    fn the_tanks_are_the_config_s_two() {
        let tanks = tanks();
        assert_eq!(tanks.tanks, vec![8_000, 16_000]);
        assert_eq!(tanks.inputs, vec![0], "only the input tank may be filled");
        assert_eq!(
            tanks.outputs,
            vec![0, 1],
            "both are drainable, as upstream's are"
        );
        assert!(tanks.faces.iter().all(|face| face == &vec![0, 1]));
    }

    #[test]
    fn the_tanks_round_trip_through_the_saved_compound() {
        let mut nbt = Compound::new();
        assert_eq!(read_tanks(&nbt), [Tank::EMPTY, Tank::EMPTY]);
        write_tanks(
            &mut nbt,
            &[
                Tank::new(DECLARED_FLUIDS[0], 250),
                Tank::new(DECLARED_FLUIDS[1], 300),
            ],
        );
        let [input, output] = read_tanks(&nbt);
        assert_eq!(input, Tank::new("industrialforegoing:latex", 250));
        assert_eq!(output, Tank::new("industrialforegoing:pink_slime", 300));
        write_tanks(&mut nbt, &[Tank::EMPTY, Tank::EMPTY]);
        assert_eq!(read_tanks(&nbt), [Tank::EMPTY, Tank::EMPTY]);
    }

    #[test]
    fn a_placed_chamber_clamps_what_its_item_carried() {
        let mut nbt = Compound::new();
        write_int(&mut nbt, KEY_ENERGY, 50_000);
        write_int(&mut nbt, KEY_PROGRESS, -4);
        let mut entry = Compound::new();
        entry.insert(
            "Fluid".into(),
            Tag::String("industrialforegoing:latex".into()),
        );
        entry.insert("Amount".into(), Tag::Int(999_999));
        nbt.insert("fluids".into(), Tag::List(vec![Tag::Compound(entry)]));
        let patch = clamp_patch(&nbt).expect("out-of-range values are corrected");
        assert_eq!(read_int(&patch, KEY_ENERGY), CAPACITY_FE);
        assert_eq!(read_int(&patch, KEY_PROGRESS), 0);
        assert_eq!(read_tanks(&patch)[0].mb, INPUT_TANK_MB);

        let mut fine = Compound::new();
        write_int(&mut fine, KEY_ENERGY, 4_000);
        write_int(&mut fine, KEY_PROGRESS, 25);
        write_tanks(
            &mut fine,
            &[Tank::new(DECLARED_FLUIDS[0], 250), Tank::EMPTY],
        );
        assert!(
            clamp_patch(&fine).is_none(),
            "a machine in range is left alone"
        );
    }

    #[test]
    fn a_chamber_with_fluid_or_a_bar_has_something_to_carry() {
        let mut nbt = Compound::new();
        assert!(!has_contents(&nbt));
        write_tanks(&mut nbt, &[Tank::new(DECLARED_FLUIDS[0], 250), Tank::EMPTY]);
        assert!(has_contents(&nbt));
        let mut nbt = Compound::new();
        write_int(&mut nbt, KEY_PROGRESS, 3);
        assert!(has_contents(&nbt));
        let mut nbt = Compound::new();
        write_int(&mut nbt, KEY_ENERGY, 90);
        assert!(has_contents(&nbt));
    }

    #[test]
    fn the_carried_patch_names_the_entity_type() {
        let mut nbt = Compound::new();
        write_int(&mut nbt, KEY_ENERGY, 90);
        let patch = carried_patch(&nbt);
        let Some(Tag::Compound(payload)) = patch.get(&"minecraft:block_entity_data".into()) else {
            panic!("no block entity data in the patch");
        };
        assert_eq!(
            payload.get(&"id".into()),
            Some(&Tag::String(ENTITY_ID.into()))
        );
        assert_eq!(read_int(payload, KEY_ENERGY), 90);
    }

    #[test]
    fn the_declared_fluids_are_the_recipes_own() {
        assert_eq!(
            DECLARED_FLUIDS,
            [
                "industrialforegoing:latex",
                "industrialforegoing:pink_slime",
                "industrialforegoing:ether_gas",
                "industrialforegoing:meat",
                "industrialforegoing:essence",
                "industrialforegoing:sewage",
                "industrialforegoing:milk"
            ]
        );
    }

    #[test]
    fn the_power_a_run_takes_is_the_bar_times_ninety() {
        assert_eq!(crate::chamber::POWER_PER_TICK, 90);
        assert_eq!(OUTPUT_SLOTS, 3);
        assert_eq!(INPUT_SLOTS + OUTPUT_SLOTS, SLOTS);
        assert_eq!(crate::chamber::Completion::default().consumed_fluid.mb, 0);
    }

    /// A component that survives only if the merge compares whole stacks.
    struct Marker;

    impl mod_api::Component for Marker {
        type Value = i32;

        fn id(&self) -> &str {
            "industrialforegoing:test_marker"
        }

        fn decode(&self, value: &Tag) -> Option<i32> {
            match value {
                Tag::Int(value) => Some(*value),
                _ => None,
            }
        }

        fn encode(&self, value: i32) -> Tag {
            Tag::Int(value)
        }
    }

    /// Two of this package's own item keys: `ItemKey` has no public constructor, so the test
    /// binary's one registration call mints them.
    fn two_items() -> (mod_api::ItemKey, mod_api::ItemKey) {
        crate::tests::two_item_keys()
    }

    fn marked(count: u32) -> ItemStack {
        let (beef, _) = two_items();
        let mut stack = ItemStack::new(beef, count).expect("a nonzero stack");
        stack.set(&Marker, 7).expect("the marker encodes");
        stack
    }

    #[test]
    fn a_product_with_another_component_does_not_merge_into_the_same_item() {
        let (beef, _) = two_items();
        let mut outputs = vec![None; 3];
        outputs[0] = Some(marked(1));
        let leftover = insert(&mut outputs, ItemStack::new(beef, 1).unwrap(), 64);
        assert!(leftover.is_none(), "the empty slot took it");
        assert_eq!(
            outputs[0].as_ref().map(ItemStack::count),
            Some(1),
            "the marked stack was not grown: a different patch is a different stack"
        );
        assert_eq!(
            outputs[0].as_ref().and_then(|stack| stack.get(&Marker)),
            Some(7),
            "the component the slot carried is still there"
        );
        assert_eq!(
            outputs[1].as_ref().map(ItemStack::count),
            Some(1),
            "the product landed in the next slot instead"
        );
    }

    #[test]
    fn a_product_with_the_same_components_and_damage_merges_and_keeps_them() {
        let (beef, _) = two_items();
        let mut plain = ItemStack::new(beef, 1).expect("a nonzero stack");
        plain.set_damage(4);
        let mut outputs = vec![None; 3];
        outputs[0] = Some(plain);
        let mut product = ItemStack::new(beef, 2).expect("a nonzero stack");
        product.set_damage(4);
        assert!(insert(&mut outputs, product, 64).is_none());
        let merged = outputs[0].as_ref().expect("the slot still holds the stack");
        assert_eq!(merged.count(), 3);
        assert_eq!(merged.damage(), 4, "the damage survived the merge");
    }

    #[test]
    fn a_product_that_fits_nowhere_comes_back() {
        let (_, leather) = two_items();
        let mut outputs = vec![None; 2];
        outputs[0] = Some(ItemStack::new(leather, 64).unwrap());
        outputs[1] = Some(ItemStack::new(leather, 64).unwrap());
        let leftover = insert(&mut outputs, ItemStack::new(leather, 1).unwrap(), 64);
        assert_eq!(
            leftover.map(|stack| stack.count()),
            Some(1),
            "a full container gives the reward back instead of dropping it on the floor"
        );
    }

    #[test]
    fn a_carried_chamber_payload_is_an_allowlist_without_its_container() {
        let mut nbt = Compound::new();
        write_int(&mut nbt, KEY_ENERGY, 90);
        write_int(&mut nbt, KEY_PROGRESS, 25);
        write_tanks(&mut nbt, &[Tank::new(DECLARED_FLUIDS[0], 250), Tank::EMPTY]);
        nbt.insert("Items".into(), Tag::List(Vec::new()));
        let patch = carried_patch(&nbt);
        let Some(Tag::Compound(payload)) = patch.get(&"minecraft:block_entity_data".into()) else {
            panic!("no block entity data in the patch");
        };
        assert!(
            payload.get(&"Items".into()).is_none(),
            "a container's contents ride on the engine's own spill, never twice"
        );
    }
}
