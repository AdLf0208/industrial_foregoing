//! Industrial Foregoing for Horizon — slice 1: the Water Condensator.
//!
//! Ported from Industrial Foregoing 1.21-3.6.39 (`Buuz135/Industrial-Foregoing`, MIT). Upstream the
//! machine is `WaterCondensatorTile` over Titanium's `IndustrialWorkingTile`: a block entity with a
//! 10,000 FE buffer, a 16,000 mB drain-only water tank and a bar that counts down; at zero it counts
//! the water source blocks on its six sides and runs one attempt. [`runtime`] is that arithmetic —
//! byte-identical to the retired `.hnmod` port's `runtime.rs`, tests included — and this file is the
//! glue: every number comes from `runtime`, and nothing but the shape of the game call is decided here.
//!
//! # Where the retired container's hooks went
//!
//! The 2026-11-17 mod-API merge (#16941, launcher ≥ 0.2.70) retired `.hnmod`: the plan, the injection
//! points and the per-hook helper files are gone, and a hook is a typed method or, for a moment that
//! belongs to one object, a Part. The mapping is:
//!
//! | `.hnmod` hook (API) | here |
//! |---|---|
//! | `block_entity` (15) | the block entity **type** declared in [`Hooks::register`] — a declared type starts with an empty compound |
//! | `block_entity_ticks` (15) | [`Machine::block_entity_ticks`] |
//! | `block_entity_energy` (142) | [`Registrar::block_energy`] in [`Hooks::register`] |
//! | `block_entity_fluids` (54) | [`Registrar::block_fluids`] in [`Hooks::register`] |
//! | `block_entity_tick` (17) | [`Machine::block_entity_tick`] |
//! | `state_for_placement` (76) | [`Machine::state_for_placement`] |
//! | `register_commands` (8) | [`Hooks::register_commands`] |
//! | `perform_command` (8) | [`Hooks::run_mod_command`] |
//! | the `content_root` injection (AE2 pattern) | gone: one flat `.hmod` carries `assets/` and `data/` itself |
//!
//! The machine has no item slots and its GUI is Titanium's bars, so the numbers are exposed through
//! `/industrialforegoing` commands instead of a screen — upstream registers no commands of its own.
//!
//! # Slice 2: the machine keeps its contents when it is broken
//!
//! Upstream's block item carries the block entity's saved compound, so a machine broken with FE and
//! water in it is placed back full. The pin has the same route vanilla's chests use: [`Machine`]'s
//! [`Hooks::block_drops`] writes the block entity's saved compound onto the dropped stack as the
//! vanilla `minecraft:block_entity_data` component (tagged with the type id), placement merges that
//! component into the new block entity, and [`MachineItem::on_block_placed_by`] clamps the numbers
//! back to the three capacities. An empty machine's item stays plain.
pub mod runtime;

use std::array;
use std::sync::{Mutex, OnceLock};

use mod_api::{
    self, BlockDropsEvent, BlockEntityPart, BlockEntityRef, BlockEntityTickEvent,
    BlockEntityTicksEvent, BlockEntityTypeKey, BlockPart, BlockState, Command,
    CommandArgumentValue, CommandInvocation, ComponentTypesEvent, Compound, Direction, FLUIDS,
    Flow, FluidIdentity, Hand, Hooks, Host, HostSide, HostTransaction, InteractionResult, ItemKey,
    ItemPart, ItemStack, LevelHost, LevelTickPostEvent, ModEnergyBuffer, ModFluidTanks,
    OnBlockPlacedByEvent, RegisterCommandsEvent, Registrar, RegistrationContext,
    RunModCommandEvent, ServerHost, StateForPlacementEvent, Tag, Text, UseItemOnEvent,
};

use crate::runtime::{
    AUGMENT_COMPONENT, AUGMENT_SLOTS, Augment, AugmentType, Augments, CAPACITY_FE, Condensator,
    MAX_PROGRESS, TANK_CAPACITY_MB,
};

/// The machine (`ModuleResourceProduction.WATER_CONDENSATOR`).
pub const BLOCK: &str = "industrialforegoing:water_condensator";
/// The block state property Titanium's `RotationType.FOUR_WAY` sets.
pub const SUBFACING: &str = "subfacing";
/// The saved keys: the buffer's amount (the energy declaration's own), the bar and the tank.
const KEY_ENERGY: &str = "energy";
const KEY_PROGRESS: &str = "progress";
/// The two augment slots, each an item's `titanium:augments` component.
const KEY_AUGMENTS: &str = "augments";
/// `minecraft:water`, the only fluid the tank takes.
const WATER: &str = "minecraft:water";
/// A side the tank is read and drained through (all six reach it).
const READ_FACE: Direction = Direction::Up;
/// The mod command's root word.
const COMMAND: &str = "industrialforegoing";

/// The machine's item, resolved once in `register`: `block_drops` runs with a read-only level host,
/// which cannot name an item, so the drop is only given the machine when it is ours.
static MACHINE_ITEM: OnceLock<ItemKey> = OnceLock::new();

/// The block entity type `register` declares, kept so a queued augment seating can prove the block
/// entity it writes to is still this machine — wherever in the world it is, without swapping a level.
static MACHINE_KIND: OnceLock<BlockEntityTypeKey> = OnceLock::new();

/// An augment seating a right-click asked for, applied after the level ticks: a block-entity write
/// made during a tick can be overwritten by that entity's own write-back (HANDOVER §9d.10), so the
/// seat and the held stack's shrink both land in `level_tick_post`.
struct Seating {
    /// The clicking player's UUID (`BlockClick.player`).
    player: u128,
    /// The level the machine stands in, so only that level's tick applies it.
    dimension: &'static str,
    /// The clicked machine.
    pos: [i32; 3],
    /// The hand the augment came from, to shrink the right slot.
    hand: Hand,
    /// The stack as it was clicked, to give back one fewer of it.
    stack: ItemStack,
    /// Which yield the augment moves, for the one-of-each-type rule.
    kind: AugmentType,
    /// The component's value, saved into the slot unchanged.
    component: Compound,
}

/// The seatings queued by `use_item_on`, in click order.
static SEATING: Mutex<Vec<Seating>> = Mutex::new(Vec::new());

/// The six sides, in `Direction`'s order (down, up, north, south, west, east).
const SIDES: [[i32; 3]; 6] = [
    [0, -1, 0],
    [0, 1, 0],
    [0, 0, -1],
    [0, 0, 1],
    [-1, 0, 0],
    [1, 0, 0],
];

/// The mod has no settings: everything it keeps is in the block entity's saved compound.
#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    /// Declare what the game answers for the machine (`EntityBlock` plus NeoForge's
    /// `RegisterCapabilitiesEvent`): the block entity type, the 10,000 FE buffer, the 16,000 mB
    /// drain-only tank, and the Part that ticks and rotates it.
    fn register(&self, r: &mut dyn Registrar, _cx: &RegistrationContext) {
        let block = r.block_key(BLOCK);
        let item = r.item_key(BLOCK);
        // `block_drops` has no game host, so the one item it may write to is resolved here.
        let _ = MACHINE_ITEM.set(item);
        // The item half of slice 2: a placed machine clamps what its item carried.
        r.item_part(item, MachineItem);
        // The declaration is what the retired `block_entity` hook returned: the game makes one block
        // entity for each of the type's blocks, starting from an empty compound.
        let kind = r
            .block_entity_type("water_condensator")
            .blocks(&["water_condensator"])
            .build();
        let _ = MACHINE_KIND.set(kind);
        // One Part, attached once, to the block entity type: `block_entity_part` also puts its
        // block half on every block of the type (`expand_block_entity_parts`), which is what makes
        // the block's own moments — `block_entity_ticks` and the placement rotation — reach it.
        // A `block_part` call for the same type as well attaches it twice, and the engine refuses
        // the whole package ("… is attached to … twice").
        r.block_entity_part(&kind, Machine)
            .expect("the ticker attaches to the block entity type this mod just declared");
        // Titanium's `EnergyStorageComponent(10000, 10, 20)` takes and gives up to its capacity on
        // any side; the amount lives in the compound's `energy` int.
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
        // `SidedFluidTankComponent` with `Action.DRAIN`: every face reaches the tank for reading and
        // draining, no face may fill it — the machine fills it itself.
        let all = vec![0usize];
        let mut tanks = ModFluidTanks::default();
        tanks.tanks = vec![TANK_CAPACITY_MB];
        tanks.faces = array::from_fn(|_| all.clone());
        tanks.outputs = vec![0];
        r.block_fluids(&block, tanks);
    }

    /// `component_types` (`RegisterDataComponentsEvent`): the one component an augment item carries,
    /// frozen before any content loads so this package's `horizon/item/` files may set it.
    /// `titanium:augments` is Titanium's `AugmentWrapper.ATTACHMENT`, the key every IF add-on
    /// (and `mifa`) puts on its addon items.
    fn component_types(&self, _context: &RegistrationContext, event: &mut ComponentTypesEvent<'_>) {
        event.types.push(AUGMENT_COMPONENT.to_owned());
    }

    /// After a level's tick pass: apply the augment seatings its right-clicks asked for. A block
    /// entity written during a tick can be overwritten by that entity's own write-back (HANDOVER
    /// §9d.10), so both the slot and the shrunk held stack are written here, never inside the tick.
    fn level_tick_post(&self, host: HostSide, event: &mut LevelTickPostEvent) {
        let HostSide::Server(host) = host else {
            return;
        };
        let dimension = event.level.dimension_id();
        let queued = take_seatings();
        let mut later = Vec::new();
        for seat in queued {
            if seat.dimension != dimension {
                later.push(seat);
                continue;
            }
            seat_augment(host, event.level, &seat);
        }
        if !later.is_empty() {
            queue_seatings(later);
        }
    }

    /// `BlockDropsEvent`: a broken machine's own item carries the machine's FE, water and bar, so
    /// placing it puts them back. The pin applies `minecraft:block_entity_data` to the new block
    /// entity before the block is placed, which is how a vanilla chest keeps its contents; upstream
    /// copies the block entity onto the drop the same way. An empty machine's item stays plain.
    fn block_drops(&self, level: &dyn LevelHost, event: &mut BlockDropsEvent<'_>) -> Flow {
        if level.state_block(event.state) != Some(BLOCK) {
            return Flow::Continue;
        }
        // Explosions have already removed the block entity, so only a break that still holds it
        // (a player's, a mob's) can carry the contents.
        let Some(entity) = event.block_entity else {
            return Flow::Continue;
        };
        let Some(saved) = level.block_entity_data(entity) else {
            return Flow::Continue;
        };
        if !has_contents(&saved) {
            return Flow::Continue;
        }
        let Some(item) = MACHINE_ITEM.get() else {
            return Flow::Continue;
        };
        let patch = carried_patch(&saved);
        for drop in event.drops.iter_mut() {
            if drop.stack.item() == *item {
                let _ = drop.stack.apply_components(&patch);
            }
        }
        Flow::Continue
    }

    /// The harness commands (`RegisterCommandsEvent`). Upstream has none: they exist so a machine can
    /// be read and set without its GUI (precedent: `/powah info|set`, `/charginggadgets energy`).
    fn register_commands(&self, _host: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        event
            .commands
            .push(Command::new("industrialforegoing info <pos:block_pos>"));
        for usage in [
            "industrialforegoing energy <pos:block_pos> <amount:integer>",
            "industrialforegoing water <pos:block_pos> <amount:integer>",
            "industrialforegoing progress <pos:block_pos> <ticks:integer>",
            "industrialforegoing drain <pos:block_pos> <amount:integer>",
        ] {
            event.commands.push(Command::op(usage, 2));
        }
    }

    /// The bodies of those commands (`run_mod_command`), on the typed invocation: the dispatcher has
    /// already parsed and resolved every argument, so nothing here splits a line.
    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        let invocation = event.command;
        if invocation.literals.first().map(String::as_str) != Some(COMMAND) {
            return Flow::Continue;
        }
        let Some(sub) = invocation.literals.get(1).map(String::as_str) else {
            say(
                host,
                "usage: /industrialforegoing info|energy|water|progress|drain ...",
            );
            return Flow::Cancel;
        };
        let Some(pos) = block_pos(invocation) else {
            say(host, &usage(sub));
            return Flow::Cancel;
        };
        match sub {
            "info" => {
                let line = readout(host, pos);
                say(host, &line);
            }
            "energy" | "water" | "progress" => {
                let (argument, full, unit, field) = match sub {
                    "energy" => ("amount", CAPACITY_FE, "FE", Field::Int(KEY_ENERGY)),
                    "water" => ("amount", TANK_CAPACITY_MB, "mB of water", Field::Tank),
                    _ => (
                        "ticks",
                        MAX_PROGRESS,
                        "ticks of bar",
                        Field::Int(KEY_PROGRESS),
                    ),
                };
                let Some(value) = integer(invocation, argument) else {
                    say(host, &usage(sub));
                    return Flow::Cancel;
                };
                let value = value.clamp(0, full);
                let changed = set(host, pos, field, value);
                say(
                    host,
                    &if changed {
                        format!("Water Condensator at {pos:?}: {value}/{full} {unit}.")
                    } else {
                        format!("{pos:?} is not a water condensator.")
                    },
                );
            }
            "drain" => drain(host, invocation, pos),
            _ => say(host, &usage(sub)),
        }
        Flow::Cancel
    }
}

/// The machine's Part, attached to the block (whether it ticks, and the state a placement takes) and
/// to the block entity type (the server tick). The game asks a Part only for the objects it is
/// attached to, so none of these methods has to check that the object is this machine.
struct Machine;

#[mod_api::horizon_part]
impl BlockPart for Machine {
    /// A placed machine faces its placer (`RotationType.FOUR_WAY` with the four `subfacing` values).
    fn state_for_placement(
        &self,
        level: &dyn LevelHost,
        event: &mut StateForPlacementEvent<'_>,
    ) -> Option<Option<BlockState>> {
        let faced =
            level.state_with_property(event.state, SUBFACING, facing_for_yaw(event.context.yaw))?;
        Some(Some(faced))
    }

    /// `AddonItem.useOn`: right-clicking the machine with an addon seats one in the first empty
    /// augment slot and takes one from the hand. The game asks a block's Part only for that block,
    /// so this never fires for another machine; a stack without `titanium:augments` passes, as
    /// `canAcceptAugment` refuses it and upstream falls through to the item.
    ///
    /// The seating is queued, not written here: the write lands in [`Mod::level_tick_post`]. The
    /// click is consumed either way, as upstream answers `CONSUME_PARTIAL` once it seats and the
    /// machine has nothing else to use; a refused second augment simply changes nothing.
    fn use_item_on(
        &self,
        host: &mut dyn ServerHost,
        event: &mut UseItemOnEvent<'_>,
    ) -> Option<InteractionResult> {
        let click = event.click;
        let stack = click.stack?;
        let component = augment_component(&*host, stack)?;
        let augment = decode_augment(&component)?;
        queue_seatings(vec![Seating {
            player: click.player,
            dimension: click.dimension.key().id(),
            pos: click.pos,
            hand: click.hand,
            stack: stack.clone(),
            kind: augment.kind,
            component,
        }]);
        Some(InteractionResult::Consume)
    }
}

#[mod_api::horizon_part]
impl BlockEntityPart for Machine {
    /// The machine ticks on the server (`EntityBlock.getTicker`).
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    /// One server tick: count the bar down, and at zero pass the water sources beside it to the pure
    /// attempt, then write back what changed. The view's write-back is checked: the default reply
    /// (`Changed`) keeps the edits and marks the chunk, so nothing here calls `setChanged`.
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let mut machine = Condensator::from_saved(
            read_int(&event.entity.nbt, KEY_ENERGY),
            read_tank(&event.entity.nbt),
            read_progress(&event.entity.nbt),
        );
        let before = (machine.energy, machine.water_mb, machine.progress);
        machine.tick_with_augments(water_sources(host, pos), read_augments(&event.entity.nbt));
        if machine.energy != before.0 {
            write_int(&mut event.entity.nbt, KEY_ENERGY, machine.energy);
        }
        if machine.water_mb != before.1 {
            write_tank(&mut event.entity.nbt, machine.water_mb);
        }
        if machine.progress != before.2 {
            write_int(&mut event.entity.nbt, KEY_PROGRESS, machine.progress);
        }
    }
}

/// The machine's item (slice 2): the drop carries the contents, and placing it clamps them back to
/// the capacities. The game asks a Part only for the item it is attached to.
struct MachineItem;

#[mod_api::horizon_part]
impl ItemPart for MachineItem {
    /// A placed machine takes the numbers its item carried — vanilla merged the item's
    /// `minecraft:block_entity_data` into the new block entity before this runs — and out-of-range
    /// values are clamped to the three capacities, as upstream's components clamp on load.
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

/// The water source blocks on the six sides of `pos` (`getWaterSources`: a `FluidState.isSource`,
/// which for water is the still, non-falling state — the block registry's default `minecraft:water`
/// state; flowing water and a waterlogged block's water are not).
fn water_sources(host: &mut dyn ServerHost, pos: [i32; 3]) -> u32 {
    let Some(level) = host.level(host.dimension()) else {
        return 0;
    };
    let Some(water) = host.block_state_id(WATER) else {
        return 0;
    };
    SIDES
        .iter()
        .filter(|[dx, dy, dz]| {
            level.block_state(host, [pos[0] + dx, pos[1] + dy, pos[2] + dz]) == Some(water)
        })
        .count() as u32
}

/// The horizontal `subfacing` a machine takes when a placer with `yaw` places it: Titanium's
/// `FOUR_WAY` turns the block toward the placer.
fn facing_for_yaw(yaw: f32) -> &'static str {
    let quarter = ((f64::from(yaw) / 90.0 + 0.5).floor() as i64 & 3) as u8;
    match quarter {
        0 => "north",
        1 => "east",
        2 => "south",
        _ => "west",
    }
}

// ---------------------------------------------------------------------------
// The `/industrialforegoing` commands
// ---------------------------------------------------------------------------

/// The usage line for `sub`, as a player who mistyped it is told.
fn usage(sub: &str) -> String {
    match sub {
        "info" => "usage: /industrialforegoing info <x> <y> <z>".to_owned(),
        "progress" => "usage: /industrialforegoing progress <x> <y> <z> <ticks>".to_owned(),
        other => format!("usage: /industrialforegoing {other} <x> <y> <z> <amount>"),
    }
}

/// One feedback line to the command's source, vanilla's silence and track-output rules included.
fn say(host: &mut dyn ServerHost, line: &str) {
    let _ = host.commands().feedback(Text::literal(line));
}

/// The `pos` argument every usage starts with.
fn block_pos(invocation: &CommandInvocation) -> Option<[i32; 3]> {
    match invocation.argument("pos")? {
        CommandArgumentValue::BlockPos(pos) => Some(*pos),
        _ => None,
    }
}

/// The integer argument `name`.
fn integer(invocation: &CommandInvocation, name: &str) -> Option<i32> {
    match invocation.argument(name)? {
        CommandArgumentValue::Integer(value) => Some(*value),
        _ => None,
    }
}

/// Which saved value `/industrialforegoing` writes.
#[derive(Clone, Copy)]
enum Field {
    /// One int of the compound (`energy`, `progress`).
    Int(&'static str),
    /// The single tank under `fluids`.
    Tank,
}

/// The machine's block entity at `pos`, when the block there is this mod's.
fn our_entity(host: &mut dyn ServerHost, pos: [i32; 3]) -> Option<BlockEntityRef> {
    let level = host.level(host.dimension())?;
    let block = host.block_key(BLOCK)?;
    if level.block(host, pos) != Some(block) {
        return None;
    }
    host.block_entity(level, pos)
}

/// Write `value` into the machine at `pos` by merging it into the block entity's saved compound, as
/// `/data merge block` does; whether it landed.
fn set(host: &mut dyn ServerHost, pos: [i32; 3], field: Field, value: i32) -> bool {
    let Some(entity) = our_entity(host, pos) else {
        return false;
    };
    let mut patch = Compound::new();
    match field {
        Field::Int(key) => write_int(&mut patch, key, value),
        Field::Tank => write_tank(&mut patch, value),
    }
    entity.merge_data(host, patch).unwrap_or(false)
}

/// `/industrialforegoing drain <pos> <amount>`: take water out through the declared tank, which is
/// what makes it a capability rather than a private counter.
fn drain(host: &mut dyn ServerHost, invocation: &CommandInvocation, pos: [i32; 3]) {
    let amount = integer(invocation, "amount").unwrap_or(0).max(0);
    let want = u32::try_from(amount).unwrap_or(u32::MAX);
    let Some(level) = host.level(host.dimension()) else {
        say(host, &format!("{pos:?} is not a water condensator."));
        return;
    };
    let answer = host
        .transaction::<String, ()>(|host, tx| {
            let Some(tanks) = level.capability(host, &FLUIDS, pos, Some(READ_FACE)) else {
                return Ok(format!("Water Condensator at {pos:?}: no drainable fluid."));
            };
            let Some(resource) = tanks.resource(host, 0) else {
                return Ok(format!("Water Condensator at {pos:?}: no drainable fluid."));
            };
            let taken = tanks.extract(host, tx, 0, &resource, want);
            if taken == 0 {
                return Ok(format!("Water Condensator at {pos:?}: no drainable fluid."));
            }
            let name = match resource.fluid() {
                FluidIdentity::Resolved(fluid) => fluid.id(host),
                FluidIdentity::Unresolved(fluid) => fluid.id(host),
                _ => "an unknown fluid".to_owned(),
            };
            Ok(format!(
                "Water Condensator at {pos:?}: drained {taken} mB of {name}."
            ))
        })
        .unwrap_or_else(|()| format!("{pos:?} is not a water condensator."));
    say(host, &answer);
}

/// The machine's state, as `/industrialforegoing info` prints it.
fn readout(host: &mut dyn ServerHost, pos: [i32; 3]) -> String {
    let Some(level) = host.level(host.dimension()) else {
        return format!("{pos:?} is not a water condensator.");
    };
    let Some(entity) = our_entity(host, pos) else {
        return format!("{pos:?} is not a water condensator.");
    };
    let Some(data) = entity.data(host) else {
        return format!("{pos:?} is not a mod block entity.");
    };
    let energy = read_int(&data, KEY_ENERGY).clamp(0, CAPACITY_FE);
    let water = read_tank(&data).clamp(0, TANK_CAPACITY_MB);
    let progress = read_progress(&data).unwrap_or(MAX_PROGRESS);
    let augments = read_augments(&data);
    let sources = water_sources(host, pos);
    let facing = level
        .block_property(host, pos, SUBFACING)
        .unwrap_or_else(|| "unknown".to_owned());
    let time = level.game_time(host).unwrap_or(0);
    format!(
        "Water Condensator at {pos:?}: {energy}/{CAPACITY_FE} FE, water {water}/{TANK_CAPACITY_MB} mB, sources {sources}, progress {progress}/{}, augments speed {} efficiency {:.2}, facing {facing}, t={time}",
        augments.bar_max(),
        augments.speed,
        augments.efficiency,
    )
}

// ---------------------------------------------------------------------------
// Saved-compound reads and writes
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

/// A machine saved before this slice (or just placed) has no bar yet: full, as
/// `ProgressBarComponent` starts it.
fn read_progress(nbt: &Compound) -> Option<i32> {
    match nbt.get(&KEY_PROGRESS.into()) {
        Some(Tag::Int(value)) => Some(*value),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// The augment slots (slice 3)
// ---------------------------------------------------------------------------

/// The machine's two augment slots, the `titanium:augments` component each
/// item carried; an empty compound (or a missing entry) is an empty slot.
fn augment_slots(nbt: &Compound) -> [Option<Compound>; AUGMENT_SLOTS] {
    let mut slots = [const { None }; AUGMENT_SLOTS];
    let Some(Tag::List(entries)) = nbt.get(&KEY_AUGMENTS.into()) else {
        return slots;
    };
    for (slot, entry) in slots.iter_mut().zip(entries) {
        if let Tag::Compound(component) = entry
            && !component.is_empty()
        {
            *slot = Some(component.clone());
        }
    }
    slots
}

/// Write both slots back, keeping the positions: an empty slot is an empty
/// compound, so slot 1 does not move to slot 0.
fn write_augment_slots(nbt: &mut Compound, slots: &[Option<Compound>; AUGMENT_SLOTS]) {
    let entries = slots
        .iter()
        .map(|slot| Tag::Compound(slot.clone().unwrap_or_default()))
        .collect();
    nbt.insert(KEY_AUGMENTS.into(), Tag::List(entries));
}

/// The augments in force, decoded from the saved slots.
fn read_augments(nbt: &Compound) -> Augments {
    Augments::from_slots(augment_slots(nbt).map(|slot| slot.as_ref().and_then(decode_augment)))
}

/// The augment an item's `titanium:augments` component names, speed first as
/// `canAcceptAugment` asks; `None` for a component this machine does not read.
fn decode_augment(component: &Compound) -> Option<Augment> {
    AugmentType::ALL.into_iter().find_map(|kind| {
        let value = number(component, kind.name())?;
        Some(Augment { kind, value })
    })
}

/// A component value as the float Titanium stores: a data pack's JSON number
/// reaches it as a `Double`, a `/give`'s SNBT as a `Float`, a hand-written one
/// as an `Int`.
fn number(nbt: &Compound, key: &str) -> Option<f32> {
    match nbt.get(&key.into())? {
        Tag::Float(bits) => Some(f32::from_bits(*bits)),
        Tag::Double(bits) => Some(f64::from_bits(*bits) as f32),
        Tag::Int(value) => Some(*value as f32),
        _ => None,
    }
}

/// The `titanium:augments` component a clicked stack carries: its own override
/// when it has one, else the item's declared default (`AugmentWrapper`'s
/// `stack.getOrDefault(ATTACHMENT, …)` reads the effective component).
fn augment_component(host: &dyn Host, stack: &ItemStack) -> Option<Compound> {
    let patch = stack.components().map(|components| components.as_nbt());
    if let Some(Tag::Compound(component)) =
        patch.and_then(|patch| patch.get(&AUGMENT_COMPONENT.into()))
    {
        return Some(component.clone());
    }
    match host.item_default_component(stack.item(), AUGMENT_COMPONENT) {
        Some(Tag::Compound(component)) => Some(component),
        _ => None,
    }
}

/// Queue augment seatings for the level's next `level_tick_post`.
fn queue_seatings(seatings: Vec<Seating>) {
    if let Ok(mut queued) = SEATING.lock() {
        queued.extend(seatings);
    }
}

/// Take every queued seating.
fn take_seatings() -> Vec<Seating> {
    SEATING
        .lock()
        .map(|mut queued| std::mem::take(&mut *queued))
        .unwrap_or_default()
}

/// Seat one queued augment: the first empty slot of the machine at `pos` takes
/// the component, then one comes out of the hand that clicked. A second augment
/// of a type the machine already has is refused (`canAcceptAugment`), and
/// nothing is taken from the hand.
fn seat_augment(host: &mut dyn ServerHost, level: mod_api::LevelRef, seat: &Seating) {
    let Some(entity) = host.block_entity(level, seat.pos) else {
        return;
    };
    let Some(kind) = MACHINE_KIND.get() else {
        return;
    };
    if entity.kind(&*host) != Some(*kind) {
        return;
    }
    let Some(saved) = entity.data(&*host) else {
        return;
    };
    let mut slots = augment_slots(&saved);
    if slots
        .iter()
        .flatten()
        .filter_map(decode_augment)
        .any(|seated| seated.kind == seat.kind)
    {
        return;
    }
    let Some(index) = slots.iter().position(Option::is_none) else {
        return;
    };
    slots[index] = Some(seat.component.clone());
    let mut patch = Compound::new();
    write_augment_slots(&mut patch, &slots);
    if entity.merge_data(host, patch).unwrap_or(false) {
        shrink_held(host, seat);
    }
}

/// Take one augment out of the hand that clicked: the selected hotbar slot for
/// the main hand, the off hand's own slot for the other.
fn shrink_held(host: &mut dyn ServerHost, seat: &Seating) {
    let Some(player) = host.player(seat.player) else {
        return;
    };
    let slot = match seat.hand {
        Hand::Main => player.selected_slot(&*host).map(|slot| slot as u32),
        Hand::Off => Some(40),
    };
    let Some(slot) = slot else {
        return;
    };
    let rest = seat.stack.clone().shrink(1);
    let _ = player.set_inventory_slot(host, slot, rest);
}

/// The tank's amount, in the declaration's shape: one entry per tank under `fluids`.
fn read_tank(nbt: &Compound) -> i32 {
    let Some(Tag::List(entries)) = nbt.get(&"fluids".into()) else {
        return 0;
    };
    let Some(Tag::Compound(entry)) = entries.first() else {
        return 0;
    };
    match entry.get(&"Fluid".into()) {
        Some(Tag::String(fluid)) if fluid.to_string_lossy() == WATER => {}
        _ => return 0,
    }
    match entry.get(&"Amount".into()) {
        Some(Tag::Int(amount)) => *amount,
        _ => 0,
    }
}

// ---------------------------------------------------------------------------
// The machine on its own item (slice 2)
// ---------------------------------------------------------------------------

/// The block entity type's registry id, which `minecraft:block_entity_data` tags its payload with
/// (vanilla's `TypedEntityData` only loads data whose `id` names the block entity it is placed as).
const BLOCK_ENTITY_TYPE_ID: &str = "industrialforegoing:water_condensator";
/// The vanilla component that moves a block entity's saved data on its item
/// (`BlockEntity.applyComponentsFromItemStack`).
const BLOCK_ENTITY_DATA: &str = "minecraft:block_entity_data";

/// Whether the machine holds anything worth carrying on its item: FE, water or a bar part way
/// through. An empty machine's item stays plain, so nothing has to be clamped back on placing it.
fn has_contents(saved: &Compound) -> bool {
    let machine = Condensator::from_saved(
        read_int(saved, KEY_ENERGY),
        read_tank(saved),
        read_progress(saved),
    );
    machine.energy > 0
        || machine.water_mb > 0
        || machine.progress != MAX_PROGRESS
        || augment_slots(saved).iter().any(Option::is_some)
}

/// The `minecraft:block_entity_data` payload a broken machine's item carries: the block entity's
/// own saved compound, tagged with its type id.
fn carried_patch(saved: &Compound) -> Compound {
    let mut payload = saved.clone();
    payload.insert("id".into(), Tag::String(BLOCK_ENTITY_TYPE_ID.into()));
    let mut patch = Compound::new();
    patch.insert(BLOCK_ENTITY_DATA.into(), Tag::Compound(payload));
    patch
}

/// The corrections a placed machine needs when the item it came from carried values outside the
/// three capacities (`/give` with a hand-written component, or a save from another version).
/// `None` when the saved compound is already within range.
fn clamp_patch(saved: &Compound) -> Option<Compound> {
    let energy = read_int(saved, KEY_ENERGY);
    let water = read_tank(saved);
    let progress = read_progress(saved).unwrap_or(MAX_PROGRESS);
    let machine = Condensator::from_saved(energy, water, Some(progress));
    let mut patch = Compound::new();
    let mut changed = false;
    if machine.energy != energy {
        write_int(&mut patch, KEY_ENERGY, machine.energy);
        changed = true;
    }
    if machine.water_mb != water {
        write_tank(&mut patch, machine.water_mb);
        changed = true;
    }
    if machine.progress != progress {
        write_int(&mut patch, KEY_PROGRESS, machine.progress);
        changed = true;
    }
    changed.then_some(patch)
}

/// Write the tank back in the same shape (one tank).
fn write_tank(nbt: &mut Compound, mb: i32) {
    let mut entry = Compound::new();
    entry.insert(
        "Fluid".into(),
        Tag::String(if mb > 0 { WATER } else { "minecraft:empty" }.into()),
    );
    entry.insert("Amount".into(), Tag::Int(mb.clamp(0, TANK_CAPACITY_MB)));
    nbt.insert("fluids".into(), Tag::List(vec![Tag::Compound(entry)]));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The engine refuses a package that attaches one Part type to one block twice, and
    /// [`mod_api::Registrar::block_entity_part`] already puts the block half of a block-entity part
    /// on every block of its declared type (`expand_block_entity_parts`). This runs the port's own
    /// `register` through the SDK's registration path, so the assertion is the engine's own verdict:
    /// with the redundant `block_part` call it answered `Mod parts refused: … Machine is attached to
    /// industrialforegoing:water_condensator twice`.
    #[test]
    fn the_engine_accepts_every_part_of_this_package() {
        let category = |_: &str| false;
        let position = |_: &str| None;
        let schema = mod_api::__private::RuleSchema {
            category: &category,
            position: &position,
            rule_count: 0,
        };
        let mods = vec![mod_api::ModEntry::new("industrialforegoing", Box::new(Mod))];
        let refused =
            mod_api::__private::install_parts(&mods, &schema, |_: &mod_api::DimensionSettings| {
                false
            });
        assert!(
            refused.is_ok(),
            "the engine refuses this package: {refused:?}"
        );
    }

    #[test]
    fn a_placer_turns_the_machine_toward_itself() {
        assert_eq!(facing_for_yaw(0.0), "north");
        assert_eq!(facing_for_yaw(90.0), "east");
        assert_eq!(facing_for_yaw(180.0), "south");
        assert_eq!(facing_for_yaw(270.0), "west");
        // West of the block, looking east: a placer at 270° or -90° is the same quarter.
        assert_eq!(facing_for_yaw(-90.0), "west");
        assert_eq!(facing_for_yaw(360.0), "north");
        // Inside a quarter, the nearer of the two values wins.
        assert_eq!(facing_for_yaw(44.0), "north");
        assert_eq!(facing_for_yaw(46.0), "east");
    }

    #[test]
    fn the_tank_round_trips_through_the_saved_compound() {
        let mut nbt = Compound::new();
        assert_eq!(read_tank(&nbt), 0);
        write_tank(&mut nbt, 1234);
        assert_eq!(read_tank(&nbt), 1234);
        write_tank(&mut nbt, 0);
        assert_eq!(read_tank(&nbt), 0);
    }

    #[test]
    fn a_tank_holding_another_fluid_reads_empty() {
        let mut entry = Compound::new();
        entry.insert("Fluid".into(), Tag::String("minecraft:lava".into()));
        entry.insert("Amount".into(), Tag::Int(500));
        let mut nbt = Compound::new();
        nbt.insert("fluids".into(), Tag::List(vec![Tag::Compound(entry)]));
        assert_eq!(read_tank(&nbt), 0);
    }

    #[test]
    fn the_tank_never_saves_more_than_it_holds() {
        let mut nbt = Compound::new();
        write_tank(&mut nbt, TANK_CAPACITY_MB + 5);
        assert_eq!(read_tank(&nbt), TANK_CAPACITY_MB);
    }

    /// The family rule of slice 2: a broken machine's item carries the machine, and placing it
    /// puts those numbers back. The payload is vanilla's `minecraft:block_entity_data` because
    /// the pin loads that component into the new block entity before the block is placed.
    #[test]
    fn the_machine_s_item_carries_its_saved_state() {
        let mut nbt = Compound::new();
        write_int(&mut nbt, KEY_ENERGY, 7_000);
        write_tank(&mut nbt, 5_000);
        write_int(&mut nbt, KEY_PROGRESS, 40);
        assert!(
            has_contents(&nbt),
            "a machine holding FE, water and a part-done bar has something to carry"
        );
        let patch = carried_patch(&nbt);
        let Some(Tag::Compound(payload)) = patch.get(&BLOCK_ENTITY_DATA.into()) else {
            panic!("the patch has no {BLOCK_ENTITY_DATA}");
        };
        assert_eq!(
            payload.get(&"id".into()),
            Some(&Tag::String(BLOCK_ENTITY_TYPE_ID.into())),
            "vanilla's TypedEntityData needs the block entity type id"
        );
        let machine = Condensator::from_saved(
            read_int(payload, KEY_ENERGY),
            read_tank(payload),
            read_progress(payload),
        );
        assert_eq!(
            (machine.energy, machine.water_mb, machine.progress),
            (7_000, 5_000, 40),
            "the machine's own saved numbers travel unchanged"
        );
    }

    #[test]
    fn an_empty_machine_has_nothing_to_carry() {
        let mut nbt = Compound::new();
        write_int(&mut nbt, KEY_ENERGY, 0);
        write_tank(&mut nbt, 0);
        write_int(&mut nbt, KEY_PROGRESS, MAX_PROGRESS);
        assert!(!has_contents(&nbt), "an empty machine's item stays plain");
    }

    /// A `/give` (or an older save) can carry anything: placing it must clamp to the three
    /// capacities, as upstream's `EnergyStorageComponent`/`FluidTank.setFluid` do on load.
    #[test]
    fn a_placed_machine_clamps_what_its_item_carried() {
        let mut too_much = Compound::new();
        write_int(&mut too_much, KEY_ENERGY, 50_000);
        // A hand-written amount, not `write_tank`: that one clamps as it saves.
        let mut entry = Compound::new();
        entry.insert("Fluid".into(), Tag::String(WATER.into()));
        entry.insert("Amount".into(), Tag::Int(999_999));
        too_much.insert("fluids".into(), Tag::List(vec![Tag::Compound(entry)]));
        write_int(&mut too_much, KEY_PROGRESS, 500);
        let patch = clamp_patch(&too_much).expect("an out-of-range machine is corrected");
        assert_eq!(read_int(&patch, KEY_ENERGY), CAPACITY_FE);
        assert_eq!(read_tank(&patch), TANK_CAPACITY_MB);
        assert_eq!(read_progress(&patch), Some(MAX_PROGRESS));

        let mut fine = Compound::new();
        write_int(&mut fine, KEY_ENERGY, 4_000);
        write_tank(&mut fine, 1_000);
        write_int(&mut fine, KEY_PROGRESS, 25);
        assert!(
            clamp_patch(&fine).is_none(),
            "a machine within capacity is left exactly as it was"
        );
    }

    // -----------------------------------------------------------------------
    // The augment slots (slice 3)
    // -----------------------------------------------------------------------

    /// A `titanium:augments` component: Titanium's `Map<String, Float>`, which
    /// a data pack's JSON reaches as a double and a `/give`'s SNBT as a float.
    fn component(kind: &str, value: Tag) -> Compound {
        let mut map = Compound::new();
        map.insert(kind.into(), value);
        map
    }

    #[test]
    fn an_augment_component_decodes_speed_before_efficiency() {
        assert_eq!(
            decode_augment(&component("Speed", Tag::Float(4.0f32.to_bits()))),
            Some(Augment {
                kind: AugmentType::Speed,
                value: 4.0
            })
        );
        let both = {
            let mut map = component("Efficiency", Tag::Double(0.8f64.to_bits()));
            map.insert("Speed".into(), Tag::Float(3.0f32.to_bits()));
            map
        };
        assert_eq!(
            decode_augment(&both).map(|augment| augment.kind),
            Some(AugmentType::Speed),
            "canAcceptAugment asks Speed first"
        );
        assert_eq!(
            decode_augment(&component("Efficiency", Tag::Double(0.8f64.to_bits()))),
            Some(Augment {
                kind: AugmentType::Efficiency,
                value: 0.8
            }),
            "a data pack's JSON number is a Double"
        );
        assert_eq!(
            decode_augment(&component("Speed", Tag::Int(3))),
            Some(Augment {
                kind: AugmentType::Speed,
                value: 3.0
            })
        );
        assert_eq!(
            decode_augment(&component("Processing", Tag::Float(3.0f32.to_bits()))),
            None
        );
        assert_eq!(decode_augment(&Compound::new()), None);
    }

    #[test]
    fn a_seated_augment_round_trips_through_the_saved_compound() {
        let speed_four = component("Speed", Tag::Float(4.0f32.to_bits()));
        let mut nbt = Compound::new();
        assert_eq!(augment_slots(&nbt), [None, None]);
        write_augment_slots(&mut nbt, &[Some(speed_four.clone()), None]);
        assert_eq!(augment_slots(&nbt), [Some(speed_four), None]);
        assert_eq!(read_augments(&nbt).speed, 4);
    }

    #[test]
    fn the_two_slots_keep_their_positions() {
        let efficiency = component("Efficiency", Tag::Double(0.8f64.to_bits()));
        let mut nbt = Compound::new();
        write_augment_slots(&mut nbt, &[None, Some(efficiency.clone())]);
        let slots = augment_slots(&nbt);
        assert_eq!(slots[0], None, "the first slot stays empty");
        assert_eq!(slots[1], Some(efficiency));
        let augments = read_augments(&nbt);
        assert_eq!(augments.efficiency, 0.8);
        assert_eq!(augments.speed, 1, "no speed augment is the base rate");
    }

    #[test]
    fn an_empty_machine_with_only_an_augment_has_something_to_carry() {
        let mut nbt = Compound::new();
        assert!(!has_contents(&nbt), "a fresh machine carries nothing");
        write_augment_slots(
            &mut nbt,
            &[Some(component("Speed", Tag::Float(3.0f32.to_bits()))), None],
        );
        assert!(
            has_contents(&nbt),
            "a machine holding an augment has something to carry"
        );
    }
}
