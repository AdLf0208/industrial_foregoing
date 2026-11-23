# Industrial Foregoing — port notes (slice 2: the Water Condensator)

Ported from **Industrial Foregoing 1.21-3.6.39**
(`industrialforegoing-1.21-3.6.39.jar`, SHA-1
`a653f5c2b56f51c82d008b473f2b89aeff5747fd` = the pack record, SHA-256
`abeb644675398e7729eb5f3637e1dfbe080e0dbdfcced1b19bd4ab595ccdd4bb`) for
Minecraft Java 1.21.1 / NeoForge. **MIT → licence rank 0**; local-only until
publishing is approved (ARCHITECTURE.md §8).

**Format: the SDK package (2026-11-18).** This port was migrated from the retired
`.hnmod` container to the flat `.hmod` the mod-API merge (#16941) introduced. Since slice 2 it
builds at nightly **`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`** with mixins
**`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`** (0.2.0 and 0.2.1 built at `0b14f8a`; the repin is
additive for this port — no source change, the three breaking hooks are unused), and
`src/runtime.rs` is the retired `runtime.rs` byte for byte (`7b377432…41b4d`). See
[`## Migration to the SDK format`](#migration-to-the-sdk-format) for the recipe, the hook
mapping and the traps; everything below it that names `mixins/`, `horizon-mixin.toml`, an
injection or `support.rs` describes the **retired** port and is kept as history.

Status: **ported-partial** — slice 1 (the Water Condensator: its block, FE
buffer, water tank, the one condensation rule and its placement rotation) is verified in game,
slice 2 (0.2.2, 2026-11-21) adds the machine's **contents surviving a break** and makes its
**carried recipe actually craft**, slice 3 (0.2.3, 2026-11-21) adds the **`titanium:augments`
component and the speed/efficiency arithmetic** the parent's machines read, and slice 4 (0.2.4,
2026-11-21) ships the **`ModuleCore` item and block set** — the four machine frames, the three gears,
plastic and the 16 laser lenses, with the tags the pack's add-ons bind to and the condensator's recipe
naming them again. The current pin is nightly **`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`** + mixins
**`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`**. **0.2.0 — the version merged in the registry —
could not load at all**: it attached `Machine` as both the block's Part and the block entity
type's Part, which the engine refuses ("`industrialforegoing::Machine is attached to
industrialforegoing:water_condensator twice`"), and it had no in-game run to catch that. 0.2.1
removes the redundant `block_part` call (see [The doubled Part](#the-doubled-part-021-2026-11-20));
0.2.2 carries that repair **and never shipped before**; 0.2.3 is the same machine plus its
augment slots; 0.2.4 adds the Core set beside it. See [Slice 2](#slice-2--contents-survive-the-break--the-recipe-crafts-022-2026-11-21),
[Slice 3](#slice-3--the-augment-family-023-2026-11-21) and [Slice 4](#slice-4--the-modulecore-item-and-block-set-024-2026-11-21).
slice 7 (0.2.7, 2026-11-22) adds the **laser drill and its ore base** with their own
`industrialforegoing:laser_drill_ore` recipe type and ten of the jar's ore recipes — the parent
surface `industrialforegoingsouls` is blocked on — so the family list is the rest of the mod. slice 6
(0.2.6, 2026-11-22) adds the **latex chain** — the Fluid Extractor and the Latex
Processing Unit with the extractor's own recipe type and its nine log recipes, `dryrubber` and the
`smelting dryrubber -> plastic` recipe — so `plastic` finally has a source and the fluids slice 5
declared are produced by a machine. slice 8 (0.2.8, 2026-11-22) adds the **fluid laser base** and its
own `industrialforegoing:laser_drill_fluid` recipe type — the tank machine the same drill charges —
so `industrialforegoing:ether_gas` has a source and only `:pink_slime` remains sourceless. slice 9
(0.2.9, 2026-11-23) adds the **mob slaughter factory** and the **pink slime set**: `:pink_slime` now
has its source, the pink slime item/ingot/block ship with their recipes and tags, and the dissolution
chamber finally runs an `outputFluid` recipe in game (the jar's only one, `pink_slime_ball`). The gap
list at the end is the rest of the mod.

**Why this mod now:** it is the rank-0 `content`/`machines` core mod
`precheck-mod.py industrialforegoing` offers (`DISPATCHABLE`, jar licence MIT,
rank 0, no unported content dependency, no worldgen); its declared library
`titanium` is a `library` row mapped to Horizon APIs/gaps, which does not block
the port (ARCHITECTURE.md §8, "Libraries are mapped, not ported").
`ports/_shared/queue/brief-industrialforegoing.md` is the brief.

## Class + tier and the approach

- **Class:** `content`; **tier:** `machines` (`ports/ledger.csv`,
  `ports/_shared/source-map/atm10-sources.csv`).
- **Approach** (`horizon/docs/modpacks.md`): a **machines** shape — a block
  entity with a server tick, a declared FE capability, a declared fluid tank and
  a menu/screen where the machine has one — plus the carried block assets/lang/
  data. Here the machine has no item slots and its GUI is Titanium's bars, so
  the numbers are exposed through commands instead of a screen (gap 1).
- **Carry-over vs agent port:** the blockstate, both block models (the
  condensator's and its `base_block` parent), the item model, five textures, the
  lang strings, the loot table, the recipe and the advancement are carried from
  the jar (`tools/assemble_content.py`, decisions in [`CONTENT.md`](CONTENT.md));
  the block entity, the FE buffer, the tank, the rule, the tick, the placement
  rotation and the commands are the agent port.

## What the slice does

- **The block.** `industrialforegoing:water_condensator` on the
  **`minecraft:iron_block`** template (upstream's
  `Properties.ofFullCopy(Blocks.IRON_BLOCK)`: hardness 5.0, blast resistance
  6.0, correct tool for drops, iron sounds), with the **`subfacing`** state
  property Titanium's `RotationType.FOUR_WAY` uses — the jar's own four-variant
  blockstate selects on it unchanged.
- **Placement rotation.** `state_for_placement` (API 76) sets `subfacing` from
  the placer's yaw, the face turning toward the placer (the e2e asserts `west`
  for a placer standing west of the block).
- **The buffer.** 10,000 FE (`WaterCondensatorConfig.maxStoredPower`), declared
  through `block_entity_energy` (API 142, `Capabilities.Energy.BLOCK`): any side
  may fill or drain it, up to the capacity a call, as Titanium's
  `EnergyStorageComponent(10000, 10, 20)` (the `10`/`20` are GUI pixels; its
  maxIO is the capacity). The amount is the int `energy` in the block entity's
  compound.
- **The tank.** 16,000 mB (`maxWaterTankSize`), water only, **drain-only**
  (`Action.DRAIN`): every face reaches it, no face may fill it, the machine
  fills it itself. Declared with `block_entity_fluids` (API 54); the contents
  live in the block entity's `fluids` list in the pin's shape, so
  `native_fluid_extract` drains it like any other tank.
- **The rule** (pure, `runtime.rs`): the bar starts full at 100, counts down one
  a tick, and at zero one attempt runs:
  - fewer than **2** water source blocks on the six sides: nothing, cooldown
    100 ticks, no FE;
  - 2 or more and at least **1,000 FE**: `sources × 100` mB, 1,000 FE spent,
    cooldown 10 ticks;
  - 2 or more and under 1,000 FE: `sources × 50` mB, nothing spent, cooldown 50
    ticks.
  The cooldown is `floor(100 × workAmount)`, exactly what Titanium's
  `ProgressBarComponent` sets from `WorkAction(0.1 | 0.5 | 1, energy)`.
- **Tank capacity.** A full (or nearly full) tank clamps the water but the
  operation still spends its 1,000 FE, as upstream's `fillForced` clamps and
  the tick still extracts.
- **A water source** is the still, non-falling water block state (vanilla's
  `FluidState.isSource` for water): the block registry's default
  `minecraft:water` state. Flowing water and a waterlogged block's water do not
  count.
- **Harness controls.** `/industrialforegoing info <pos>`,
  `/industrialforegoing energy|water|progress <pos> <amount>`,
  `/industrialforegoing drain <pos> <amount>` — upstream has no commands of its
  own (its GUI is the only control); precedent `/powah info|set`,
  `/charginggadgets energy`.
- **The augment slots (slice 3).** Two slots in the block entity's compound (`augments`), each the
  `titanium:augments` component an item carried. Right-clicking the machine with an addon seats one
  (after the level ticks, never mid-tick) and takes it from the hand; a second of the same type is
  refused. `Speed` sets how far the bar falls a tick and `Efficiency` scales its length
  (`floor(100 * efficiency)`), which is what the next cooldown is taken from — Titanium's
  `IndustrialWorkingTile`, not a new mechanic. `/industrialforegoing info` prints both and the bar's
  current maximum.

## Hooks — the retired `.hnmod` plan (history; the mapping is in §Migration)

The eight stamped hook points and the raw `content_root` target below are what the `.hnmod`
port injected. None of them exists in a package: the current shape is one `impl Hooks` and one
Part, and the table in §Migration names where each one went.

| Hook (API) | Helper | What it did |
|---|---|---|
| `block_entity` (15) | `block_entity.rs` | the machine's block entity |
| `block_entity_ticks` (15) | `block_entity_ticks.rs` | ticks on the server |
| `block_entity_energy` (142) | `block_entity_energy.rs` | the 10,000 FE buffer |
| `block_entity_fluids` (54) | `block_entity_fluids.rs` | the 16,000 mB drain-only tank |
| `block_entity_tick` (17) | `block_entity_tick.rs` | the bar, the attempt, the sources |
| `state_for_placement` (76) | `state_for_placement.rs` | `subfacing` toward the placer |
| `register_commands` (8) | `register_commands.rs` | the five harness commands |
| `perform_command` (8) | `perform_command.rs` | their bodies |
| `content_root` (raw target) | `embedded_content.rs` | writes the bundle's own content out and loads it |

9 injections (8 named hook points + one raw `content_root` target), 2 modules,
**0 unstamped**.

## Pin facts that shaped this slice (the retired `1d63e0a5` pin; the SDK's are in §Migration)

1. **Block-to-block FE exists but is not needed**: the buffer is read and written
   as the BE's own `energy` int, and the API-142 handler moves FE in and out; no
   neighbour walk (`native_move_energy`) is involved, so this slice does not
   exercise ARCHITECTURE §3's network gap. In the SDK the same two facts hold: the
   amount is the compound's `energy` int, and `Registrar::block_energy` is the
   whole capability.
2. **Fluid tanks are real at this pin** (API 54): the `fluids` list in the BE
   compound is the tank, and `native_fluid_extract` drains it — the e2e proves
   it with a 400 mB drain, so the tank is a capability, not a private counter.
3. **A BE that writes its own NBT inside `block_entity_tick`** kept the write at
   the retired pin — the write was on the injected `&mut ModBlockEntity` and the
   chunk was marked modified, the same pattern `auroral` and `charginggadgets`
   verified. The SDK replaces the mechanism with a checked write-back (see
   §Migration, "The two risk areas"): `Changed`, the default, keeps the edits and
   dirties the chunk.
4. **`workAmount` is the cooldown, not the progress**: Titanium's decreasing bar
   sets `progress = floor(max × workAmount)` after the attempt, which is why a
   powered operation repeats after 10 ticks and an idle one after 100.

## Content — retired `content/` manifest, carried folder now at the root

`assets/` and `data/` are generated from the jar by
[`tools/assemble_content.py`](tools/assemble_content.py); [`CONTENT.md`](CONTENT.md)
lists every carried and every written file and why. 18 files: the blockstate
(4 `subfacing` variants), the condensator and `base_block` models, the item
model, 26.3's item definition, five textures, the three lang strings, the loot
table and recipe (upgraded to 26.3), the advancement, the block/tab
declarations and the pickaxe tag. Since the migration they are part of the one
flat `industrialforegoing.hmod` (no content-only manifest, no second archive).

## Embedded content (AE2 pattern) — retired with the container

The registry used to take **one file per project**, so the `.hnmod` carried its own `.hmod` and
wrote it out at start. A flat `.hmod` holds `src/`, `assets/` and `data/` at once, so this whole
mechanism is gone: no `data/industrialforegoing-content.hmod`, no `content_root` injection, no
`.horizon-cache` copy. The old bundle and its embedded archive are kept for the record in
`../_shared/evidence/industrialforegoing/retired-hnmod-0.1.0/`.

## Migration to the SDK format

*Written 2026-11-18. Horizon's mod-API merge (#16941, launcher ≥ 0.2.70, PR #17992) retired
`.hnmod`; the registry closed every `.hnmod` submission that day with one reason, and the
container no longer loads. The port is now the flat SDK package: nightly
**`0b14f8acfd97fa2c750e42795249dcc981178ca4`**, mixins
**`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`**, launcher 0.2.78, `cargo-horizon-mod` from
`horizon-rust/horizon@main`. Followed `ports/imfast/notes.md §Migration to the SDK format`,
the first migration; where this port differs (it has content, a Part, capabilities and
commands), the difference is in the traps.*

### Files created

| Path | What it is |
|---|---|
| `horizon-mod.toml` | the only manifest and the only identity; `[native]` carries the two 40-hex pins |
| `Cargo.toml` | one crate; its only dependency is `horizon-mod-api` from `.horizon/sdk/` |
| `src/lib.rs` | the one `#[mod_api::horizon_mod] impl Hooks`, the `Machine` Part, the block entity type, the energy/fluid declarations and the commands |
| `src/runtime.rs` | `cp` of the retired `mixins/industrialforegoing/runtime.rs`; `cmp` clean, `7b377432…41b4d` in the port **and** in the package. Its doc comments still name the retired `native_block_entity_energy` hook — byte-identity was the requirement, so they stay and the mapping below is the current truth |
| `.gitignore` | `/target`, `/.horizon`, `/Cargo.lock` (a packed lock changes the artifact — trap 8) |
| `LICENSE`, `LICENSE-NOTICE.md` | the licence files travel inside the package (they were the review repair on PR #600) |
| `assets/industrialforegoing/`, `data/industrialforegoing/`, `data/minecraft/tags/` | the carried content, moved from `content/` to the package root |

### Files deleted, and where the evidence went

All of it is in [`../_shared/evidence/industrialforegoing/retired-hnmod-0.1.0/`](../_shared/evidence/industrialforegoing/retired-hnmod-0.1.0/).

| Deleted | Why |
|---|---|
| `mixins/industrialforegoing/` (`horizon-mixin.toml`, `runtime.rs`, `support.rs`, the nine per-hook helpers) | the plan/injection schema is retired; `runtime.rs` moved to `src/`, `support.rs` became `src/lib.rs` |
| `industrialforegoing.hnmod` | unloadable on launcher ≥ 0.2.70 |
| `compatibility.json` | `[native]` in the manifest is the pin now |
| `rust-toolchain.toml` | the pinned Horizon commit decides the toolchain |
| `content/` (its `horizon-mod.toml`, the old `industrialforegoing.hmod`, `content/target/`) | `assets/`+`data/` moved to the root; one manifest, one archive |
| `tools/horizon.py`, `tools/package.py`, `tools/package.sh`, `tools/build_hmod.sh`, `tools/hmod-load.sh`, `tools/ladder-remote.sh`, `tools/test_stage.py`, `tools/test_template.py`, `tools/audit/` | staging, `.hnmod` packaging and the plan audits belong to the retired tool |
| `Cargo.lock` (untracked) | no registry dependency of the mod's own |

**Kept:** `tools/assemble_content.py` (the jar → `assets`/`data` carry), `tools/e2e.sh` (the
scenario; see *The e2e* below), `notes.md`, `FINDINGS.md`, `CONTENT.md`, `README.md`.

### Manifest mapping

| Old | New |
|---|---|
| `horizon-mixin.toml` `[mod] id` | `horizon-mod.toml` `id` (also the resource namespace) |
| `[mod] version = "0.1.0"` | `version = "0.2.0"` — a resubmission is always a new version |
| `[mod] priority = 100` | gone; a hook's order is `#[listener(priority = …)]`, and this port has no ordering need |
| `[compatibility] horizon_revision = 1d63e0a5…` | `[native] horizon = 0b14f8ac…` |
| mixins `5a64af93…` (schema 6) | `[native] mixins = e59da909…` (the mixins pin moved with the merge) |
| the content mod's own `horizon-mod.toml` (`api 0.2`, `side both`, `registries`) | merged into the one manifest, `[native]` added; `api = "0.2"`, `side = "both"` and `capabilities = ["registries"]` unchanged |
| `game.rs` SHA-256 in `compatibility.json` | no field for it; the pin is the commit, and `cargo horizon-mod check` verifies the package against that build's `surface.json` |
| `horizon-native.json` | gone (the manifest is the only one) |

### Hook → host/event, exactly

| Retired call | Replaced by | Where the number comes from |
|---|---|---|
| `block_entity` hook, returning `Some((BLOCK, Compound::new()))` | **no hook**: `Registrar::block_entity_type("water_condensator").blocks(&["water_condensator"]).build()` — a declared type starts with an empty compound | — |
| `block_entity_ticks` hook, `ticks = true` | `BlockEntityPart::block_entity_ticks(&self, host: &dyn Host, event: &mut BlockEntityTicksEvent)` → `event.ticks = true` | — |
| `block_entity_energy` hook, `Some(ModEnergyBuffer { … })` | `Registrar::block_energy(&BlockKey, ModEnergyBuffer)` at registration | `runtime::CAPACITY_FE` |
| `block_entity_fluids` hook, `Some(ModFluidTanks { … })` | `Registrar::block_fluids(&BlockKey, ModFluidTanks)` at registration | `runtime::TANK_CAPACITY_MB` |
| `block_entity_tick` hook: `game: &mut Game`, `pos`, `&mut ModBlockEntity` | `BlockEntityPart::block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent)`; `event.pos`, `event.entity` is a `ModBlockEntityView` whose `.nbt` reads and writes through `DerefMut` | `runtime::Condensator::tick` |
| `game.world.block(p)` / `game.pack.assets.registry.default_state("minecraft:water")` | `host.level(host.dimension()) -> LevelRef`; `host.block_state_id("minecraft:water")`; `level.block_state(host, p)` | `runtime` is untouched; this is the source count |
| `game.world.modified.insert((cx, cz))` (`setChanged`) | the view's checked write-back: the default `Changed` reply keeps the edits and dirties the chunk, so there is no `setChanged` call | — |
| `state_for_placement` hook, `answer: &mut Option<Option<BlockStateId>>` | `BlockPart::state_for_placement(&self, level: &dyn LevelHost, event: &mut StateForPlacementEvent) -> Option<Option<BlockState>>`, `level.state_with_property(event.state, "subfacing", value)` | `facing_for_yaw(event.context.yaw)` (moved from `support.rs`, now tested) |
| `register_commands` hook, `&mut Vec<NativeCommand>` | `Hooks::register_commands(&self, host, event: &mut RegisterCommandsEvent)`; `Command::new(usage)` / `Command::op(usage, 2)` | — |
| `perform_command` hook, `game`, `player`, `command: &str`, `ran: &mut bool` | `Hooks::run_mod_command(&self, host, event: &mut RunModCommandEvent) -> Flow`; `event.command` is a typed `CommandInvocation` (`literals`, `argument("pos") -> BlockPos`, `argument("amount") -> Integer`), so nothing splits a line | — |
| `game.native_feedback(Text::literal(…))` | `host.commands().feedback(Text::literal(…))` | — |
| `game.native_with_mod_block_entity(pos, f)` | `level.block_entity(host, pos) -> BlockEntityRef`, then `entity.data(host)` to read and `entity.merge_data(host, patch)` to write (the `/data merge block` path) | — |
| `game.native_fluid_extract(dim, pos, face, n)` | `host.transaction(..)` + `level.capability(host, &FLUIDS, pos, Some(Direction::Up))` + `ResourceHandler::extract(host, tx, 0, &resource, n)` | — |
| `game.native_block_property(pos, "subfacing")` | `level.block_property(host, pos, "subfacing")` | — |
| `game.world.game_time` | `level.game_time(host)` | — |
| the `content_root` injection (AE2 pattern) | **deleted**; the flat package loads its own `assets/`+`data/` | — |

**The two risk areas, and what happened to them.**

- **Machine ticks.** The tick body is the same `Condensator::from_saved → tick → write back`
  sequence; what changed is where it reads and writes. The campaign's API-gap table (§3,
  "Block-entity writes inside a tick are reverted") warned that a block entity writing its own NBT
  inside `block_entity_tick` loses the write, while this port's own 0.1.0 run recorded the bar
  moving every tick — the record disagreed with itself. The SDK settles it: the Part reply is a
  **checked write-back** — `Changed` (the default, set before every Part runs) keeps the payload and
  dirties the chunk, `Unchanged` discards that Part's own edits — so the port writes and does not
  compensate. The read of the six neighbours and the write to the compound are the same calls the
  0.1.0 e2e measured, in the same order.
- **Recipe matching.** The recipe is data, not code: `data/industrialforegoing/recipe/water_condensator.json`
  travelled unchanged. The migration did not touch `assets/` or `data/` at all — the packed
  folders are **byte-identical to the retired package's**
  (`diff -r` over both trees: no difference; `assets` tree hash `7f5b75a5…f9da`, `data` tree
  hash `24491602…2411`), and `cargo horizon-mod build` validates the content as it packs it
  (it refuses a file outside the mod's namespaces and vanilla/convention `c` tags, which is how
  the first scaffold attempt failed in `/tmp`). So the carried recipe, loot table and blockstate
  behave exactly as the in-game run recorded below measured them.

### Build and check (the ladder for an SDK package)

`build-slot.sh` still speaks the retired tool and does not cd into `--port`, so every step is a
lease around a shell that cds:

```sh
cd ports/industrialforegoing
CARGO_INCREMENTAL=0 cargo test                                  # 22 passed (18 runtime + 4 glue)
cargo fmt --check                                               # NOT --all: that checks the SDK (trap 11)
cargo clippy --all-targets -- -D warnings
rm -f Cargo.lock && cargo horizon-mod build                     # -> target/horizon-mod/industrialforegoing.hmod
cp target/horizon-mod/industrialforegoing.hmod ./industrialforegoing.hmod
cargo horizon-mod check industrialforegoing.hmod                # exit 0
cargo horizon-mod check industrialforegoing.hmod --uses
cargo horizon-mod fits industrialforegoing.hmod --build nightly # "0b14f8a made for", exit 0
sha256sum industrialforegoing.hmod
```

Measured (evidence in `ports/_shared/evidence/industrialforegoing/`): **22 passed**, 0 failed;
clippy 0 warnings; `fmt --check` clean; package **28,979 B**, sha256
`c10182f32584e6522b274855f6c8331793421237112d87acf804a4fb17752ed7` (a second build of the
same sources produced the same bytes), **25 entries** — the 0.2.0 figures; 0.2.1 is
**30,694 B**, sha256 `4cc91337a2d92c023aad9d2bcfd9a83ef1c37c284bf1b33b8bbaeafebeaa1b55`, same 25
entries (see [The doubled Part](#the-doubled-part-021-2026-11-20))
(`horizon-mod.toml`, `Cargo.toml`, `LICENSE`, `LICENSE-NOTICE.md`, `README.md`, `src/lib.rs`,
`src/runtime.rs`, `uses.json`, the 17 carried content files); `check`
`industrialforegoing 0.2.0 (native-mod, format native-1/0b14f8a/e59da90, for 26.3)`, exit 0;
`fits` exit 0. `uses.json` is 272 entries; the ones that matter are
`hook:block_entity_tick`/`.pos`/`.entity`, `hook:state_for_placement`/`.state`/`.context`,
`hook:register_commands`/`.commands`, `hook:run_mod_command`/`.command`,
`method:BlockEntityPart::block_entity_tick`, `method:BlockPart::state_for_placement`,
`method:DefaultCapabilitiesRegistrar::block_energy`/`block_fluids`,
`method:HostTransaction::transaction`, `method:ResourceHandler::extract`,
`method:prelude::ServicesServerHost::commands`, `capability:horizon:fluids`,
`capability:registries`, `macro:horizon_mod`, `macro:horizon_part`.

### Traps, all hit in this migration

1. **`build-slot.sh` does not cd into `--port`, and forwarded this port to a path that does not
   exist on the far box** (`cd: /home/deepseek/atm10/industrialforegoing: No such file or
   directory`). Pass `-- bash -c 'cd ports/industrialforegoing && …'` and set `BUILD_LOCAL=1`.
2. **`JavaString` is not exported at `mod_api`'s root**, and it implements `From<&str>` only
   (`From<String>` is not implemented): write `"energy".into()` and `Tag::String("minecraft:water".into())`,
   never `.to_owned().into()`.
3. **`ModFluidTanks` is `#[non_exhaustive]`**, so the retired struct literal does not compile:
   `ModFluidTanks::default()`, then assign `tanks`/`faces`/`outputs`. `ModEnergyBuffer` is not
   non-exhaustive and its literal still works.
4. **`block_entity_ticks` is a `BlockEntityPart` method, not a `BlockPart` one**, and a Part needs
   `#[mod_api::horizon_part]` on each impl (it generates the hidden `block_subscribed` /
   `block_entity_subscribed`). One struct may carry both impls, and it is attached **once**, to the
   block entity type: `r.block_entity_part(&kind, Machine)`. `expand_block_entity_parts` then puts
   its `BlockPart` half on every block of the type, which is what reaches `state_for_placement` and
   `block_entity_ticks`. Attaching it with `r.block_part(block, Machine)` **as well** — which this
   note originally recommended — attaches the type twice and the engine refuses the whole package
   at load. That is exactly what 0.2.0 shipped; see [The doubled Part](#the-doubled-part-021-2026-11-20).
5. **The host has no level/block-entity reads.** They are generated on the handles:
   `level.block_state(host, pos)`, `level.block`, `level.block_property`, `level.game_time`,
   `entity.data(host)`, `entity.merge_data(host, patch)`, `FluidKey::id(host)`. The `host.level_*`
   / `host.fluid_key_id` / `host.block_entity_merge_data` names in the backend traits do not exist
   on `dyn ServerHost`.
6. **The e2e could not be re-run at migration time** (no `horizon-launcher` here then). **Superseded
   2026-11-20:** the launcher pipeline exists, `ports/tools/launcher-game.sh` builds a game at
   `0b14f8a` with this package, and the leg is green — see
   [The doubled Part](#the-doubled-part-021-2026-11-20).
7. **`host.transaction` takes a `Result` closure** and needs `HostTransaction` in scope; a bare
   `String` return does not compile.
8. **`Cargo.lock` is packed and changes the hash.** Any `cargo test`/`clippy` writes one;
   `rm -f Cargo.lock` immediately before `cargo horizon-mod build`, and keep it gitignored.
9. **`FluidIdentity` is `#[non_exhaustive]`**: the `drain` readout needs a wildcard arm.
10. **Two mutable borrows of the host in one expression** (`say(host, &readout(host, pos))`)
    do not compile: bind the line first.
11. **`cargo fmt --all --check` reports the fetched SDK's files** (it is a path dependency); use
    `cargo fmt --check`.
12. **The packer validates content while it packs.** A scaffold with `assets/demo/` failed with
    `assets/demo/x.json: a mod adds files in its own namespaces …` — which is also the cheapest
    content check this campaign has, and it passed for this port's 17 files.

### The e2e, and what stands as evidence

**Re-run and green (2026-11-20).** `tools/e2e.sh` no longer carries the retired container's legs:
one flat `.hmod` has no `--dev-mod content` and no `.horizon-cache`, so the `EMBEDDED`/`CONTROL`
pair retired with `.hnmod`, and the script now launches the launcher-built game with no dev mod and
reads `CTL`/`GAME`/`RES`/`SOCK` from `launcher-game.sh env`. The control leg is the same script on
`GAME=$CONTROL_GAME` — the same Nightly, unmodified — and it fails at the first assertion (the
machine item does not register). The 0.1.0 transcript below stays as the record of the same rules
at the retired pin.

## The doubled Part (0.2.1) (2026-11-20)

**0.2.0 could not load, and it is the version the registry merged.** `Hooks::register` attached
`Machine` twice — `r.block_part(block, Machine)` and `r.block_entity_part(&kind, Machine)` — and
the engine refuses a Part type attached to one block twice:

```
Horizon: Mod parts refused:
industrialforegoing::Machine is attached to industrialforegoing:water_condensator twice, by industrialforegoing and industrialforegoing
```

`expand_block_entity_parts` (`horizon-mod-api/src/parts/block_entities.rs`) already inserts the
`BlockPart` half of every block-entity part on every block of its declared type, so the
`block_part` call was the second attachment. 0.2.0 had **no in-game leg at all**
(`ports/_shared/evidence/industrialforegoing-e2e/` was empty), which is why nothing caught it —
`check`, `fits`, clippy and the unit tests cannot see a refused part.

The fix deletes that one call; nothing else changed. `src/runtime.rs` and the content are
untouched, and the block half — `block_entity_ticks` and the placement rotation — still arrives
through `block_entity_part`, which the in-game run proves.

| step | command | result |
|---|---|---|
| failing test first | `cargo test --offline` | **22 passed, 1 failed** — `tests::the_engine_accepts_every_part_of_this_package` printed the engine's own verdict: `Err(["industrialforegoing::Machine is attached to industrialforegoing:water_condensator twice, by industrialforegoing and industrialforegoing"])` (`evidence/industrialforegoing/registration-red.log`) |
| tests | `cargo test --offline` | **23 passed**, 0 failed (18 runtime + 4 glue + the registration test) (`sdk-test.log`) |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | exit 0, **0 warnings** (`sdk-clippy.log`) |
| format | `cargo fmt --check` | exit 0 |
| build 1 + 2 | `rm -f Cargo.lock && cargo horizon-mod build`, two runs `cmp` | **byte-identical**, no `Cargo.lock` |
| package | `unzip -l industrialforegoing.hmod` | **30,694 B**, SHA-256 `4cc91337a2d92c023aad9d2bcfd9a83ef1c37c284bf1b33b8bbaeafebeaa1b55`, **25 entries** |
| package check | `cargo horizon-mod check industrialforegoing.hmod` | `industrialforegoing 0.2.1 (native-mod, format native-1/0b14f8a/e59da90, for 26.3)`, exit 0 (`sdk-check.log`) |
| fits | `cargo horizon-mod fits industrialforegoing.hmod --build 0b14f8a` | exit 0, *"0b14f8a made for"* (`sdk-fits.log`) |
| gate | `python3 ports/tools/verify-sdk-package.py industrialforegoing` | **1/1** |
| before (0.2.0) | the same sources with the one `block_part` call restored, launcher-built and launched | **game refuses the package**: `industrialforegoing::Machine is attached to industrialforegoing:water_condensator twice, by industrialforegoing and industrialforegoing`, `LAUNCH-EXIT:3` (`evidence/industrialforegoing-e2e/e2e-launcher-before.log`) |
| in-game (modded) | `launcher-game.sh build industrialforegoing`, then the leg in a slot | **`PASS` + `EXIT:0`** (`evidence/industrialforegoing-e2e/e2e-launcher.log`) |
| control | same script, `GAME=$CONTROL_GAME` (same Nightly, no package) | `Unknown item 'industrialforegoing:water_condensator'` → **`EXIT:1`** (`…-control.log`) |
| selftest | `SELFTEST=1` (flips B's 200 mB to 201) | `FAIL: the water is 200, expected 201` → **`EXIT:1`** (`…-selftest.log`) |
| log gate | `check-e2e-logs.py industrialforegoing-e2e --strict` | `logs=4 ok=4` |

The registration test is the SDK's own path, not a copy of its rule: it builds a `RuleSchema`, puts
this crate's `Mod` in a `ModEntry` and calls `mod_api::__private::install_parts`, which is what the
game calls at startup. It is the one test that installs parts (that state is process-wide).

The in-game run's phase A is what proves the behaviour the deleted call was believed to provide:
`Water Condensator at [202, 101, 0]: … facing west` — `BlockPart::state_for_placement`, dispatched
through `table.block_value`, reached through the block half `block_entity_part` installs. The rest
of the run re-proves the rule set (B–J) unchanged.

**Not published from here** (the brief owns no registry entry): the merged 0.2.0 is live-broken, so
the registry needs a 0.2.1 PR carrying this artifact, and the entry's `version`/`sha256` must move
with it.

### The changelog for the resubmission

> Supersedes #600, closed because Horizon retired `.hnmod` (#17992). Changes: flat SDK package
> with `api = "0.2"` and `[native]` pinned to `0b14f8a` / mixins `e59da90`; the eight injected
> hooks and the embedded-content `content_root` injection are now one typed
> `impl mod_api::Hooks` plus one `#[mod_api::horizon_part]` Part — the block entity type and the
> FE/fluid declarations are registration calls, the tick reads `BlockEntityTickEvent` and the
> `LevelRef` host reads, the placement rotation is `BlockPart::state_for_placement`, and the
> commands are `register_commands`/`run_mod_command` over typed arguments; the content
> (`assets/`, `data/`) travels in the same file, so the AE2 embedded-content trick is gone;
> `runtime.rs` and its 18 tests are unchanged, byte for byte, and the content is byte-identical
> to 0.1.0's.

## Slice 2 — contents survive the break + the recipe crafts (0.2.2, 2026-11-21)

**The load repair ships here.** 0.2.1 fixed the doubled Part but its PR was closed, so the
registry's live 0.2.0 still installs a mod the engine refuses. 0.2.2 carries the same one-line
repair (no `block_part` call; `block_entity_part` alone installs both halves) and was proven at
the campaign pin, not only at `0b14f8a`:

| step | command | result |
|---|---|---|
| before (0.2.0 registration, rebuilt at `0a875da`) | launcher `native-build` of a copy with `r.block_part(block, Machine);` restored, then `ctl launch` | **refused at load**: `Horizon: Mod parts refused:` / `industrialforegoing::Machine is attached to industrialforegoing:water_condensator twice, by industrialforegoing and industrialforegoing`, `LAUNCH-EXIT:3` (`evidence/industrialforegoing/load-before-0a875da.log`) |
| after (this package) | the same launch with 0.2.2 | `REFUSED-LINES:0`; `give … water_condensator` → `Gave 1 [Water Condensator]`; the block `setblock` reads back `industrialforegoing:water_condensator`; `Horizon: content from 1 mod(s)`; `EXIT:0` (`evidence/industrialforegoing/load-after-0a875da.log`) |
| control | the same Nightly with no package | `Unknown item 'industrialforegoing:water_condensator'` → `EXIT:1` (`e2e-launcher-control.log`) |

**The slice: gap 5 — "Breaking the machine loses its FE and water".** The gap was written when
the pin had no route for a mod block's drop; the 2026-11-21 ruling in `ARCHITECTURE.md` §3 closes
that (first proven by `rftoolsutility` 0.2.1): `Hooks::block_drops` writes the block entity's saved
compound onto the dropped stack as vanilla `minecraft:block_entity_data` tagged with the type id,
and placement merges it into the new block entity before the block is placed.

- `Hooks::block_drops` answers only for this block, reads the removal-held saved compound
  (`level.block_entity_data`), and leaves an **empty** machine's item plain (`has_contents`).
- The payload is the machine's own compound (`energy`, `fluids`, `progress`) plus
  `id = "industrialforegoing:water_condensator"`, which is what vanilla's `TypedEntityData`
  requires.
- `MachineItem` (`ItemPart::on_block_placed_by`) clamps what the item carried to the three
  capacities (`clamp_patch`), so a hand-written `/give … minecraft:block_entity_data` cannot put
  50,000 FE in a 10,000 FE buffer.
- The numbers stay in `runtime.rs`; the new helpers in `src/lib.rs` only build/read the compound.

**TDD, in order.** The three new glue tests were written first against `todo!()` bodies:
`the_machine_s_item_carries_its_saved_state`, `an_empty_machine_has_nothing_to_carry`,
`a_placed_machine_clamps_what_its_item_carried` — **23 passed, 3 failed**, each
`panicked at src/lib.rs: not yet implemented` (`evidence/industrialforegoing/sdk-test-red.log`).
Then the glue, then **26 passed, 0 failed** (`sdk-test.log`).

**The carried recipe was dead data, and the game said so.** The package's recipe used the
"one tag or a vanilla fallback" list shape for `#c:plastics`, `#c:gears/iron` and
`#industrialforegoing:machine_frame/pity`. The 26.3 loader rejects that shape, and the log named
the file and every offending ingredient:

```
[load-items/ERROR]: recipes: invalid ingredient (unresolved tag or malformed input): ["#c:plastics","minecraft:slime_ball"]
[load-items/ERROR]: recipes: invalid ingredient (unresolved tag or malformed input): ["#industrialforegoing:machine_frame/pity","minecraft:iron_block"]
[load-items/ERROR]: recipes: invalid ingredient (unresolved tag or malformed input): ["#c:gears/iron","minecraft:iron_block"]
[horizon-advancements/ERROR]: data/industrialforegoing/advancement/recipes/misc/water_condensator.json: unknown load conditions [neoforge:item_exists]; file left out
```

So the machine was **uncraftable** and the recipe's advancement was dropped — the previous
wave's "fallback list" repair did not survive `26.3`. Fixed under the upstream-defect rule:

- `data/industrialforegoing/recipe/water_condensator.json`: the three list ingredients are the
  vanilla fallback alone (`minecraft:slime_ball`, `minecraft:iron_block`, `minecraft:iron_block`).
  The tags have no items in a standalone package, and a list that names them is invalid, so the
  fallback is the only shape that both loads and crafts.
- `data/industrialforegoing/advancement/recipes/misc/water_condensator.json`: the unknown
  `neoforge:conditions` gate is dropped (the item exists whenever this package does) and the
  `has_item` criterion names `minecraft:slime_ball` instead of the absent `#c:plastics`.

A user notices: the Water Condensator is craftable from the recipe in a plain crafting table, and
the recipe's advancement unlocks.

**In game (the same `tools/e2e.sh`, phases K and L added).**

```
== K: the carried recipe crafts the machine from the tags' vanilla fallbacks ==
  crosshair -> {"block": {"block": "minecraft:crafting_table", ...}, "kind": "block"}
  interact -> {..., "ok": true, "screen": "inventory"}
  the declared grid -> {"count": 1, "item": "industrialforegoing:water_condensator", "slot": 200}
  glowstone instead of redstone -> null
recipe PASS: the declared grid crafts one industrialforegoing:water_condensator and consumes every ingredient
== L: a broken machine's item carries its FE and water, and placing it restores them ==
  Water Condensator at [202, 101, 0]: 7000/10000 FE, water 5000/16000 mB, sources 0, progress 96/100, facing west, t=643
  break: {"block":"minecraft:air","drops":{"industrialforegoing:water_condensator":1},"picked_up":{"industrialforegoing:water_condensator":1},"items_left_on_ground":0}
  the broken machine came back with 7000/10000 FE and 5000/16000 mB
PASS: … the carried recipe crafts one machine from its declared ingredients and consumes them, while a wrong grid stays empty; and a machine broken with 7,000 FE and 5,000 mB comes back with exactly those numbers on the item it dropped
EXIT:0
```

**Fail-capable, three ways.** (1) The same scenario against the **previous package** (0.2.1 at
this pin, identical content, no `block_drops`) fails exactly at the new assertion:
`FAIL: the buffer the item carried is 0, expected 7000` → `EXIT:1`
(`e2e-before-slice.log`). (2) `SELFTEST=1` flips phase B's 200 mB to 201 → `EXIT:1`
(`e2e-launcher-selftest.log`). (3) `SELFTEST=contents` flips phase L's 5,000 mB to 5,001 →
`FAIL: the restored tank is 5000, expected 5001` → `EXIT:1`
(`e2e-launcher-selftest-contents.log`). Phase K carries its own control: the same grid with
glowstone instead of redstone leaves the result empty.

**Harness facts learned here** (recorded so the next slice does not pay them again): a screen or
`click-slot` needs the **offscreen** renderer (the old headless leg left the yaw untouched, which
is why the facing assertion passed without the offscreen renderer; the scenario now also waits a
tick after `look-at-block`, because the rotation reaches the server a tick later); the drop has to
be broken **in survival with a pickaxe** (a creative break drops nothing, and the iron-block
template needs a tool); the item has to be stood on to be picked up; and water near the machine
must be cleared in a shell before each attempt, because flow from an earlier leg keeps reaching
the faces.

**The package.**

| | |
|---|---|
| version | `0.2.2` |
| pin | `native-mod`, format `native-1/0a875da…/e59da90…`, for 26.3 |
| size | **34,096 B**, 25 entries, no packed `Cargo.lock` |
| SHA-256 | `2672a80085cb0a258239408750b625fbee497f760693e473d823c0b947404826` (two clean-lock builds `cmp`-identical) |
| `check` | `industrialforegoing 0.2.2 (native-mod, …)`, exit 0 |
| `fits` | `0a875da  made for`, `0b14f8a  fits`, both exit 0 |
| runtime | `src/runtime.rs` packed = tree, `7b377432…41b4d` |

### Coverage (families this package ships)

| family | state |
|---|---|
| the block and its item | `tested` — phase A places it from the item and reads it back; the control game refuses the id |
| placement rotation (`subfacing`) | `tested` — phase A: `facing west` for a placer west of it |
| the condensation rule (sources, power, cooldowns) | `tested` — phases B–H (2/1/0/3/6 sources, powered/unpowered, tank clamp) |
| the FE buffer (capability) | `tested` — phases B/H/J read and spend it; `/industrialforegoing energy` writes it |
| the water tank (fluid capability) | `tested` — phase I drains 400 mB through `FLUIDS` |
| placement of the drop's contents (`block_drops` + `block_entity_data`) | `tested` — phase L (before/after differential) |
| the carried recipe | `tested` — phase K crafts it and consumes the ingredients; a wrong grid is empty |
| the carried advancement | `not ported` (data) — repaired and loaded, but no criterion is exercised in game |
| GUI screen / tooltip | `not ported` — gap 1 (no menu; the readout is the command) |
| redstone control modes | `not ported` — gap 2 |
| augments | `tested` — slice 3, phases M–Q below |
| comparator output, config file | `not ported` — gaps 7, 8 |
| the other ~71 blocks and their recipes | `not ported` — gap 9 (they are other slices) |

### The changelog for the 0.2.2 resubmission

> Supersedes the closed 0.2.1 PR (which superseded #600). **The load repair ships here:** 0.2.0,
> the version in the registry, attaches `industrialforegoing::Machine` to
> `industrialforegoing:water_condensator` twice and the engine refuses the whole package at load;
> this version attaches the Part once (`block_entity_part`), and the refusal and the working load
> are both reproduced at the campaign pin `0a875da`. New: a machine broken with FE and water in it
> drops an item carrying them as vanilla `minecraft:block_entity_data`, and placing it restores
> them (clamped to the buffer and tank capacities), proven in game with a before/after
> differential. Fixed: the carried recipe used a tag-or-item list shape the 26.3 loader rejects,
> so the machine could not be crafted; the three ingredients are their vanilla fallbacks and the
> recipe now crafts and consumes them in game, and the carried advancement's unknown
> `neoforge:item_exists` condition is dropped. `runtime.rs` is unchanged, byte for byte.

## Slice 3 — the augment family (0.2.3, 2026-11-21)

**The slice: gap 3 — "No augments. Speed, efficiency and processing augments are Titanium
components; none is ported, so every machine runs at the base rate."** It is the surface two
add-ons of this campaign are blocked on, measured rather than assumed: `mifa` resolved 29 of its
61 external symbols as missing and the blocking one is `titanium:augments` plus the machine that
reads it (`evidence/mifa/parent-references.log`), and `industrialforegoingsouls` references the
parent seven times with none resolved. The port now registers the component and reads it, so an
item that carries it is no longer inert.

Upstream, read at `Buuz135/Industrial-Foregoing` `ce2852db` and Titanium `e53e869`:

- `AugmentWrapper` stores a `Map<String, Float>` under **`titanium:augments`**; `AugmentTypes`
  names the keys `Speed` and `Efficiency`.
- `AddonItem.useOn`: right-click a `MachineTile` with an augment → the first empty slot of its
  two-slot augment inventory takes one and the hand gives one up (`CONSUME_PARTIAL`);
  `IndustrialMachineTile.canAcceptAugment` refuses a second augment of the same type.
- `IndustrialWorkingTile`: every tick sets the bar's `progressIncrease` from the speed value, and
  after a completed operation sets `maxProgress = floor(100 * efficiency)` and rests the bar at
  `maxProgress * workAmount`. So **speed divides the cooldown** (the bar falls faster) and
  **efficiency shortens the bar** at the same 1,000 FE a operation — the same energy per
  operation, a higher rate.
- The parent's own items are `speed_addon_tier_1/2` (`Speed` 2.0 / 3.0) and
  `efficiency_addon_tier_1/2` (`Efficiency` 0.9 / 0.8).

What this package does with them:

| piece | where | what |
|---|---|---|
| component registration | `Hooks::component_types` (`src/lib.rs`) | declares `titanium:augments` before content loads, so this package's `horizon/item/` files may set it |
| the two slots | `augments` list in the block entity's compound | two compounds, each an item's `titanium:augments` value; an empty compound is an empty slot, and the positions never move |
| seating | `BlockPart::use_item_on` → queued, applied in `Hooks::level_tick_post` | the click queues the seat; the tick's end merges the slot into the block entity and takes one from the hand (`Hand::Main` = the selected hotbar slot, `Hand::Off` = 40). A block-entity write made *during* a tick is overwritten by that entity's own write-back (HANDOVER §9d.10), which is why it lands here and not in the item hook |
| the arithmetic | `src/runtime.rs` | `AugmentType`/`Augment`/`Augments`, `Augments::bar_max()` (`floor(100 * efficiency)` in float, as Java's `(int) Math.floor`), `Attempt::cooldown_in(bar_max)`, `Condensator::tick_with_augments`; `tick` is the same call with `Augments::NONE` |
| the readout | `/industrialforegoing info` | `progress <p>/<bar max>` and `augments speed <n> efficiency <0.00>` |
| the items | `data/industrialforegoing/horizon/item/` | the parent's four addon items with their declared `titanium:augments` default and `max_stack_size` 16, and their own carried textures/models |

**TDD, in order.** The nine runtime tests and four glue tests were written first against `todo!()`
bodies: **16 passed, 23 failed**, every failure `not yet implemented` — the arithmetic
(`a_speed_augment_falls_four_a_tick`, `a_speed_augment_finishes_the_bar_in_twenty_five_ticks`,
`an_efficiency_augment_changes_the_bar_maximum_after_a_work`,
`the_efficiency_multiplier_floors_like_java_float_math`,
`the_speed_value_truncates_toward_zero_like_a_java_cast`,
`the_augment_path_with_no_augment_is_the_base_rate`) and the saved shape
(`an_augment_component_decodes_speed_before_efficiency`,
`a_seated_augment_round_trips_through_the_saved_compound`, `the_two_slots_keep_their_positions`,
`an_empty_machine_with_only_an_augment_has_something_to_carry`). Green: **39 passed, 0 failed**
(`evidence/industrialforegoing/sdk-test-red-slice3.log`, `sdk-test-slice3.log` = `sdk-test.log`).

**In game** (launcher build at the pin, `evidence/industrialforegoing-e2e/e2e-launcher-slice3.log`),
phases M–Q, quoted:

```
== M: with no augment the bar falls exactly one a tick ==
  Water Condensator at [202, 101, 0]: 0/10000 FE, water 5000/16000 mB, sources 0, progress 81/100, augments speed 1 efficiency 1.00, facing west, t=1129
  Water Condensator at [202, 101, 0]: 0/10000 FE, water 5000/16000 mB, sources 0, progress 98/100, augments speed 1 efficiency 1.00, facing west, t=1150
  Water Condensator at [202, 101, 0]: 0/10000 FE, water 5000/16000 mB, sources 0, progress 88/100, augments speed 1 efficiency 1.00, facing west, t=1160
  empty slot: progress 98 -> 88 over 10 ticks
== N: an efficiency addon shortens the bar to 80 and the cooldown to 8 ==
  Water Condensator at [202, 101, 0]: 0/10000 FE, water 5000/16000 mB, sources 0, progress 44/80, augments speed 1 efficiency 0.80, facing west, t=1204
  Water Condensator at [202, 101, 0]: 4000/10000 FE, water 400/16000 mB, sources 2, progress 7/80, augments speed 1 efficiency 0.80, facing west, t=1246
  efficiency 0.80: +400 mB for 1000 FE, bar 7/80
== O: a seated speed addon makes the bar fall four a tick (100 -> 60 in 10) ==
  Water Condensator at [202, 101, 0]: 0/10000 FE, water 800/16000 mB, sources 0, progress 4/80, augments speed 4 efficiency 0.80, facing west, t=1315
  Water Condensator at [202, 101, 0]: 0/10000 FE, water 800/16000 mB, sources 0, progress 84/80, augments speed 4 efficiency 0.80, facing west, t=1335
  Water Condensator at [202, 101, 0]: 0/10000 FE, water 800/16000 mB, sources 0, progress 52/80, augments speed 4 efficiency 0.80, facing west, t=1343
  speed 4: progress 84 -> 52 over 8 ticks
== P: a second speed addon is refused and stays in the hand ==
  Water Condensator at [202, 101, 0]: 0/10000 FE, water 800/16000 mB, sources 0, progress 44/80, augments speed 4 efficiency 0.80, facing west, t=1385
== Q: the augments travel on the machine's item ==
  Water Condensator at [202, 101, 0]: 0/10000 FE, water 800/16000 mB, sources 0, progress 72/80, augments speed 4 efficiency 0.80, facing west, t=1523
  the broken machine came back with speed 4 and efficiency 0.80
PASS: … a machine broken with 7,000 FE and 5,000 mB comes back with exactly those numbers on the item it dropped; a seated speed augment of 4 makes the bar fall four a tick where an empty slot falls one, a second speed augment is refused and stays in the hand, an efficiency augment of 0.8 shortens the bar to 80 with an 8-tick cooldown at the same 1,000 FE, and both augments travel on the machine's item
```

The rate is measured against the readout's own `t=` (`progress 84 -> 52 over 8 ticks` is exactly
`4 x 8`), so the chat round trip and the harness's waits cannot skew it, and the window is short
enough that a finished 80-tick bar cannot reset inside it (an earlier version used a 19–25 tick
window and read a reset as a negative rate). The run is fail-capable four ways: `SELFTEST=1` flips
phase B (`FAIL: the water is 200, expected 201`); `SELFTEST=augments` leaves phase B alone and
flips phase O's rate (`selftest: asserting a fall of 57` / `FAIL: speed 4 fell 56 over 14 ticks,
expected 57`), because a run that fails at B never reaches the augment legs; `SELFTEST=contents`
flips phase L; and the control leg (the same Nightly without the package) stops at the first
`give` (`FAIL: the machine item did not register`). All four are `EXIT:1`.

### Coverage (families this slice adds)

| family | coverage |
|---|---|
| the `titanium:augments` component registration | `tested` — the four items declare it and the e2e reads `speed 4` / `efficiency 0.80` off the machine after seating them |
| the two augment slots (saved shape, positions) | `tested` — `the_two_slots_keep_their_positions`, and phase Q breaks the machine and reads both back |
| seating an augment (`AddonItem.useOn`) | `tested` — phases N/O: the hand gives one up (`held … 0`), the slot takes it |
| one of each type (`canAcceptAugment`) | `tested` — phase P: a second speed addon is refused and stays held |
| the speed arithmetic | `tested` — phase O: `progress 84 -> 52 over 8 ticks`, four a tick, against phase M's one a tick |
| the efficiency arithmetic | `tested` — phase N: `progress …/80` and `+400 mB for 1000 FE`, cooldown 1..8 |
| the processing augment | `not ported` — a third type; its parent item is `processing_addon_*`, and this machine runs one operation a cycle, so it adds nothing here (gap 3b) |
| the addons' dissolution-chamber recipes | `blocked(industrialforegoing:dissolution_chamber)` — the machine is a later slice; the four items are `give`-only, and their recipes name that chamber |
| the GUI augment slots | `not ported` — gap 1 (no menu); the slots are seated by right-clicking the machine |

### Script regression found and fixed while doing this slice

`tools/assemble_content.py` regenerated `data/…/recipe/water_condensator.json` and the advancement
**from the jar**, which re-introduced the three absent tags slice 2 had replaced (`#c:plastics`,
`#c:gears/iron`, `#industrialforegoing:machine_frame/pity`) and the `neoforge:item_exists`
condition — the machine stopped crafting in game (`recipe PASS` → `the declared recipe did not
craft … None`). The repair now lives **in the script** (`upgrade_recipe`'s `fallbacks`,
`upgrade_advancement`), so a rerun reproduces slice 2's behaviour instead of undoing it; the
script also writes to the port root (`CONTENT = ROOT`) and no longer writes a content-only
manifest over the port's `horizon-mod.toml`.

### The changelog for the 0.2.3 update

> Supersedes the closed 0.2.2 PR of this entry (itself superseding 0.2.1/#600). **New: the
> augment family.** `titanium:augments` is registered through `component_types`, the Water
> Condensator has the parent's two augment slots in its saved compound, right-clicking it with an
> addon seats one (after the level ticks) and one of each type is allowed, and the speed and
> efficiency augments move the bar exactly as Titanium's `IndustrialWorkingTile` does — speed by
> its value a tick, efficiency by `floor(100 * efficiency)`. The parent's four addon items ship as
> `give`-only data with their component defaults and their own textures. Proven in game: an empty
> slot falls one a tick, a seated speed 4 falls four (84 → 52 over 8 ticks), an efficiency 0.8
> gives an 80-tick bar and an 8-tick cooldown at the same 1,000 FE, a second speed addon is refused
> and stays in the hand, and both augments travel on the broken machine's item. No `runtime.rs`
> rule of slices 1–2 changed: the base-rate path is the same call with no augment seated, and the
> 26 slice-1/2 tests pass unchanged.

## Slice 4 — the `ModuleCore` item and block set (0.2.4, 2026-11-21)

**The slice: gap 9** ("One machine only. The jar's other 71 blockstates … are outside the slice")
together with gap 6's own closing sentence ("When the frames/plastics are ported, the ingredients can
name the tags again — as **single** tags, not lists"). It is the surface **two stopped add-ons** name,
measured, not assumed (HANDOVER §9g): `mifa` is `blocked(industrialforegoing:dissolution_chamber)` and
resolved `c:gears/diamond`, `c:gears/gold` and `c:gears/iron` as **empty** tags plus
`industrialforegoing:{iron,gold,diamond}_gear` as **missing** items
(`evidence/mifa/parent-references.log`), and `industrialforegoingsouls` is
`blocked(missing-parent-item: industrialforegoing:blue_laser_lens)` and needs `#c:plastics`,
`#c:gears/diamond` and `industrialforegoing:machine_frame/advanced`
(`evidence/industrialforegoingsouls/BLOCKED-parent-surface.md`). This slice unblocks the **items**;
the chamber is the next one.

### What upstream registers (jar `industrialforegoing-1.21-3.6.39.jar`; source read at `ce2852db`)

| id | upstream | what the port ships |
|---|---|---|
| `machine_frame_pity/simple/advanced/supreme` | `ModuleCore.PITY`…`SUPREME`, `new MachineFrameBlock(Properties.ofFullCopy(IRON_BLOCK))` + a `MachineFrameItem` with a custom rarity; the blockstate's one variant is keyed `""` (no state property); loot `droppingSelf` | the block and its item, the iron-block template, the jar's blockstate/block model/item model and three textures each, the four `industrialforegoing:machine_frame/<tier>` **block and item** tags, one loot table each and the four entries in `minecraft:mineable/pickaxe` |
| `iron_gear` / `gold_gear` / `diamond_gear` | `ModuleCore.*_GEAR`, `RecipelessCustomItem("…", TAB_CORE)`; recipes are shaped 4-ingot rings | the items, the `c:gears` umbrella and `c:gears/{iron,gold,diamond}` tags with their items, the three recipes |
| `plastic` | `ModuleCore.PLASTIC`; its recipe is **`smelting dryrubber`** (the latex chain, out of this slice) | the item and the `c:plastics` tag. **No recipe**: its source is a machine this slice does not take, and a recipe naming an absent item is dropped whole |
| `<colour>_laser_lens` × 16 | `ModuleCore.LASER_LENS[DyeColor] = new LaserLensItem(colour)` — `Properties().stacksTo(1)`, name composed at runtime as "%s Laser Lens"; recipe `RecolorRecipe(any lens + that colour's dye)` | the 16 items (`max_stack_size` 1), their textures, models and resolved names, the port's `industrialforegoing:laser_lens` tag (the recipe's "any lens"), `c:dyes/<colour>` with the vanilla dye each, and all 16 recolour recipes |

**Artefact correction — there are 16 lenses, not 32.** The brief says "the 16 laser lenses plus their
16 inverted variants". `ModuleCore` registers **16** items, one per `DyeColor`, and the jar's
`models/item/laser_lens_inverted[0-15].json` + `textures/item/laser_lens[0-15]_inverted.png` are
**orphans**: `grep -rl laser_lens_inverted` over the jar's `assets/` and `data/` hits only the 18 lang
files — no item, blockstate, model or line of code names them (the lang key
`item.industrialforegoing.laser_lens_inverted` is unused). The port ships the 16 real items and does
**not** invent 16 ids for orphan art; the alternative would be 16 items upstream has no way to obtain
or use.

**Rarities.** Titanium's four frame rarities are **custom enum extensions** (`CustomRarity`: green,
aqua, light-purple, gold) with no 26.3 expression. The port maps the four tiers onto vanilla's four
rarity slots **in order** — pity `common`, simple `uncommon`, advanced `rare`, supreme `epic` — so the
tier order is visible and the colours differ (a user sees white/yellow/aqua/light-purple rather than
green/aqua/purple/gold). Recorded under "Deviations".

### Upstream defects fixed

1. **The recolour recipes name an ingredient as a list of 16 alternatives**
   (`[{"item":"…white_laser_lens"}, …]`, `laser_lens_<colour>_recolor.json`). 26.3 rejects that shape
   whole — the class measured on this port's own condensator recipe in slice 2
   (`invalid ingredient (unresolved tag or malformed input)`, HANDOVER §9d.11) — so all 16 recipes would
   have been dropped and no lens could ever change colour. The port declares
   `data/industrialforegoing/tags/item/laser_lens.json` with exactly those 16 items and the recipes name
   `#industrialforegoing:laser_lens`; a user notices that every recolour crafts, from any lens.
2. **`DyeColor.getTag()` is `c:dyes/<colour>`, and 26.3's base data ships no `c:` item tags at all**
   (`data/c/` is absent from the prepared resources; measured, HANDOVER §9g). The port declares the
   convention tags its carried recipes and this slice's frame recipe name — `c:dyes/<colour>` (one
   vanilla dye), `c:ingots/iron`, `c:storage_blocks/redstone`, `c:gears`, `c:gears/*`, `c:plastics` —
   with their vanilla or shipped values, so every recipe resolves standalone and a pack's own values
   merge in (tags merge across packs, `load_tag_lists`).
3. **The condensator's three stand-ins go back to the tags** (slice 2's repair, as gap 6 promised):
   `#c:plastics`, `#c:gears/iron`, `#industrialforegoing:machine_frame/pity` — single tags, never a
   list. A user notices the machine now costs an iron gear, two plastic and a pity frame instead of
   slime balls and iron blocks.

### TDD, in order

The slice's rules are the **shape of the package**, so the failing tests came first as
`#[cfg(test)] mod content` in `src/lib.rs` (seven tests: the 24 ids declared, each with a 26.3
definition and a model whose texture exists, each with an English name, the tags naming the items, the
condensator naming the three tags, every carried recipe's ingredients resolving, every frame's loot
table and pickaxe entry). They were written before `tools/assemble_content.py` could produce any of it:

```
---- content::the_condensator_recipe_names_the_three_tags_again stdout ----
the condensator recipe no longer names #c:plastics
---- content::every_module_core_id_of_this_slice_is_declared_once stdout ----
machine_frame_pity is a block upstream registers and this package does not declare
---- content::the_tags_name_the_shipped_items stdout ----
…/data/c/tags/item/gears/iron.json: No such file or directory (os error 2)
---- content::every_carried_recipe_names_an_ingredient_that_exists stdout ----
the slice carries 21 recipes (pity frame, 3 gears, the condensator, 16 recolours), found 1
test result: FAILED. 39 passed; 7 failed; 0 ignored; 0 measured; 0 filtered out
```

Then the generator, then **46 passed, 0 failed** (`sdk-test-red-slice4.log`, `sdk-test.log`).
`tools/verify_content.py` is the JSON-level pass over the same rules and reads the **delivered
`.hmod`** as well as the tree; its `--selftest` drops the `c:plastics` tag, drops a declaration and
restores slice 2's stand-ins, and all three are caught (`sdk-content-slice4.log`).

Two test iterations were honest corrections of my own assumptions, not of the artefacts: the jar's
`c:gears` lists the three **items** (not the three sub-tags), and `#minecraft:logs` is the base data's
tag, so the recipe check only demands a package tag outside `minecraft:`.

### In game (phases K, R, S, U; `evidence/industrialforegoing-e2e/e2e-launcher.log`)

```
== K: the carried recipes craft (the condensator from the tags, the frame, the gears) ==
the condensator from the tags (#c:plastics, #c:gears/iron, machine_frame/pity) -> {"count": 1, "item": "industrialforegoing:water_condensator", "slot": 200}
the control: slice 2's vanilla stand-ins leave the result empty -> null
the pity machine frame (WIW/IRI/WIW) -> {"count": 1, "item": "industrialforegoing:machine_frame_pity", "slot": 200}
the iron gear -> {"count": 1, "item": "industrialforegoing:iron_gear", "slot": 200}
the gold gear -> {"count": 1, "item": "industrialforegoing:gold_gear", "slot": 200}
the diamond gear -> {"count": 1, "item": "industrialforegoing:diamond_gear", "slot": 200}
crafted 5 shaped recipes, and the stand-in grid stayed empty
== R: every ModuleCore id of this slice registers and reads back ==
  the 24 Core ids read back: 24 items
  the four frames place and read back by id
  an unregistered item and block id are refused
== S: a machine frame breaks and drops itself ==
  break: {"block":"minecraft:air","drops":{"industrialforegoing:machine_frame_pity":1},"picked_up":{"industrialforegoing:machine_frame_pity":1},"items_left_on_ground":0}
== U: the 16 lens recolours craft from the lens tag and c:dyes, counts before and after ==
  white: industrialforegoing:black_laser_lens 1->0, minecraft:white_dye 1->0, industrialforegoing:white_laser_lens 0->1
  orange: industrialforegoing:white_laser_lens 1->0, minecraft:orange_dye 1->0, industrialforegoing:orange_laser_lens 0->1
  … (all 16 colours, each: the lens consumed, the dye consumed, the colour's lens gained)
crafted all 16 lens recolours and read the player's counts before and after each
```

The condensator's grid is `PBP/LML/GRG` with a real iron gear, plastic and a pity frame; after taking
the result every ingredient is gone and the grid holds nothing but the **empty bucket the water bucket
leaves behind** (vanilla's `craftRemainder` — the honest exception to "all nine cells empty", asserted
as exactly that). The control puts slice 2's stand-ins (`slime_ball`, `iron_block` ×3) into the same
grid and the result slot stays empty, which is the counterfactual the old package would have failed.

**Fail-capable, four ways**: the control leg (same Nightly, no package) stops at the first `give`
(`FAIL: the machine item did not register`) → `EXIT:1`; `SELFTEST=1` flips phase B
(`FAIL: the water is 200, expected 201`) → `EXIT:1`; `SELFTEST=contents` flips phase L; and the new
**`SELFTEST=core`** flips phase U's red-lens count (`FAIL: the red lens count after the craft is 1,
expected 2`) → `EXIT:1`. All four logs are in `evidence/industrialforegoing-e2e/` and
`check-e2e-logs.py industrialforegoing-e2e --strict` is 4/4.

**Harness facts this slice added**: `ctl open-inventory` opens the **creative** screen in creative
mode, which has no 2×2 grid — the recolour leg switches to survival first (the 2×2 is
`open-inventory`'s grid 100-103, result 200, and works in survival); a water bucket's
`craftRemainder` leaves an empty bucket in the grid, so a "the grid is empty" assertion must expect
it; and two ingredients of the same item in different cells must be **one** `give` and a right-click
per cell, because a single-cell ingredient given as a stack is placed whole.

**A dialect flag, not a defect.** `ports/tools/check_261_dialect.py` reports the four frames' loot
tables (and the condensator's) for `"bonus_rolls": 0.0` and `"rolls": 1.0` — the two shapes HANDOVER
§9d.13 marks as **unverified** ("no game has shown them to change behaviour; a flag is not a
defect"). They are the jar's own pool fields, the condensator has shipped them since 0.2.2, and this
slice's phase S proves the drop: `drops{"industrialforegoing:machine_frame_pity": 1}`.

### Coverage (families this slice adds)

| family | coverage |
|---|---|
| the four machine frames (block + item) | `tested` — phase R places each and reads it back by id; phase S breaks one in survival and reads `drops{machine_frame_pity: 1}` |
| the frames' loot tables | `tested` — phase S's drop is the loot table; a frame outside `mineable/pickaxe` would drop nothing |
| the three gears and plastic | `tested` — phase R gives and reads each back; phases K/… craft the gears; the condensator consumes both gears (tag) and plastic |
| the 16 lenses | `tested` — phase R gives and reads each back; phase U crafts all 16 recolours |
| `industrialforegoing:machine_frame/{pity,simple,advanced,supreme}` (block + item) | `tested` — the pity tag is the condensator's `M` ingredient (phase K); the other three are carried for the add-ons and asserted by `verify_content.py` (values resolve) |
| `c:gears`, `c:gears/{iron,gold,diamond}`, `c:plastics` | `tested` — phases K and U craft through `#c:gears/iron` and `#c:plastics`; the umbrella `c:gears` has no recipe consumer (asserted as data only) |
| `c:dyes/<colour>` (16) | `tested` — phase U crafts all 16 recolours through them |
| `c:ingots/iron`, `c:storage_blocks/redstone` | `tested` — phase K crafts the pity frame through both |
| `industrialforegoing:laser_lens` (the port's "any lens" tag) | `tested` — phase U feeds a **black** lens to the white recolour, so the tag is what matched |
| the `core` creative tab | `not asserted in game` (a tab is a client screen; the ids are asserted registered and named) |
| the 21 carried recipe advancements | `carried, not exercised` — the load check asserts the game logged no refused recipe/advancement file; no command exposes a criterion |
| the simple/advanced/supreme frames' recipes, the lenses' own recipes | `blocked(industrialforegoing:dissolution_chamber)` — the next slice; those three frames are `give`-only and the lenses are craftable from each other |
| `plastic`'s own recipe | `not ported` — `smelting dryrubber`, and dryrubber is the latex chain (a later slice) |

### The changelog for the 0.2.4 update

> Supersedes the closed 0.2.3 PR of this entry. **New: the `ModuleCore` item and block set.** The four
> machine frames (iron-block template, their own state-free blockstate, models, textures, loot tables
> and pickaxe entries), the iron/gold/diamond gears, plastic and the 16 coloured laser lenses are
> registered, named and given their real tags — `industrialforegoing:machine_frame/<tier>` (block and
> item), `c:gears`, `c:gears/*`, `c:plastics` — and this package now also declares the convention tags
> its recipes name (`c:dyes/<colour>`, `c:ingots/iron`, `c:storage_blocks/redstone`) because 26.3's base
> data ships none. The Water Condensator's carried recipe names `#c:plastics`, `#c:gears/iron` and
> `#industrialforegoing:machine_frame/pity` again instead of slice 2's stand-ins. Proven in game: 24 ids
> read back, the four frames placed and one broken for its drop, the condensator crafted **from the
> tags** with every ingredient consumed (and the stand-in grid empty), the pity frame and the three
> gears crafted, and all 16 lens recolours crafted with the player's counts read before and after. Fixed
> upstream defects: the recolour recipes' 16-alternative ingredient list (26.3 drops such a recipe
> whole) became the `industrialforegoing:laser_lens` tag. `runtime.rs` is unchanged, byte for byte; the
> dissolution chamber, the latex/plastic machines and the frames' own recipes are the next slice.


## Slice 5 — the dissolution chamber and its own recipe type (0.2.5, 2026-11-21)

**Chosen from the port's own gap list and from the campaign's blocker record.** Gap 9's closing
sentence ("Still outside the slice: … the **dissolution chamber**") and gap 17 ("Three machine frames
and 16 lenses are `give`-only … their recipes are dissolution-chamber recipes"), plus
[`HANDOVER.md`](../_shared/HANDOVER.md) §9g, where `mifa` is parked on
`blocked(industrialforegoing:dissolution_chamber)` and its 22 recipes name the parent's
`industrialforegoing:dissolution_chamber` type 21×. Tier **machines** (ledger): a block entity with an
item container, two fluid tanks, an FE buffer, a tick and commands — no renderer, no dimension work. The
three higher frames, all 16 lenses and the four addon items this package already ships had **no source**
in this port without it; after this slice 23 of the jar's chamber recipes are carried and crafted.

### Upstream (jar `industrialforegoing-1.21-3.6.39.jar`; source `Buuz135/Industrial-Foregoing`
### `ce2852db`; Titanium `e53e869` — read for behaviour, clones deleted)

| upstream | behaviour | what the port carries |
|---|---|---|
| `DissolutionChamberBlock` | `Properties.ofFullCopy(IRON_BLOCK)`, `RotationType.FOUR_WAY`, `IndustrialBlockItem` in `TAB_CORE` | block + item, `subfacing` (four values), iron-block template, `core` tab, pickaxe tag, loot |
| `DissolutionChamberTile` | 8 input slots (`stacksTo(1)`, input only), 3 output slots (output only), an 8,000 mB input tank and a 16,000 mB output tank, a 10,000 FE buffer, 90 FE a tick | a declared `ModContainer` (11 slots, inputs 0–7, outputs 8–10), two declared tanks, the buffer, the tick |
| `DissolutionChamberRecipe` (`MapCodec`) | `input` (0–8 `Ingredient`s), `inputFluid` (`SizedFluidIngredient`: fluid + mB), `processingTime`, `output` (`ItemStack.CODEC`), optional `outputFluid`; `matches` needs **exactly** the recipe's item set and the tank's fluid | decoded from the package's own files (`src/dissolution.rs`), matched in `src/chamber.rs` |
| `IndustrialProcessingTile` + Titanium `ProgressBarComponent` | the bar rises one a tick (the speed augment's value from the second tick, because Titanium sets `progressIncrease` in `onTickWork` *after* `setProgress`), 90 FE is taken a tick, at `maxProgress` the bar resets and `onFinish` runs; `maxProgress` is the recipe's `processingTime` scaled by `floor(processingTime × efficiency)` at `onStart`; a bar that cannot work resets to 0 and takes nothing | the same arithmetic in `src/chamber.rs`, reusing `runtime::Augments` |

Config (`DissolutionChamberConfig`): `maxProgress` 100 (fallback only — the chamber's bar length is the
recipe's), `powerPerTick` **90** (its own comment says 60; the value is 90 and is what the game uses),
`maxStoredPower` 10,000, input tank 8,000 mB, output tank 16,000 mB.

### The recipe type — measured, and where the brief's symbols point

The brief names `mod_api::CraftingRecipeKind` + `Registrar::register_crafting_recipe_serializer` and
says `pocketstorage` proved them. They exist and work, but they register a **3×3 crafting-grid** kind
(`register_crafting_recipe_serializer` calls `add_recipe_serializer(name, "minecraft:crafting", …)`):
a chamber recipe placed in a crafting table, not a machine's own type. The pin's machine route is API
36's pair, and it is the one the game accepts:

- `Registrar::register_recipe_type("industrialforegoing:dissolution_chamber")` **and**
  `Registrar::register_raw_recipe_serializer("dissolution_chamber", …)` (the read-only
  `RecipeSerializer` whose value is the file's JSON, published through `Host::recipes_of_type`).
- **Without the serializer the game refuses the file and crashes.** Measured on the first build in
  game, verbatim: `recipe data/industrialforegoing/recipe/dissolution_chamber/advanced_machine_frame.json,
  unregistered recipe serializer industrialforegoing:dissolution_chamber` → `Horizon crashed: …`. The
  first probe's log is in this report; the fix is the serializer above.
- With it, the machine's table reads back complete: `/industrialforegoing info` prints `recipes 23/23`
  (23 files carried, 23 decoded), and a grid of the jar's lens ingredients matches
  `industrialforegoing:dissolution_chamber/white_laser_lens`.

### The fluid gap — measured, and the route the brief named

`native_fluid_insert` refuses anything but `minecraft:water`/`minecraft:lava`
(`game/machines/native_fluids.rs`), and it is the only fill path the pin gives a mod block's tank. The
port therefore:

1. **declares** the three fluids its recipes name as fluid keys — `industrialforegoing:latex`,
   `:pink_slime`, `:ether_gas` (`Registrar::declare_fluid`) — so a tank can hold one and the readout
   resolves it;
2. **writes the entity's own `fluids` compound** (the pin's tank shape: one `{Fluid, Amount}` per
   tank) through `merge_data`, as `/data merge block` does, from `/industrialforegoing fluid <pos>
   <fluid> <amount>`;
3. proves it through the **declared capability**: `/industrialforegoing info` reports the input tank
   as the capability sees it (`capability industrialforegoing:latex 2000 mB`), not from its own
   counter. Phase Y asserts that string; with the fluid not declared or not in the compound it reads
   `empty`.

### TDD, in order

1. **Red** (`src/chamber.rs`, the rules as tests; `sdk-test-red-slice5.log`): `test result: FAILED. 60
   passed; 15 failed` — e.g. `chamber::tests::the_exact_item_set_matches … assertion failed:
   recipe_for(&recipes, &held, &latex()).is_some()` and `…the_hundred_tick_recipe…: assertion
   `left == right` failed: tick 1 of a 100-tick recipe still works`. (The first red, before any body
   existed, was `49 passed; 17 failed`; the log is the same set of tests with the three bodies put
   back at their pre-implementation state.)
2. **The rules** (`src/chamber.rs`, a second pure module — `runtime.rs` stays byte-identical, which
   `verify-sdk-package.py` checks): `Tank`, `Ingredient`, `ChamberRecipe`, `Machine` (energy, bar, two
   tanks, the bar's step), `recipe_index`/`recipe_for` (the exact-multiset match), `bar_length`
   (`floor(ticks × efficiency)`), `tick` (the bar's rise, 90 FE a tick, the reset, the completion with
   its product and fluids). 20 tests, incl. the 100-tick/9,000 FE run, the 111-tick buffer limit, the
   speed augment's one-tick-late start and a 0.8 efficiency bar of 240.
3. **The glue** (`src/dissolution.rs`): the declarations, the recipe table (decoded once per table
   identity — the file's own allocation — so a `/reload` rebuilds it and a tick does not cross into
   the game), the tick, the container/energy/fluid declarations, the block's contents on its item,
   and the readout/setters.
4. **In game**, phases X–AE (below), `PASS` + `EXIT:0`.

### In game (phases X–AE; `evidence/industrialforegoing-e2e/e2e-launcher.log`)

| phase | what it asserts | the value it read |
|---|---|---|
| X | the chamber places from its item, faces its placer, and the machine reads its own recipe table | `subfacing=west`; `recipes 23/23`; fresh: `0 FE`, `input empty 0/8000`, `recipe none` |
| Y | a **mod fluid** in the declared tank, reported back by the capability | `input industrialforegoing:latex 2000/8000 mB`, `capability industrialforegoing:latex 2000 mB` |
| Z | the white lens recipe, a full 100-tick operation | 100 ticks (the game's own `t`), `10000 → 1000 FE` (100 × 90), `2000 → 1750 mB` (250 drained), `white_laser_lens ×1`, every input slot consumed |
| AA | **the fluid gate**: the same grid with an empty tank | `recipe none`, `progress 0`, `0 FE` spent, no product; 250 mB of latex later it runs and spends 9,000 FE |
| AB | the output into a hopper and a barrel | the barrel holds the lenses, the chamber's output slots are empty |
| AC | the simple frame's whole 300-tick recipe | the rate `90 FE a tick` measured between two reads, the bar climbing without resetting, `machine_frame_simple ×1`, `1000 → 750 mB` |
| AD | **the `mifa` point**: `#c:plastics` + the crafted `#c:gears/diamond` + `machine_frame/advanced` | `machine_frame_advanced ×1` from a grid naming those three, `1000 → 500 mB` of pink slime |
| AE | a broken chamber's item carries its buffer and fluid | broken at 4,000 FE / 1,500 mB, placed back at 4,000 FE / 1,500 mB |

`SELFTEST=chamber` flips Z's product count to 2 (`FAIL: the product is
industrialforegoing:white_laser_lens x1, expected … x2`, `EXIT:1`); the control leg (same Nightly,
no package) still fails at its first assertion (`unknown item industrialforegoing:water_condensator`).
The three legs of the earlier slices (`SELFTEST=1`, `SELFTEST=core`) were re-run on 0.2.5 and still
fail at their own assertions.

### Coverage (families this slice adds)

| family | coverage |
|---|---|
| `industrialforegoing:dissolution_chamber` (block + item) | `tested` — phase X places it from its item, reads the state (`subfacing=west`) and the loot drops it in the e2e's break of AE |
| its block state, model, item definition, name, `core` tab entry | `tested` by name/model/state where the game exposes them (X, AE); the tab is data (asserted by `verify_content.py`) |
| its own recipe type (`industrialforegoing:dissolution_chamber`) | `tested` — `recipes 23/23` (data loaded) and two recipes matched and run (Z, AC, AD) |
| the 8 input slots, 3 output slots, and their automation | `tested` — items enter through the container (`/item replace block … container.N`, a server-side insert) and leave into a hopper (AB); the output slots also drop with the machine (AE's break) |
| the 8,000 mB input tank / 16,000 mB output tank as capabilities | `tested` — Y reads the fluid back through the declared capability; the output tank has no carried recipe that fills it (see gaps) |
| the 10,000 FE buffer, 90 FE a tick | `tested` — Z's 100 × 90 = 9,000 FE and AC's measured rate |
| the recipe `input`/`inputFluid`/`processingTime`/`output`/`outputFluid` fields | `tested` for the first four (Z, AC, AD); `outputFluid` is decoded but no carried recipe has one (asserted as data) |
| the speed and efficiency augments on the chamber | `ported, not exercised in game` — the base tile's two augment slots are seated through the same `use_item_on` path the condensator's phases M–Q prove; the chamber's own e2e asserts the empty slots (`augments speed 1 efficiency 1.00`) and the arithmetic is unit-tested (20 tests), but no phase seats one in this machine |
| the jar's other 26 chamber recipes (range/infinity addons, `mechanical_dirt`, `mycelial_reactor`, `pink_slime_*`, `processing_addon_*` tier 3+, `xp_bottles`) | `not carried` — each names an **output** this package does not ship (or, `xp_bottles`, a fluid literally called `None`) |
| the four addon items' own recipes (tiers 1 and 2) | `carried` — their ingredients all resolve; asserted by `verify_content.py` and crafted-by-type (Z) rather than one by one in game |
| a GUI screen, the lockable input bundle, redstone control | `not ported` — `machines` tier without a menu, as slice 1 (gaps 1–2) |

### Deviations and gaps this slice adds

19. ~~**The fluids have no source in this package.**~~ **Closed in 0.2.6 (slice 6) for
    `industrialforegoing:latex`**: the Fluid Extractor produces it, and phase AI drains 400 mB of it
    out of the declared tank. **`:ether_gas` is closed in 0.2.8 (slice 8)**: the fluid laser base's
    ether recipe fills its tank with it (phase BJ/BK). **`:pink_slime` is the only declared fluid
    with no source** — its machine is a later slice — and the fill path has not changed; the
    original note follows.
    **The fluids have no source in this package.** `industrialforegoing:latex`, `:pink_slime` and
    `:ether_gas` are declared fluid keys, and a tank holds them by writing its own `fluids` compound;
    nothing in the port produced them (the latex/plastic machines were a later slice), and
    `native_fluid_insert` refuses a mod fluid, so `/industrialforegoing fluid` is the only fill path.
    A pack with the latex chain ported writes the same compound. Recorded, not faked.
20. **The input slots have no per-slot limit.** `DissolutionChamberTile` builds its input inventory
    with `setSlotLimit(1)`; the pin's `ModContainer` declares slots, faces and policies, not a slot
    limit, so a hopper can pile a stack in one slot. A pile still matches an ingredient **once**
    (the matcher counts occupied slots, as upstream's `matches` counts `handlerItems`), so a pile
    blocks the recipe exactly as upstream's would — but upstream never accepts the pile.
21. **The output tank is inert with the carried recipes.** No recipe this package carries has an
    `outputFluid` (the jar's only one is `pink_slime_ball`, whose input item this slice does not
    ship); the tank, its declaration and the completion's fill path are exercised by the unit tests,
    not in game.
22. **The chamber's augments are seated but not exercised in the e2e.** The seating path is the
    condensator's (`use_item_on` → `level_tick_post`), and the arithmetic is unit-tested; no phase of
    X–AE sits an addon in the chamber, so its in-game augment behaviour is untested (the condensator's
    phases M–Q cover the component and the queue).

### The changelog for the 0.2.5 update

> Supersedes the closed 0.2.4 PR of this entry. **New: the Dissolution Chamber** — the machine the
> parent's own add-ons and every higher machine frame depend on. It carries its **own recipe type**
> (`industrialforegoing:dissolution_chamber`, declared with its read-only serializer) and 23 of the
> jar's recipes: the simple, advanced and supreme machine frames (previously `give`-only), the 16
> coloured laser lenses (previously craftable only from each other) and the four speed/efficiency
> addon items (previously `give`-only). Eight input slots, three output slots, an 8,000 mB input tank
> and a 16,000 mB output tank of this mod's own fluids (`industrialforegoing:latex`, `:pink_slime`,
> `:ether_gas` — declared fluid keys; the tank is filled by `/industrialforegoing fluid`, because the
> pin's fluid insert takes only water and lava), a 10,000 FE buffer at 90 FE a tick, and the base
> tile's two augment slots. Proven in game: the machine places and faces its placer, its table reads
> 23/23, a white lens costs exactly 100 ticks and 9,000 FE and 250 mB of latex, an empty tank runs
> nothing and 250 mB starts it, a hopper empties the output into a barrel, the simple frame runs its
> 300 ticks at exactly 90 FE a tick, the advanced frame is made from `#c:plastics`, the crafted
> `#c:gears/diamond` gear and the simple frame, and a broken chamber carries its FE and latex back on
> its item. `src/runtime.rs` is unchanged, byte for byte; a second pure module (`src/chamber.rs`)
> holds the chamber's rules. The latex/plastic machines, the remaining chamber recipes (their products
> are not in this package) and the GUI are the following slices.

## Slice 6 — the latex chain: `fluid_extractor` + `latex_processing_unit` (0.2.6, 2026-11-22)

**The slice: the port's own deviation 19** — *"the fluids have no source in this package …
nothing in the port produces them (the latex/plastic machines are a later slice), and
`native_fluid_insert` refuses a mod fluid, so `/industrialforegoing fluid` is the only fill path"* —
together with deviation 17's last sentence: *"`plastic` is still `give`-only: its recipe is
`smelting dryrubber`, and `dryrubber` is the latex chain."* It is the family the condensator's own
`#c:plastics` ingredient and every add-on recipe that names plastic wait on. Tier **machines**
(ledger): two block entities with a fluid tank each, an FE buffer, one item container, a server
tick and commands — no renderer, no dimension work, no entity AI.

### The fluid-move question, settled first

The brief asked whether `native_move_fluids` (API 55) moves a **mod** fluid between two block
entities. It does not, for two independent reasons, both measured:

1. **It is not on the package's surface.** `move_fluids` is a `.hnmod` **helper**
   (`horizon/native-hooks.json`, `helpers[].name = "move_fluids"`), not a hook, and it has no SDK
   counterpart: `ports/_shared/sdk/0a875da/surface.json` and the `horizon-mod-vocab` host areas
   have no `move_fluids`, no `fluid_tank_move` and no equivalent — the SDK's fluid writes are
   [`fluid_tank_insert`](FluidsEnergyBackendMut::fluid_tank_insert) and
   `fluid_tank_extract` per tank. A package cannot call it at all.
2. **Even engine-side it moves nothing.** `native_move_fluids` takes from the source and then fills
   the target through `native_fluid_insert`, whose first statement is
   `if fluid != "minecraft:water" && fluid != "minecraft:lava" { return amount; }`
   (`game/machines/native_fluids.rs:206`); what the target refuses goes straight back into the
   source, so the net move of `industrialforegoing:latex` is zero. The same gate is what the SDK
   documents for `fluid_tank_capacity` ("none … a fluid the tanks there don't take: other than
   water and lava") and for `fluid_tank_insert`.

Proven in game, in phase AK, through the declared capability itself (the same path
`native_fluid_insert` serves): `/industrialforegoing fill <unit> water 1000` →
`accepted 1000 mB of minecraft:water (tank 1 took 1000 mB)`, and
`/industrialforegoing fill <unit> latex 100` →
`accepted 0 mB of industrialforegoing:latex (the pin's fill takes water and lava only)`. So the
tanks are filled by the command path — `/industrialforegoing fluid`, which merges the block
entity's own `fluids` compound as `/data merge block` does — and the report says so instead of
implying a move.

### Upstream (jar `industrialforegoing-1.21-3.6.39.jar`; source `Buuz135/Industrial-Foregoing`
### `ce2852db`, read for behaviour, clone deleted)

| upstream | behaviour | what the port carries |
|---|---|---|
| `FluidExtractorBlock` | `Properties.ofFullCopy(STONE)`, `RotationType.FOUR_WAY`, `TAB_CORE` | block + item, `subfacing` (four values), stone template |
| `FluidExtractorTile` | `IndustrialAreaWorkingTile(RangeType.BEHIND)`: a 100-tick bar counting **down**, `WorkAction(0.4f, 500)` powered / `(1f, 0)` idle, a 1,000 mB latex tank (`Action.DRAIN`), a 10,000 FE buffer | the tick, the tank, the buffer, the parts |
| `FluidExtractorRecipe` (+ its own `ModuleCore.FLUID_EXTRACTOR_TYPE`/`_SERIALIZER`) | `input` (Ingredient), `result` (BlockState), `breakChance`, `output` (FluidStack), `defaultRecipe`; `findRecipe` takes a named recipe before the default one | the recipe type + read-only serializer (API 36, as the chamber's), the jar's nine files, the decode in `src/latex.rs` |
| `work()` | the block **behind** the machine; an empty or full machine is `(1f, 0)`; else `output.amount × (powered ? 3 : 1)`; a `breakChance` roll raises a per-**position** counter shared by every extractor; the **eighth** hit resets it, sets the block to `result` and adds `output.amount × (powered ? 200 : 1)` | the same, with the counter in a static map and the roll from the level's own random |
| `LatexProcessingUnitBlock` | `ofFullCopy(STONE)`, `FOUR_WAY`, `TAB_CORE` | block + item |
| `LatexProcessingUnitTile` | `IndustrialProcessingTile`: a bar **rising** to 100, 20 FE a tick, latex and water tanks of 16,000 mB, nine output slots, `onFinish` = −750 mB latex, −500 mB water, +1 `dryrubber` | the same arithmetic, the tanks, the container |
| `ModuleCore.DRY_RUBBER` | `RecipelessCustomItem("dryrubber", TAB_CORE)` | the item, its texture/model/definition, the `core` tab |
| `plastic`'s recipe | `smelting dryrubber`, 200 ticks, 0.3 xp | carried, and crafted in a furnace in game |

`FluidExtractorConfig`: `maxProgress` 100, `powerPerOperation` **500**, `maxStoredPower` 10,000,
`maxLatexTankSize` **1,000** (its comment says 8,000; the value is what the game uses).
`LatexProcessingUnitConfig`: `powerPerTick` 20, `maxStoredPower` 10,000, both tanks 16,000; the
tile's `AMOUNT_LATEX` is **750** and `AMOUNT_WATER` **500** (the jar's own patchouli text says 75
and 1,000 — the code is what runs, and the port follows the code).

### TDD, in order

1. **Red** (`src/latex.rs`, the rules as 17 tests with three bodies at `todo!()`;
   `evidence/industrialforegoing/sdk-test-red-slice6.log`): **78 passed, 17 failed**, every failure
   `panicked at src/latex.rs: not yet implemented` — `recipe_for`, the extractor's `tick` and the
   unit's `tick`.
2. **The rules** (`src/latex.rs`, a third pure module — `runtime.rs` stays byte-identical, which
   `verify-sdk-package.py` checks): `ExtractorRecipe` + `recipe_for` (a named recipe before the
   default one), `Extractor` (bar down from 100, 500 FE an operation, the 40/100 cooldown, the tank
   clamp, the hit counter, the burst), `Unit` (the 750/500 gate, 20 FE a tick, the 100-tick bar,
   `floor(100 × efficiency)` with an efficiency augment, the speed augment one tick late).
3. **The glue** (`src/fluidchain.rs`): both block entity types, their Parts (placement rotation, the
   tick, the augment seating slice 3 shares, the item that carries the contents), the extractor's
   recipe table (decoded once per table identity, as the chamber's), the readouts and the setters.
   Ten glue tests: the declarations, the block behind, the tank round trip and its clamp, the
   carried patch's `id`, the shared counter.
4. **In game**, phases AF–AM (below), `PASS` + `EXIT:0`.

### In game (`evidence/industrialforegoing-e2e/e2e-launcher.log`, phases AF–AM)

| phase | what it asserts | the value it read |
|---|---|---|
| AF | the extractor places, faces its placer, sees the log **behind** it, reads its own recipe table | `subfacing=west`, `log minecraft:oak_log`, `recipe industrialforegoing:fluid_extractor/oak`, `recipes 9/9`; one unpowered operation: `latex 2 mB`, `0 FE` |
| AG | a powered operation is exactly three times the recipe | `latex 8 mB` (2 + 6), `4500/10000 FE` (500 spent), bar resting at 36/100 |
| AH | **the counterfactual**: the same machine facing stone | `log minecraft:stone`, `recipe none`, the tank and the buffer unchanged |
| AI | the eighth hit consumes the log and bursts 200× | `hits 7` + a pinned roll 0 → `hits 0`, the log became `minecraft:stripped_oak_log`, `latex +406 mB` (6 + 400), and `/drain 400` answered `drained 400 mB of industrialforegoing:latex` |
| AJ | a roll **over** the break chance moves nothing | `roll 1.0` → `hits 7` kept, the log stays an oak log, the operation still gave its 6 mB |
| AK | the unit takes latex and water and makes dry rubber | latex 1,000 mB by `/fluid`, water 1,000 mB by `/fill` (the capability), `fill latex` accepted **0 mB**; 100 ticks later `latex 250`, `water 500`, `8000/10000 FE`, `output industrialforegoing:dryrubber x1` |
| AL | the chain closes: a hopper feeds a furnace and the rubber smelts into plastic | the furnace's output `industrialforegoing:plastic x1`, its input empty, the unit's output empty |
| AM | a broken unit carries its tank and buffer | broken at `1500 mB` latex / `400 mB` water / `4000 FE`, placed back with exactly those numbers |

**Fail-capable, six ways, all `EXIT:1`:** the control leg (the same Nightly without the package)
stops at its first `give` (`FAIL: the machine item did not register`); `SELFTEST=1` flips B
(`FAIL: the water is 200, expected 201`); `SELFTEST=contents` flips L (`FAIL: the restored tank is
5000, expected 5001`); `SELFTEST=core` flips U (`FAIL: the red lens count after the craft is 1,
expected 2`); `SELFTEST=chamber` flips Z; and **`SELFTEST=latex`** — this slice's own — flips AF
(`FAIL: the latex one unpowered operation gives is 3, expected 2`). Phases AH and AJ are in-run
counterfactuals: the same machine with stone behind it and with a roll over its break chance.
`check-e2e-logs.py industrialforegoing-e2e --strict` reads **7/7**.

**Harness facts this slice added** (each cost a run):

- `ctl messages N` can return an **earlier** readout of the same machine when the reply lands a
  tick or two late; the readout helper now reads the game's own `[CHAT]` log (`ctl logs`) and waits
  for a line whose `t=` differs from the previous read's.
- A helper that took only `$5` truncated `fluid … latex 1000` to `fluid … latex`, and the
  dispatcher answered `Unknown or incomplete command` — a harness bug that reads exactly like a
  registration bug.
- A block entity keeps working while the harness talks to it: every phase parks the bar at 100 and
  reads a **before** value, so a finished bar between two commands cannot spend a pinned roll.
- `merge_data` replaces a list, so writing one tank from an empty patch drops the other; the
  command path now seeds its patch with the entity's own `fluids`.
- A hopper under a machine eats its drop; the contents leg takes the hopper out first.

### Coverage (families this slice adds)

| family | coverage |
|---|---|
| the `fluid_extractor` block and item | `tested` — AF places it from its item, reads `subfacing=west` and the block id back |
| its `industrialforegoing:fluid_extractor` recipe type and the jar's nine recipes | `tested` — AF/AI/AJ: `recipes 9/9`, a named log recipe and the `default` fallback both matched and run in game |
| `output.amount × 3` powered / `× 1` free, 500 FE an operation, the 40/100-tick cooldown | `tested` — AF (2 mB, 0 FE) and AG (6 mB, 500 FE, bar 36) |
| the block **behind** it (`RangeType.BEHIND`) | `tested` — AH: stone gives `recipe none` and no latex; AF: the oak log east of a west-facing machine |
| the hit counter and the eighth-hit consumption (`breakChance` 0.01) | `tested` — AI (a pinned roll 0 consumes the log and bursts 400 mB) and AJ (a roll of 1.0 does not) |
| the 1,000 mB latex tank as a capability | `tested` — AI drains 400 mB of `industrialforegoing:latex` out of it |
| the `latex_processing_unit` block and item | `tested` — AK places it and reads both tanks empty |
| the 750 mB latex + 500 mB water → dryrubber operation | `tested` — AK: 100 ticks, `dryrubber x1`, latex 1,000 → 250, water 1,000 → 500, 2,000 FE |
| its two 16,000 mB tanks | `tested` — latex by the command path, water through the **declared capability** (`fill water 1000` accepted), both reported back by the capability |
| the nine output slots | `tested` — AK reads the product out of them; AL empties them through a hopper into a furnace |
| `industrialforegoing:dryrubber` | `tested` — AK makes it, AL smelts it, R gives and reads it back |
| the `smelting dryrubber -> plastic` recipe | `tested` — AL: a furnace turns it into `industrialforegoing:plastic x1` and consumes the rubber |
| the machines' items carrying their contents | `tested` — AM (the unit: latex, water, FE); the extractor carries its tank and bar by the same `block_drops` path |
| the augments on both machines | `ported, not exercised in game` — both seat through the same `use_item_on` path the condensator's phases M–Q prove, and the unit's arithmetic is unit-tested (the efficiency bar, the speed tick); no phase seats one in either machine |
| a GUI screen, redstone control, a comparator | `not ported` — gaps 1, 2, 7 |
| the jar's other machines (mycelial, conveyors, laser drills, the refinery, the sewage chain) | `not ported` — gap 9 |

### Deviations and gaps this slice adds

23. **The latex tank's fill face is closed on the unit.** Upstream's latex tank is fillable (with a
    latex validator); the pin's `ModFluidTanks` has no per-tank fluid filter and
    `native_fluid_insert` fills a mod block's tanks with any water/lava it is handed into *any*
    input tank holding that fluid or none — so a water fill could land in the latex tank. The
    latex tank therefore declares no fill face and the water tank is the only input: nothing can be
    inserted into the latex tank at this pin anyway, because a mod fluid has no insert path at all.
    `/industrialforegoing fluid` writes it, as slice 5's chamber's tank is written.
24. **The break roll is the level's own random, with a harness override.** `breakChance` is rolled
    against `level.random.nextDouble()` (`LevelRef::with_random`), exactly as upstream. Because an
    0.01 chance needs ~800 operations to consume one log (about half an hour of game time),
    `/industrialforegoing roll <pos> <0..1>` pins the next operation's roll, and
    `/industrialforegoing extraction <pos> <hits>` sets the shared counter — the only way the
    eighth-hit path can be asserted in a bounded run. Both are OP-level harness controls, like
    `energy`/`progress`.
25. **The hit counter is in memory, per position and dimension.** Upstream's
    `FluidExtractorTile.EXTRACTION` is a static map keyed by dimension type, chunk and position, and
    it is not saved either; the port keeps the same map, keyed by dimension id and position, and a
    restart resets it as upstream's does.
26. **The extractor keeps extracting from the stripped log.** After the eighth hit the block
    becomes a stripped log, which `#minecraft:logs` still names, so the `default` recipe answers it
    at 1 mB an operation — upstream's behaviour with the same data, and phase AI reads exactly that
    (`recipe industrialforegoing:fluid_extractor/default` after the strip). Not a defect: it is what
    the jar's own recipes do.


## Slice 7 — the laser drill and its own ore recipe type (0.2.7, 2026-11-22)

**The slice: gap 9's "the laser drills"** — the family the 0.2.6 report named as the next one, and
the surface `industrialforegoingsouls` is blocked on (`blocked(missing-parent-recipe-type:
industrialforegoing:laser_drill_ore)`, HANDOVER §9g/§9h). Its jar carries **one** `laser_drill_ore`
file (echo shard), and it ships no recipe type: ours is what it loads under. The jar's own
`recipe/laser_drill_ore/` holds 42 files; ten name an output this package or vanilla has (coal,
diamond, emerald, lapis, nether quartz, redstone, glowstone, ancient debris, raw iron, raw gold) and
the other 32 name other mods' ores. Tier **machines** (ledger): two block entities, one item
container, one FE buffer, one recipe type and commands — no renderer, no entities.

### Upstream (jar `industrialforegoing-1.21-3.6.39.jar`; source `Buuz135/Industrial-Foregoing`
### `ce2852db`, Titanium `e53e869`, read for behaviour, clones deleted)

| upstream | behaviour | what the port carries |
|---|---|---|
| `LaserDrillBlock` | `Properties.ofFullCopy(IRON_BLOCK)`, `RotationType.SIX_WAY`, `TAB_RESOURCE` | block + item, `facing` (six values), iron-block template |
| `LaserDrillTile` (`IndustrialAreaWorkingTile(BEHIND)`) | a 50-tick bar counting **down**; at zero `work()`; `WorkAction(1, 1000)` when a laser base is in the cube two blocks along the opposite of `facing` and the buffer holds 1,000 FE, else `WorkAction(1, 0)`; the speed augment falls the bar faster (from the second tick), the efficiency one shortens it | the tick, the target scan, the queued advance, the buffer |
| `OreLaserBaseBlock` | `ofFullCopy(IRON_BLOCK)`, no rotation, `TAB_RESOURCE` | block + item, state-free blockstate |
| `OreLaserBaseTile` | a 100-tick bar **rising**, advanced only by the drill; at the top `onWork()`: every recipe whose **first** matching rarity passes depth + biome-tag + dimension-type filters is a weighted candidate; weight = rarity weight + 8 per lens slot holding the catalyst; `WeightedRandom` picks one; one stack into the 15 output slots; six lens slots, every face mode `NONE`; **no** FE of its own | the bar, the filters, the weighted pick, the six + fifteen slots |
| `LaserDrillOreRecipe` (+ `ModuleCore.LASER_DRILL_TYPE`/`_SERIALIZER`) | `output` (`SizedIngredient`: item or `#tag` + count), `catalyst` (an `Ingredient`), `rarity[]` (`biome_filter`, `dimension_filter`, `depth_min/max`, `weight`), optional `entity_data` | the recipe type + read-only serializer (API 36, as the chamber's and the extractor's), the ten decoded rules |
| `LaserDrillRarity.getValidRarity` | the **first** rarity whose depth band contains `miningDepth`, whose dimension-type filter passes and whose biome-tag filter passes | `laser::Rarity::matches`, `OreRecipe::rarity_for` |
| configs | drill `maxProgress` 50, `powerPerOperation` 1000, `maxStoredPower` 10000; base `maxProgress` 100, `catalystModifier` 8 | the constants |

The book (`patchouli …/ore_laser_base.json`) says it: *"Each ore that can be produced has a weight
based in the biome the machine is placed in and the depth the machine is configured to mine in."*
The pin expresses all three filters — `LevelRef::dimension_type`, `LevelRef::biome_at`,
`LevelRef::biome_in_tag` — so the port implements the jar's rule, not a stand-in.

### The brief's "the ore the base sits on", resolved

The brief's assertion reads "the ore the base sits on"; the jar's rule is **biome + depth +
dimension**, and the base *never* reads the block under it. The pin can read the biome's tags, so the
faithful rule ships and the e2e's counterfactual is the **End's own biome** (no recipe valid), not
the support block: the base sits on the same stone floor in both legs. Recorded here instead of
quietly changing the mechanic.

### TDD, in order

1. **Red** (`src/laser.rs`, 14 rules as tests with `todo!()` bodies; `sdk-test-red-slice7.log`):
   `test result: FAILED. 106 passed; 14 failed`, every failure `not yet implemented`, e.g.
   `laser::tests::the_drill_bar_falls_one_a_tick_and_charges_the_base_at_zero … panicked at
   src/laser.rs:231:9: not yet implemented: true Augments { speed: 1, efficiency: 1.0 }`.
2. **The rules** (`src/laser.rs`, a fourth pure module — `runtime.rs` stays byte-identical, which
   `verify-sdk-package.py` checks): `Filter` (the jar's empty-whitelist rule), `Rarity`, `OreRecipe`,
   `candidates` (weights with the lens bonus), `pick` (the weighted draw), `Drill` (bar down, 1,000 FE
   a charge, speed one tick late, efficiency bar), `Base` (bar up, `floor(100 × efficiency)`).
3. **Glue red** (`src/laserdrill.rs`, 9 tests; `sdk-test-red-slice7-glue.log`): **122 passed, 9
   failed** — `not yet implemented` in the declarations and shapes, plus the engine registration test
   (the `todo!()` `declare` refused the package). Then the glue: declarations, the recipe table
   (decoded once per table identity), the six-way placement, the tick, the pending advance queue, the
   readouts, the setters, the carry/clamp. Green **131**.
4. **Content** (50 files) with `tools/assemble_laser.py` (`--check`), and `verify_content.py`'s two
   new families plus two selftest cases. Green **134**.

### In game, phases BA–BH (`evidence/industrialforegoing-e2e/e2e-launcher.log`)

| phase | what it asserts | the value it read |
|---|---|---|
| BA | the base and the drill are placed (the drill from its item while the placer looks down, `SIX_WAY`) and the drill's working area finds the base; the base reads its own table | `facing up`, `target [232, 101, 0]`, `recipes 10/10`, `depth 101`, `dimension minecraft:the_end`, `biome minecraft:the_end`, `recipe none` |
| BB | a mountain biome makes the emerald recipe the only match | `biome minecraft:jagged_peaks`, `recipe industrialforegoing:laser_drill_ore/emerald_ore`, `weight 1` |
| BC | a lime lens in a lens slot adds the catalyst modifier | `lens industrialforegoing:lime_laser_lens`, `weight 9`; back to `weight 1` after the lens is taken out |
| BD | one completed base bar, fed 10,000 FE: the recipe's ore and the charge | `output minecraft:emerald_ore x1`, `9000/10000 FE`, base `progress 0/100` |
| BE | **the counterfactual**: the same charge in the End's own biome | `recipe none`, the output unchanged, `8000/10000 FE` (the drill still pays) |
| BF | a second drill placed from its item while looking east | `facing west` |
| BG | **the add-on's own file and biome tag**, loaded as its package ships them | the pack `[file/if-souls-surface (world)]` enabled, `recipes 11/11`, then `biome minecraft:deep_dark`, `depth 0`, `recipe industrialforegoingsouls:laser_drill_ore/echo_shard`, and one operation yields `minecraft:echo_shard x1` |
| BH | a broken base drops itself and its slots; its item keeps the bar and the depth | `drops {ore_laser_base: 1, emerald_ore: 1, echo_shard: 1}`; placed back at `progress 40/100`, `depth 7`, `output empty` |

The transcript's own lines:

```
== BA: the ore base resolves no ore in the End's own biome, and the drill sees it ==
  Laser Drill at [232, 102, 0]: 0/10000 FE, progress 44/50, target [232, 101, 0], augments speed 1 efficiency 1.00, facing up, t=4079
  Ore Laser Base at [232, 101, 0]: progress 0/100, depth 101, biome minecraft:the_end, dimension minecraft:the_end, recipe none, weight 0, recipes 10/10, lens empty, output empty, augments speed 1 efficiency 1.00, t=4084
== BB: a mountain biome makes the emerald recipe the only match ==
  Ore Laser Base at [232, 101, 0]: progress 0/100, depth 101, biome minecraft:jagged_peaks, dimension minecraft:the_end, recipe industrialforegoing:laser_drill_ore/emerald_ore, weight 1, recipes 10/10, …
== BC: a lime lens in a lens slot adds the catalyst modifier ==
  … weight 9, recipes 10/10, lens industrialforegoing:lime_laser_lens, …
  weight 1 -> 9 with a lime lens, back to 1 after
== BD: one completed base bar makes the recipe's ore for the drill's 1,000 FE ==
  … 9000/10000 FE, progress 43/50, target [232, 101, 0], …
  … output minecraft:emerald_ore x1, …
  9000 FE left, minecraft:emerald_ore x1, bar 0/100
== BE: the counterfactual — the End's own biome, the same charge, nothing mined ==
  … 8000/10000 FE … recipe none … output minecraft:emerald_ore x1 …
== BF: a drill placed from its item while looking east faces west ==
  Laser Drill at [236, 101, 0]: 0/10000 FE, progress 42/50, target none, …, facing west, t=4208
== BG: the add-on's own laser_drill_ore recipe loads under this package's type ==
  There are 2 data pack(s) enabled: [vanilla (built-in)], [file/if-souls-surface (world)] / There are no more data packs available
  … recipe none, …, recipes 11/11, …
  … recipe industrialforegoingsouls:laser_drill_ore/echo_shard, weight 1, recipes 11/11, …
  … output minecraft:emerald_ore x1, minecraft:echo_shard x1, …
  11/11 recipes; the add-on's industrialforegoingsouls:laser_drill_ore/echo_shard ran: minecraft:echo_shard x1
== BH: a broken base drops itself and its slots, and its item keeps the bar ==
  break: {"block":"minecraft:air","drops":{"industrialforegoing:ore_laser_base":1,"minecraft:echo_shard":1,"minecraft:emerald_ore":1},"picked_up":{}}
  Ore Laser Base at [232, 101, 0]: progress 40/100, depth 7, …, output empty, …
```

**The add-on count: 1/1.** `industrialforegoingsouls` carries exactly one `laser_drill_ore` recipe
(echo shard); its file loads under this package's type (the table moves 10 → 11) and it **runs** when
its own biome tag says so — the surface its `blocked(missing-parent-recipe-type:
industrialforegoing:laser_drill_ore)` row named.

**Fail-capable, three ways, all `EXIT:1`:** the control leg (the same Nightly, no package) stops at
its first `give` (`FAIL: the machine item did not register`); `SELFTEST=1` flips phase B
(`FAIL: the water is 200, expected 201`); and **`SELFTEST=laser`** — this slice's own — flips BD's
charge (`FAIL: the drill's buffer after one charge (10,000 - 1,000 FE) is 9001, expected 9000`).
Phases BE (the End's own biome), BH (the drops) and the "recipe none" reads are in-run
counterfactuals. `check-e2e-logs.py industrialforegoing-e2e --strict` reads **8/8**.

### Upstream defects fixed

1. **The two machines' loot tables could never drop them.** The jar's pool is
   `"conditions": [{"condition": "minecraft:survives_explosion"}]`; the engine's condition parser
   reads a condition object's `type` (26.3's shape), so the object's kind is empty and the pool is
   skipped — measured: a base broken with two ores in it dropped the ores and **not itself**
   (probe, 2026-11-22). The package's other eight machines already use a pool-level
   `"condition": {"type": "minecraft:survives_explosion"}`, which the engine does not read (an
   unconditional self-drop); the two new tables are written in that shape, and phase BH asserts the
   drop. A user notices that an ore base or a laser drill can be picked up again.
2. **The jar's `output` is an ingredient object** (`{"item": …}` / `{"tag": …}`), and the pin's
   `Host::recipe_ingredient` reads an object only for a NeoForge kind. The decode flattens
   `item`/`tag` to a bare id / `#tag` before asking (one in-game run showed `recipes 0/10` with ten
   files kept — the diagnosis is the red→green of `the_ingredient_forms_the_recipe_files_use_are_all_read`).
   This is what makes a **foreign** pack's ore file (`catalyst: {"item": …}`, exactly as the jar
   writes it) load, which the add-on leg proves.

### Deviations and gaps this slice adds

27. **The ore is chosen by biome tags + depth + dimension, not by the support block** — the jar's own
    rule (the book above), expressed with the pin's `biome_at`/`biome_in_tag`/`dimension_type`. The
    brief's "the ore the base sits on" is read as the ore its place resolves; the counterfactual is
    the End's own biome.
28. **`entity_data` recipes are left out of the table.** `LaserDrillOreRecipe.entity_data` gates a
    recipe on nearby entities of a kind (the jar uses it for mob-derived ores); this machine slice
    does not port entity matching, so such a file is skipped and the readout counts it on the
    `files` side (`recipes n/m`). None of the ten or the add-on's one uses it.
29. **32 of the jar's 42 ore recipes are not carried**: their outputs are other mods' ores
    (cinnabar, sodalite, fluorite, bitumen, ruby, niter, peridot, sapphire, draconium, yellorite,
    sulfur, arcane) or raw-material tags of metals no package here ships. A pack with those mods
    provides its own files and this type loads them.
30. **A broken base's slots drop as items, not inside the machine.** The engine drops a mod
    container's slots itself (`preRemoveSideEffects` → `Containers.dropContents`, measured), so the
    item's `block_entity_data` **strips `Items`** — carrying both would duplicate the contents. The
    bar, the depth and the augments still travel on the item (phase BH).
31. **The base accepts only the Efficiency augment** (`OreLaserBaseTile.canAcceptAugment`); its bar
    is advanced by the drill, so Speed has nothing to move. The drill takes both. The engineering of
    the arithmetic is the shared `runtime::Augments`, and no phase seats one on either machine (the
    condensator's M–Q cover the component and the seat).
32. **`miningDepth` is set by `/industrialforegoing depth`** — upstream's GUI edit box. It starts at
    the base's own Y (and the `-100` sentinel resets it there), as `setChanged` does.
33. **No GUI screen or tooltip** (gap 1): the readouts are the commands, as for the other
    machines. No comparator, redstone modes or config file (gaps 2, 7, 8).
34. **The output choice prefers `minecraft:` first** (`TagConfig.ITEM_PREFERENCE` upstream; this
    package ships no `TagConfig`), then the file's own order. Only matters for a tag output with
    more than one item; the ten recipes' tags hold one item each.
35. **The two new loot tables carry the package's `bonus_rolls`/`rolls` spelling.**
    `check_261_dialect.py` flags `"bonus_rolls": 0.0` and `"rolls": 1.0` on them exactly as it flags
    the eight tables carried before this slice; HANDOVER §9d.13 records those two fields as
    **unverified** ("no game has shown them to change behaviour; a flag is not a defect"), and phase
    BH reads the drop, so the slice keeps the package's shape rather than mixing two dialects.
36. **The era's per-slot limit is not expressible** (`SidedInventoryComponent.setSlotLimit(1)`): as
    with the chamber (deviation 20), the harness writes slots directly and a hopper could pile a
    stack in one; the matcher counts occupied slots, as upstream's does.

### Harness facts this slice paid for

- **`ctl messages` can return an older readout; `ctl logs` is complete within a session but does
  not carry a session after a relaunch** (measured with markers: both a before- and an
  after-relaunch `/say` vanished from `ctl logs`, while `ctl messages` held only the new session's
  chat). The readout helper therefore takes the live window after a relaunch and the log before it.
- **`grep -m1` in a chat window reads the oldest line of the window**: the chamber's AC leg read a
  stale 1,000 mB tank. Every helper now takes the newest matching line and waits for a *new* one.
- **A one-cell `/fillbiome` does not move `LevelReader.getBiome`**: the biome zoom averages the
  four surrounding cells, so the whole 16×16×16 box is filled. A single cell reads back
  `minecraft:the_end`.
- **`/reload` discovers and enables a datapack added while the game runs, but not its biome tag**:
  the level's biome tag table is built at world load, so the add-on leg writes the pack and reopens
  the world (its recipe would load either way; its tag needs the reload of the registry).
- **`look-at-block` refuses beyond vanilla reach and the pitch threshold is 40 degrees**: a placer
  five blocks up and one east aims steeply enough to place a `SIX_WAY` block `facing up` while
  staying out of the block being placed (two runs were lost to a 40-degree aim and to landing on the
  base).
- **A readout is not a product**: BD reads the base's `info` and the container, and BH reads both
  the drop set and the placed machine.

### Coverage (families this slice adds)

| family | coverage |
|---|---|
| `ore_laser_base` (block + item) | `tested` — BA places it from its item and reads it back; BH breaks it and reads `drops{ore_laser_base: 1}` |
| `laser_drill` (block + item), `SIX_WAY` placement | `tested` — BA places it from its item while looking down (`facing up`, target found); BF places a second while looking east (`facing west`) |
| `industrialforegoing:laser_drill_ore` (recipe type + serializer) | `tested` — BA reads `recipes 10/10`; BG loads a foreign file of the type and reads `11/11` |
| the ten carried ore recipes | `tested` — BB/BD (emerald, matched and run); the content rules assert the other nine's outputs, catalysts and depth bands |
| the biome-tag / depth / dimension-type filters | `tested` — BA (`none` in the End), BB (`emerald` in a mountain biome), BE (back to the End, `none`), BG (`deep_dark` at depth 0) |
| the weighted pick | `unit-tested` (`the_weighted_pick_follows_the_total_weight`); with one valid recipe in game the pick is that recipe |
| the lens catalyst weight (+8 a slot) | `tested` — BC: `weight 1` → `9` with a lime lens, back to `1` |
| the six lens slots / fifteen output slots | `tested` — BC writes a lens through `container.0`; BD/BG read the product from `container.6`/`7`; the face modes are a unit test |
| the drill's 1,000 FE a charge, bar and target | `tested` — BD: `9000/10000 FE`, base bar 0; BE: the same charge with no recipe still pays (`8000`) |
| the base's 100-tick bar | `tested` — BD forces 99 and the charge completes it; the arithmetic is `laser::Base` |
| the machines' items carrying their bar/depth | `tested` — BH: `progress 40/100`, `depth 7` on the placed base; the slots dropped separately |
| the containers' contents on a break | `tested` — BH: the emerald and the echo shard drop as items (the engine's own contents drop) |
| the augments on either machine | `ported, not exercised in game` — the base refuses Speed (glue), the drill seats both through the shared `use_item_on`; no phase seats one |
| `entity_data` recipes | `not ported` — deviation 28, none of the carried files uses it |
| the `fluid_laser_base` and the laser-drill fluid recipes | `tested` — slice 8 (phases BI–BM): the tank machine under the same drill, the ether recipe at depth 101 and not 300, 10 mB a bar in the declared tank, the drain through the capability and the chamber's 135 mB premium recipe; the lava recipe is `carried, not exercised` and oil is data (deviation 37) |
| GUI screens, redstone modes, comparator, config | `not ported` — gaps 1, 2, 7, 8 |

### The changelog for the 0.2.7 update

> Supersedes the closed 0.2.6 PR of this entry. **New: the laser drill and its ore base**, with their
> own recipe type `industrialforegoing:laser_drill_ore` (declared with its read-only serializer —
> the id an add-on's own files load under; `industrialforegoingsouls` is blocked on it). The drill is
> a `SIX_WAY` machine with a 10,000 FE buffer, a 50-tick bar and one charge of 1,000 FE per advance
> of the base's bar; the ore base has a 100-tick bar, a `miningDepth` (its own Y until `/depth`
> changes it), six lens slots and fifteen output slots, and picks a weighted ore by the biome tags
> and dimension type at that depth, with +8 weight a matching lens slot. Ten of the jar's 42 ore
> recipes ship (the ones whose ores vanilla or this package has); the other 32 name other mods'
> ores. Proven in game: the drill placed above the base faces up and finds it, the table reads
> 10/10 and resolves `none` in the End's biome and emerald at weight 1 in a mountain one, a lime
> lens raises it to 9, one completed bar yields `emerald_ore x1` for exactly 1,000 FE (and nothing
> when the biome does not match, while the FE is still spent), and the add-on's own
> `echo_shard` recipe — installed as its package ships it — loads under this type (11/11) and runs
> (`echo_shard x1`). Fixed: both machines' loot tables used the jar's condition shape, which the
> engine reads as an empty kind, so a broken machine dropped its contents but not itself; they use
> the package's 26.3 shape and BH reads the self-drop. A broken base drops its slots as items (the
> engine's own contents drop) and its item carries the bar and the depth. `src/runtime.rs` is
> unchanged, byte for byte; `src/laser.rs` is a fourth pure module.

## Slice 8 — the fluid laser base and `industrialforegoing:laser_drill_fluid` (0.2.8, 2026-11-22)

**The slice: slice 7's own coverage row** — *"the `fluid_laser_base` and the laser-drill fluid
recipes — `not ported` — a later slice (a tank machine and its own fluid type)"* — together with gap
9's *"the other laser bases"* and deviation 19's remaining sentence: *"`:pink_slime` and `:ether_gas`
still have no source (their machines are later slices)"*. Tier **machines** (ledger): one block
entity, one fluid tank, one lens slot, one recipe type and commands — **no FE of its own**, no
renderer, no entity work. It closes `:ether_gas`'s source and leaves `:pink_slime` the only declared
fluid with none.

### Upstream (jar `industrialforegoing-1.21-3.6.39.jar`; source `Buuz135/Industrial-Foregoing`
### `ce2852db`, read for behaviour, clone deleted)

| upstream | behaviour | what the port carries |
|---|---|---|
| `FluidLaserBaseBlock` | `Properties.ofFullCopy(IRON_BLOCK)`, no rotation, `TAB_RESOURCE` | block + item, the state-free blockstate, the iron-block template, the `resource` tab, the pickaxe entry, its loot table |
| `FluidLaserBaseTile` (`IndustrialMachineTile`) | a **20-tick bar rising**, advanced **only by a laser drill** (the drill's `isValidTarget` is any `ILaserBase`); at the top `onWork()` takes the **first file-order** recipe whose catalyst sits in the lens slot, whose first valid rarity answers depth/biome/dimension and whose optional entity gate passes; `fillForced` adds `output.amount`; **no energy component** (`EnergyStorageComponent(0, 4, 10)`) | the bar, the first-match rule, the tank fill, the shared drill queue |
| `SidedFluidTankComponent("output", 32000, …, Action.DRAIN)` | every face drains, none fills, the machine fills itself | `ModFluidTanks` — 32,000 mB, drain-only, contents in the block entity's own `fluids` compound |
| `SidedInventoryComponent("lens", …, 1, 0)` | one catalyst slot, every face mode `NONE` | `ModContainer` — one slot, no face reaches it |
| `LaserDrillFluidRecipe` (+ `ModuleCore.LASER_DRILL_FLUID_TYPE`/`_SERIALIZER`) | `output` (sized fluid), `catalyst` (Ingredient), optional `entity_data`, `rarity[]` | the recipe type + read-only serializer (API 36, as the chamber's, the extractor's and the ore base's), the jar's three files, the decode in `src/fluidbase.rs` |
| `LaserDrillRarity.getValidRarity` | the **first** rarity whose depth band holds `miningDepth`, whose dimension-type filter passes and whose biome-tag filter passes | `fluidlaser::FluidRecipe::rarity_for`, reusing slice 7's `laser::Filter` |
| `entity_data` | a living entity of the named kind with health > **5** in the 3×3×3 box (one below to two above); its `data` must be a subset of the entity's NBT; the match takes **5** health | `EntityGate`, `data_matches`, `level.entities_in` + `host.living` + `entity.hurt(…, "minecraft:generic", 5.0, …)` |
| configs | `maxProgress` **20**, tank **32,000 mB**, entity minimum health 5, damage 5 | the constants (`the_constants_are_the_jars_own`) |

`FluidLaserBaseConfig`: the bar's 20 ticks and `catalystModifier` do **not** apply here — the fluid
base has no weighted pick (the ore base's weight is `laser`-side), so the jar's `catalystModifier` is
not in this machine's tick.

### The brief's assertion, corrected from the artefact

The brief asks for "a biome the **ether** recipe names" and a counterfactual that is "the End's own
biome". The jar's `laser_drill_fluid/ether.json` names **no biome**: its `biome_filter` whitelist and
blacklist are both empty (every biome), its depth band is the whole world (−64…256), and its gate is
`entity_data` on a nearby `minecraft:wither`. The two files that do name a place are **lava** (the
Nether, depth 5–20) and **oil** (ocean/desert, depth 20–60), and oil's output is
`pneumaticcraft:oil` — another mod's fluid key, which a standalone package cannot declare, so the
game keeps the file and this table counts it on the `files` side (`recipes 2/3`).

So the in-game leg is the **same box, same purple lens, same charge**, differing only in the thing
that decides: **no wither → `recipe none`, tank empty, the drill still pays 1,000 FE** (phase BI); a
wither → the ether recipe and 10 mB into the declared tank (BJ); and **the depth band** is the
differential inside BJ — with the wither present, `/depth 300` (outside 256) reads `recipe none` and
`/depth 101` reads the ether recipe, with no FE spent between the two reads. The biome half of the
rarity filter is exercised by slice 7's phases BA/BB/BE on the ore base, which shares
`laser::Filter`; no shipped fluid recipe has a biome whitelist that this package can produce
(deviations 37–38).

### TDD, in order

1. **Red** (`src/fluidlaser.rs`, `choice`, `Tank::fill`, `Base::advance` and `data_matches` back at
   `todo!()`; `sdk-test-red-slice8.log`): **142 passed, 6 failed**, each `not yet implemented` —
   e.g. `fluidlaser::tests::the_first_recipe_whose_catalyst_rarity_and_gate_pass_is_the_one_used …
   panicked at src/fluidlaser.rs:106:5: not yet implemented`, and
   `the_tank_adds_its_fluid_up_to_the_capacity … src/fluidlaser.rs:137:9`.
2. **The rules** (`src/fluidlaser.rs`, a **fifth** pure module — `runtime.rs` stays byte-identical,
   which `verify-sdk-package.py` checks): `EntityGate`, `FluidRecipe` (`catalyst_matches`,
   `rarity_for`), `choice` (file order, catalyst, first valid rarity, gate), `Tank` (32,000 mB, the
   same-fluid clamp), `Base` (20 ticks, `floor(20 × efficiency)`), `data_matches` (the `entity_data`
   subset). Eight tests.
3. **The glue** (`src/fluidbase.rs`): the declarations (block entity type, drain-only tank, lens
   slot, Part), the recipe table decoded once per table identity (as the chamber's and the
   extractor's), the drill's queued advance, `on_work` (the first match, the entity gate, the 5
   damage, `fillForced`), the saved compound, item carry/clamp, the readout and the setters. Five
   glue tests: `the_base_declares_one_lens_slot_reachable_from_no_face`,
   `the_tank_is_drain_only_and_thirty_two_thousand_mb`,
   `the_tank_round_trips_through_the_saved_compound`,
   `a_broken_base_carries_its_tank_and_not_its_lens`,
   `a_placed_base_clamps_what_its_item_carried`.
4. **`src/laserdrill.rs`** now targets **either** laser base (`isValidTarget` = any `ILaserBase`):
   the advance queue is shared, so one drill charges an ore base or a fluid base with the same
   1,000 FE a step.
5. **Content** (16 files) with `tools/assemble_fluidlaser.py` (`--check`), and
   `verify_content.py`'s new family (`the_fluid_laser_base_and_its_fluid_recipes_are_carried`) plus
   two selftest mutations.

Green: **148 passed, 0 failed** (`sdk-test.log`, `Compiling industrialforegoing v0.2.8`).

### In game, phases BI–BM (`evidence/industrialforegoing-e2e/e2e-launcher.log`)

| phase | what it asserts | the value it read |
|---|---|---|
| BI | the fluid base under its own drill, purple lens, depth 101, a charged bar, **no wither in the box** | `target [240, 101, 0]`; `tank empty 0/32000 mB`, `capability empty 0 mB`, `recipe none`, `recipes 2/3`; the drill `9000/10000 FE` (the charge still cost it 1,000) |
| BJ | a **NoAI wither in the box**, then the **depth band** as the differential, then one completed bar | at depth 300 `recipe none`; at depth 101 `recipe industrialforegoing:laser_drill_fluid/ether`; then `tank industrialforegoing:ether_gas 10/32000 mB`, `capability industrialforegoing:ether_gas 10 mB`, drill `8000/10000 FE` |
| BK | a second completed bar adds the same amount | `tank … 20/32000 mB`, drill `7000/10000 FE` |
| BL | the tank drains through the **declared capability** | `/drain 10` → tank 20 → 10, capability 10 |
| BM | the chamber runs the one carried recipe that names ether gas | `supreme_machine_frame`: 135 mB of `industrialforegoing:ether_gas` + the eight items → `machine_frame_supreme x1`, ether 0, items 0 |

The transcript's own lines:

```
== BI: the fluid base with no wither answers no recipe, and the charge still costs FE ==
  Laser Drill at [240, 102, 0]: 9000/10000 FE, progress 43/50, target [240, 101, 0], …, facing up, t=4693
  Fluid Laser Base at [240, 101, 0]: tank empty 0/32000 mB, capability empty 0 mB, progress 0/20, depth 101, biome minecraft:deep_dark, dimension minecraft:the_end, recipe none, recipes 2/3, lens industrialforegoing:purple_laser_lens, augments speed 1 efficiency 1.00, t=4704
  2/3 recipes, none without a wither, tank 0 mB, 9000 FE
== BJ: a wither in the box makes the ether recipe, and one bar fills the tank ==
  Fluid Laser Base at [240, 101, 0]: tank empty 0/32000 mB, …, depth 300, …, recipe none, recipes 2/3, …, t=4728
  Fluid Laser Base at [240, 101, 0]: tank empty 0/32000 mB, …, depth 101, …, recipe industrialforegoing:laser_drill_fluid/ether, recipes 2/3, …, t=4734
  depth 300: no recipe; depth 101: the ether recipe
  Laser Drill at [240, 102, 0]: 8000/10000 FE, progress 43/50, target [240, 101, 0], …, t=4750
  Fluid Laser Base at [240, 101, 0]: tank industrialforegoing:ether_gas 10/32000 mB, capability industrialforegoing:ether_gas 10 mB, progress 0/20, depth 101, …, recipe industrialforegoing:laser_drill_fluid/ether, recipes 2/3, lens industrialforegoing:purple_laser_lens, …, t=4755
  industrialforegoing:laser_drill_fluid/ether: 10 mB, capability 10 mB, 8000 FE
== BK: a second bar adds ten more mB to the same tank ==
  tank 20/32000 mB of industrialforegoing:ether_gas      (drill 7000/10000 FE)
== BL: the tank drains through the declared capability ==
  drained 10 mB; tank and capability both read 10 mB
== BM: the chamber runs the one carried recipe that names ether gas ==
  Dissolution Chamber at [220, 101, 0]: 3550/10000 FE, input industrialforegoing:ether_gas 135/8000 mB, …, recipe industrialforegoing:dissolution_chamber/supreme_machine_frame, items 8, recipes 23/23, capability industrialforegoing:ether_gas 135 mB, …, t=4850
  … input empty 0/8000 mB, progress 0/0, recipe none, items 0, …, t=4866
  135 mB of ether gas and eight items -> industrialforegoing:machine_frame_simple x1, machine_frame_advanced x1, machine_frame_supreme x1
EXIT:0
```

**Fail-capable, three ways, all `EXIT:1`:** the control leg (the same Nightly, no package) stops at
its first `give` (`FAIL: the machine item did not register`); `SELFTEST=1` flips phase B
(`FAIL: the water is 200, expected 201`); and **`SELFTEST=fluid`** — this slice's own — flips BJ's
ether count (`FAIL: the ether one bar yields is 11, expected 10`) after the whole scenario has run.
BI is the in-run counterfactual (the same machine and lens with no wither), and BJ's depth 300 read
is the second (the same box and wither, only `miningDepth` changed). All four logs end with their
own exit status; `check-e2e-logs.py industrialforegoing-e2e --strict` reads **9/9**.

### Coverage (families this slice adds)

| family | coverage |
|---|---|
| `fluid_laser_base` (block + item) | `tested` — BI places it from its item under a drill and reads it back; the item carries the tank on a break (`a_broken_base_carries_its_tank_and_not_its_lens`, clamped) |
| `industrialforegoing:laser_drill_fluid` (recipe type + serializer) | `tested` — BI reads `recipes 2/3` (three files kept, two decodable) and BJ resolves `…/ether`; the file-count side is asserted |
| the **ether** recipe (10 mB, purple lens, entity gate) | `tested` — BI (no wither, none) → BJ (wither, 10 mB) |
| the **lava** recipe (100 mB, orange lens, Nether 5–20) | `carried, not exercised in game` — its dimension is the Nether and its band 5–20; the rules test asserts both (`the_rarity_band_and_dimension_are_the_jars`). A Nether leg is the next slice's first option (deviation 37) |
| the **oil** recipe (50 mB, black lens, ocean/desert 20–60) | `carried as data, not in the table` — `pneumaticcraft:oil` is another mod's fluid key; the game keeps the file, the decode leaves it out (`recipes 2/3`) |
| the depth band | `tested` — BJ: depth 300 → `none`, depth 101 → ether, the wither present in both |
| the biome filter | `ported, not exercised for this type` — the ether file names no biome; the shared `laser::Filter` is exercised by the ore base's BA/BB/BE |
| the entity gate (`entity_data`, health > 5, 5 damage) | `tested` — BJ/BK (a NoAI wither keeps the recipe valid over two bars); the health threshold and the damage are unit-tested |
| the 32,000 mB drain-only tank / declared capability | `tested` — BJ reads `10/32000 mB` and `capability … 10 mB`; BL drains 10 out through the capability; SC/`/fill` refusal is the pin's rule (deviation 38) |
| the one lens slot / no FE of its own | `tested` — BI writes the lens through `/item replace block … container.0`; BD/BJ show the drill pays and the base's `info` has no FE field |
| the shared drill (one drill, two base kinds) | `tested` — BI's drill targets `[240, 101, 0]` (a fluid base) where slice 7's phases targeted an ore base |
| the fill path (`/industrialforegoing fluid`) | `tested` — BM writes the chamber's 135 mB of `:ether_gas`; the fluid base's own tank is filled by the machine, not by a command |
| the augments on the fluid base | `ported, not exercised in game` — it seats through the shared `use_item_on`; `Base::advance` scales with efficiency (unit test), no phase seats one |
| a GUI screen, redstone modes, comparator, config | `not ported` — gaps 1, 2, 7, 8 |

### Deviations and gaps this slice adds

37. **The fluid type's place filter is proven for depth, not for dimension or biome.** The only
    standalone recipe of the type is ether, whose band is the whole world with an empty biome
    whitelist; lava names the Nether (5–20) and oil names ocean/desert (20–60) but oil's output is a
    foreign fluid key. So the in-game differential is `depth 300` → none against `depth 101` → ether,
    and the Nether/biome arms are unit-tested (`the_rarity_band_and_dimension_are_the_jars`) and, for
    the shared `laser::Filter`, exercised in game by the ore base (BA/BB/BE). A Nether leg that runs
    lava (orange lens, y 5–20) is the honest way to close it.
38. **The tank is filled by the machine only.** As with every tank at this pin, `native_fluid_insert`
    takes water and lava only, so a mod fluid cannot be inserted; the machine's own `fillForced` is
    the fill path and the tank is `Action.DRAIN` on every face, as upstream. `/industrialforegoing
    fluid` writes a *chamber* or *unit* tank for the e2e, never this one — BL proves the drain side
    through the declared capability.
39. **`oil` stays in the package as data.** Its file is the jar's own, the game loads it under this
    type (`3` files kept), and the decode leaves it out because this package declares no
    `pneumaticcraft:oil` key — a pack that ships PneumaticCraft gets its file decoded, since the
    decode asks the game's own fluid registry. The readout counts it on the `files` side.
40. **The `entity_data` gate damages the entity, and `NoAI` keeps it alive.** The match takes 5
    health per operation; the e2e's wither is `NoAI`/`Silent`/`PersistenceRequired` so it stays in
    the box for two bars. `data` is compared as a JSON subset of the entity's saved compound
    (numbers by value), which is what the jar's `entityData.data` does with an empty object.
41. **No weighted pick.** The jar's *ore* base weights candidates; the fluid base takes the first
    file-order match. The port implements the fluid rule and does not reuse the ore base's weights
    (`the_first_recipe_whose_catalyst_rarity_and_gate_pass_is_the_one_used`).

### The changelog for the 0.2.8 update

> Supersedes the closed 0.2.7 PR of this entry. **New: the Fluid Laser Base and its own recipe type**
> `industrialforegoing:laser_drill_fluid` (declared with its read-only serializer). It is the tank
> machine the same laser drill charges: no FE of its own, a 20-tick bar advanced only by the drill
> (which now targets either laser base), one lens slot, and a 32,000 mB drain-only tank reported
> through the declared fluid capability. At the top of the bar the first file-order recipe whose
> catalyst is in the lens slot, whose first valid rarity answers the biome tags, dimension type and
> mining depth, and whose optional nearby-entity gate passes fills the tank. The jar's three files
> ship — lava (the Nether, depth 5–20), a pack's oil (ocean/desert, 20–60; kept as data because its
> fluid key belongs to another mod) and this mod's **ether gas** (every biome and depth, gated on a
> nearby wither) — which gives `industrialforegoing:ether_gas` a source. Proven in game: with the
> drill charged above it and no wither the base answers no recipe and leaves the tank empty while
> the drill still pays 1,000 FE; with a wither in the box it resolves the ether recipe at depth 101
> and not at 300, one completed bar puts 10 mB into the tank the declared capability reports, a
> second bar adds ten more, the tank drains through that capability, and the dissolution chamber
> runs its one carried recipe naming ether gas (the supreme machine frame, 135 mB) to the product.
> `src/runtime.rs` is unchanged, byte for byte; `src/fluidlaser.rs` is a fifth pure module. Only
> `:pink_slime` is now a declared fluid with no source.

## Slice 9 — the mob slaughter factory and the pink slime set (0.2.9, 2026-11-23)

**The slice: the port's own deviation 19** — *"`:pink_slime` is the only declared fluid with no
source — its machine is a later slice"* — together with **deviation 21** — *"The output tank is inert
with the carried recipes. No recipe this package carries has an `outputFluid` (the jar's only one is
`pink_slime_ball`, whose input item this slice does not ship)"*. The machine is the **Mob Slaughter
Factory**: it kills a mob in the block behind it for 400 FE and fills a meat tank and a **pink slime
tank** from its health, so `:pink_slime` gets its source; shipping the `pink_slime` item makes the
jar's only `outputFluid` recipe carryable, which is the one carried exercise of the chamber's output
tank. Tier **machines** (ledger): one block entity, two fluid tanks, one FE buffer, a server tick and
an entity scan — no renderer, no dimension work, no new recipe type.

### Upstream (jar `industrial-foregoing-1.21-3.6.39.jar`; source `Buuz135/Industrial-Foregoing`
### `ce2852db`, clone deleted after the findings)

| upstream | behaviour | what the port carries |
|---|---|---|
| `SlaughterFactoryBlock` | `Properties.ofFullCopy(IRON_BLOCK)`, `RotationType.FOUR_WAY`, `TAB_AG_HUS` | block + item, `subfacing` (four values), the iron-block template, a new `ag_hus` item group, the pickaxe entry, a 26.3 loot table |
| `SlaughterFactoryTile extends IndustrialAreaWorkingTile(BEHIND, acceptsRangeUpgrades = true)` | a bar **counting down** from `maxProgress` 100; at zero `work()`: with ≥ 400 FE, the first alive, not permanently invulnerable **Mob** in `RangeManager(BEHIND).getBox().expandTowards(0, 2, 0)` is `remove(KILLED)`; the tanks gain `animal ? (int)health : (int)health*20` (meat) and `animal ? (int)(health*20) : (int)health` (pink slime); 400 FE is spent; `WorkAction(0.2f, 400)` → the bar rests at `floor(bar * 0.2)` | `src/slaughter.rs` (the rules) and `src/mobslaughter.rs` (the tick, the scan, the kill, the two tanks) |
| two `SidedFluidTankComponent`s (`"meat"`, `"pink_slime"`, 8,000 mB each, `Action.DRAIN`, one validator per fluid) | every face reads/drains, none fills; the machine fills them itself | `Registrar::block_fluids` with two drain-only tanks; the contents in the block entity's own `fluids` compound, meat first |
| `EnergyStorageComponent(maxStoredPower = 10,000, 10, 20)` | unlimited IO up to capacity | `ModEnergyBuffer`, the amount the compound's `energy` int |
| `ModuleCore.PINK_SLIME_ITEM`/`PINK_SLIME_INGOT`, `PINK_SLIME_BLOCK` (`new PinkSlimeBlock(… friction(0.8F).sound(SLIME_BLOCK).noOcclusion())`, a `SlimeBlock`) | two plain items and a bouncy block | the two items, the block on the `minecraft:slime_block` template with `friction 0.8`, their textures/models/definitions, the blockstate, the loot table and the jar's `c:ingots/pink_slime` / `c:block/storage_blocks/pink_slime` |
| `pink_slime_block` (3×3) and `pink_slime` (shapeless, 9) | the block packs and unpacks | both recipes, re-spelled for 26.3 |
| `dissolution_chamber/pink_slime_ball` | **300 mB pink slime + a glass pane → 1 pink slime + 150 mB `minecraft:water`**, 200 ticks | the recipe (`outputFluid`), the first carried one |
| `dissolution_chamber/pink_slime_ingot` | 1,000 mB pink slime + 2 iron + 2 gold → 1 ingot, 300 ticks | the recipe |

`SlaughterFactoryConfig`: `maxProgress` 100, `powerPerOperation` **400**, `maxStoredPower` 10,000,
`maxMeatTankSize` 8,000, `maxPinkSlimeTankSize` 8,000 — the values the game uses (the jar's
`FluidExtractorConfig` comments lied once; these are read from the code).

### The brief's assertion, corrected from the artefact (again)

The brief says the no-mob counterfactual leaves *"`tank empty`, FE still spent"*. The artefact says
otherwise: `work()` answers `WorkAction(1f, 0)` when there is no mob — `onFinishWork` then extracts
**0** FE — so the port asserts **FE unchanged** (phase CB). The in-game run reads it: 5,000 FE in,
5,000 FE out, tanks empty, the bar back near full. Recorded here instead of changing the mechanic.

### TDD, in order

1. **Red** (`src/slaughter.rs`'s eight rules and `src/mobslaughter.rs`'s nine glue tests with
   `yield_for`, `Factory::tick`, `Factory::rest`, `tanks`, `behind`, `animal_category`, `read_tanks`,
   `write_tanks`, `carried_patch` and `clamp_patch` at `todo!()`; `sdk-test-red-slice9.log`):
   **149 passed, 17 failed**, every failure `panicked at src/… not yet implemented` — e.g.
   `slaughter::tests::a_cow_gives_ten_meat_and_eighty_slime`,
   `slaughter::tests::a_villager_gives_twenty_slime_and_four_hundred_meat`,
   `mobslaughter::tests::the_factory_declares_two_eight_thousand_mb_tanks`.
2. **The rules** (`src/slaughter.rs`, a **sixth** pure module — `runtime.rs` stays byte-identical,
   which `verify-sdk-package.py` checks): `Victim`, `Yields`, `Factory` (the bar down, the 400 FE, the
   two tanks with their clamps, the rest at `floor(bar * 0.2)`, the speed augment one tick late and
   the efficiency bar) and `yields_for` — the jar's **two different casts**, both pinned with a
   fractional health: an animal's slime is `(int)(health * 20)` while a non-animal's meat is
   `(int)health * 20`, so 9.7 gives 194 and 19.7 gives 380.
3. **The glue** (`src/mobslaughter.rs`): the declarations (block entity type, buffer, two tanks, the
   Part, the item Part), the entity scan (`LevelRef::entities_in` + `Host::mob` for vanilla's `Mob`,
   `LivingRef::alive`/`health`, `EntityRef::permanently_invulnerable`), the kill
   (`EntityRef::remove(KILLED)` — vanilla's removal, no loot and no experience, as upstream), the two
   tanks in the saved compound, the item carry/clamp, the readout and the setters.
4. **Content** (39 files) with `tools/assemble_slaughter.py` (`--check`), and `verify_content.py`'s
   new family plus five selftest mutations.

Green: **166 passed, 0 failed** (`Compiling industrialforegoing v0.2.9`).

### In game (phases BN and CA–CI; `evidence/industrialforegoing-e2e/e2e-launcher.log`)

| phase | what it asserts | the value it read |
|---|---|---|
| BN | the chamber's **`outputFluid`** recipe: 300 mB of pink slime and a glass pane, 200 ticks | `recipe …/pink_slime_ball` → output slot `pink_slime x1`, **`output minecraft:water 150/16000 mB`**, the 300 mB consumed, the pane gone, `recipes 25/25` |
| CA | the factory places, faces its placer, and is empty | `facing west`, both tanks `empty 0/8000`, `0 FE`, `victims 0`, bar 97/100 |
| CB | **counterfactual: no mob** | 5,000 FE in, **5,000 FE out**, tanks empty, bar 94/100 (rested full) |
| CC | a NoAI **cow** in the box | `victims 1` → `meat 10/8000`, `pink slime 200/8000`, both capabilities report them, `4600 FE`, bar 13/100, `victims 0` |
| CD | **counterfactual: a mob but 0 FE** | `victims 1` before and after, tanks unchanged, 0 FE, bar 94/100 |
| CE | 5,000 FE then kills the **villager** (not an animal) | `meat 410/8000`, `pink slime 220/8000`, 4,600 FE; `/drain 100` → `drained 100 mB of industrialforegoing:meat and 100 mB of industrialforegoing:pink_slime`, tanks 310/120 |
| CG | the pink slime block | places and reads back; broken in survival it drops itself (`drops{pink_slime_block: 1}`) |
| CI | a broken factory's item | broken at 310 mB meat / 120 mB pink slime / 4,600 FE, placed back at exactly those |

The transcript's own lines:

```
== CC: a cow in the box is killed for 10 meat and 200 mB of pink slime ==
  Mob Slaughter Factory at [250, 101, 0]: 5000/10000 FE, tank empty 0/8000 mB, capability empty 0 mB, tank empty 0/8000 mB, capability empty 0 mB, progress 89/100, victims 1, facing west, augments speed 1 efficiency 1.00, t=5648
  Mob Slaughter Factory at [250, 101, 0]: 4600/10000 FE, tank industrialforegoing:meat 10/8000 mB, capability industrialforegoing:meat 10 mB, tank industrialforegoing:pink_slime 200/8000 mB, capability industrialforegoing:pink_slime 200 mB, progress 13/100, victims 0, facing west, augments speed 1 efficiency 1.00, t=5661
== CE: a villager is killed for 400 meat and 20 mB of pink slime, and both tanks drain ==
  Mob Slaughter Factory at [250, 101, 0]: 4600/10000 FE, tank industrialforegoing:meat 410/8000 mB, capability industrialforegoing:meat 410 mB, tank industrialforegoing:pink_slime 220/8000 mB, capability industrialforegoing:pink_slime 220 mB, progress 14/100, victims 0, facing west, augments speed 1 efficiency 1.00, t=5706
  Mob Slaughter Factory at [250, 101, 0]: drained 100 mB of industrialforegoing:meat and 100 mB of industrialforegoing:pink_slime.
== BN: the chamber's only outputFluid recipe — 300 mB of pink slime and a pane give 150 mB of water ==
  Dissolution Chamber at [220, 101, 0]: 1090/10000 FE, input empty 0/8000 mB, output minecraft:water 150/16000 mB, progress 0/0, recipe none, items 0, recipes 25/25, capability empty 0 mB, augments speed 1 efficiency 1.00, facing west, t=5531
  300 mB of pink slime and a pane -> industrialforegoing:pink_slime x1, output tank minecraft:water 150 mB
```

**Fail-capable, four ways, all `EXIT:1`:** the control leg (the same Nightly, no package) stops at
its first assertion (`FAIL: the machine item did not register`); `SELFTEST=1` flips phase B
(`FAIL: the water is 200, expected 201`); **`SELFTEST=slaughter`** — this slice's own — flips CC's
pink slime count (`FAIL: the pink slime a cow gives ((int)(health * 20)) is 201, expected 200`) after
the whole scenario has run; and CB/CD are in-run counterfactuals (no mob; a mob with no FE).
`check-e2e-logs.py industrialforegoing-e2e --strict` reads **10/10**.

### The chamber that stopped ticking (a measured platform quirk, not a port defect)

The first three runs of BN failed with the same read: `progress 0/200`, `9910/10000` FE unchanged
after 219 ticks — the chamber placed in phase AE and left after BM was **not advanced by its own
tick** any more, while its readout, its tank writes and its recipe match all still worked. Isolated
with probes against the same package in clean worlds (`evidence/industrialforegoing/probe-chamber-outputfluid.log`):
a `setblock` chamber, a chamber placed from its item, a chamber carrying `block_entity_data`, one
that has completed a recipe, one whose slots were cleared with `air` writes, one taken two chunks
away and back — **all of them run the recipe**, twice in a row, filling the output tank to 150 then
300 mB of water. So the port's path is sound and the stuck block entity is a property of that long
run's world, not reproducible from a clean one. BN therefore places its own chamber (the phase is
about the recipe, not about that block entity), and the open question — *why a mod block entity that
has been broken, re-placed and run once stops being ticked after forty phases of harness commands* —
is recorded for the next slice rather than guessed at.

While chasing it, the chamber's tick learned one thing worth keeping: a block entity whose slot view
holds fewer entries than the container declares is now **padded** (`items.resize(SLOTS, None)`)
instead of skipped, so a short view can never freeze a machine silently.

### Upstream defects fixed

1. **The jar's `minecraft:mineable/pickaxe` names ten fluid ids as if they were blocks**
   (`industrialforegoing:latex`, `:meat`, `:sewage`, `:essence`, `:sludge`, `:pink_slime`,
   `:biofuel`, `:ether_gas`, `:raw_ore_meat`, `:fermented_ore_meat`). A block tag that names a
   fluid can never resolve; the port's tag carries only the block ids it ships (this slice adds the
   factory and the pink slime block to the machines already there). The game reads the tag without
   dangling entries.

### Deviations and gaps this slice adds

42. **`entity instanceof Animal` has no pin expression.** The port reads the type's `MobCategory`
    and treats `CREATURE` and `AXOLOTLS` as vanilla's animals. The one vanilla mob that is a
    `CREATURE` but not an `Animal` is the **wandering trader** (an `AbstractVillager`), which the
    machine therefore treats as a non-animal: it would give 20 × its health in meat and its health
    in pink slime. Every common animal (cow, pig, sheep, chicken, horse, wolf, cat, turtle, frog,
    strider, bee, axolotl) is classified correctly; phase CC proves the animal arm and CE the
    non-animal one.
43. **The pink slime block does not bounce.** Upstream's `PinkSlimeBlock extends SlimeBlock`; a
    content block's declaration carries the template's *properties* (friction 0.8, the slime sound
    group, no occlusion, hardness 0) but not a Java class's `bounceUp`, and the pin's `BounceOn`/
    `IsSlimeBlock` hooks are a block Part this slice does not add. A user notices a slime block that
    looks and mines like the jar's and does not launch them.
44. **The jar's `c:fluid/pink_slime` tag is not carried.** The port declares the fluid key and its
    tanks read it, but 26.3's tag loader is the item/block one for mod content, and no carried
    recipe or add-on reads a fluid tag; a pack that asks for `#c:pink_slime` fluids would not find
    it. Recorded rather than carried blind.
45. **The pink slime ball's `outputFluid` names `minecraft:water`.** `native_fluid_insert` still
    takes water and lava only, so the *output* tank is the one tank a vanilla fluid legitimately
    reaches; the input tank is still written by `/industrialforegoing fluid`, since a mod fluid has
    no insert path (deviation 38). BN proves the fill side through the machine's own completion.
46. **The factory's `RangeType.BEHIND` working area has no range addon.** Upstream scales it with
    `RangeAddonItem.RANGE`; this package does not ship the range addons (they are the
    `industrialforegoing:range_addon_*` items of another family), so the area is the base box — the
    block behind the machine, two blocks up — exactly as with an empty range slot upstream.
47. **The two new loot tables carry the package's `bonus_rolls`/`rolls` spelling.**
    `ports/tools/check_261_dialect.py` flags `"bonus_rolls": 0.0` and `"rolls": 1.0` on the factory's
    and the pink slime block's tables exactly as it flags the ten carried before this slice;
    HANDOVER §9d.13 records those two fields as **unverified** ("no game has shown them to change
    behaviour; a flag is not a defect"), and phases CG and CI read both drops, so the slice keeps
    the package's shape rather than mixing two dialects.
48. **A broken factory carries its two tanks and its buffer, not its slots.** The engine drops a mod
    container's slots itself; the factory has no container, so its whole saved compound (energy,
    progress, `fluids`, augments) travels on the item and is clamped on placement (phase CI).

### Coverage (families this slice adds)

| family | coverage |
|---|---|
| `mob_slaughter_factory` (block + item) | `tested` — CA places it from its item and reads it back, `facing west`; CI breaks it and reads `drops{mob_slaughter_factory: 1}` |
| its 100-tick bar, 400 FE an operation, the 20-tick rest | `tested` — CC/CE read the bar at 13/14 of 100 after a kill, CB/CD at 94 after an idle attempt; the arithmetic is `slaughter::Factory::rest` |
| the working area (behind, two up) | `tested` — CC (`victims 1` for a mob behind a west-facing machine) and CD (none in front); `the_working_area_is_the_block_behind_the_machine` pins the four offsets |
| the Mob filter (vanilla `Mob`, alive, not permanently invulnerable) | `tested` — CC kills a cow, CD refuses one with no FE; the filter is `Host::mob` + `LivingRef::alive` + `permanently_invulnerable` |
| the kill (no loot, no experience) | `tested` — CC's cow leaves `victims 0` and no items; `EntityRef::remove(KILLED)` is vanilla's removal |
| the animal / non-animal yields | `tested` — CC (cow: 10 meat, 200 pink slime) and CE (villager: 400 meat, 20 pink slime), with the two casts unit-tested at fractional health |
| the meat tank and the pink slime tank (8,000 mB each, drain-only, capabilities) | `tested` — the readouts print each tank *and* the capability's answer; CE drains 100 mB out of both |
| the 8,000 mB clamps | `unit-tested` (`the_tanks_clamp_at_eight_thousand_mb_each`) |
| the factory's shaped recipe | `tested` — `tools/craft_recipe.py` crafts `PDP/SMS/ARA` from `#c:plastics`, `#c:gears/gold`, `machine_frame/pity`, two iron swords and two axes, asserts the result and that every ingredient is consumed |
| `pink_slime`, `pink_slime_ingot`, `pink_slime_block` | `tested` — the craft legs make the block from nine pink slime and give nine back; CG places and breaks the block; all three are also `give`-read back by the Core phase |
| `pink_slime_block`'s loot | `tested` — CG: `drops{pink_slime_block: 1}` |
| `c:ingots/pink_slime`, `c:block/storage_blocks/pink_slime` | `asserted as data` (`verify_content.py` reads their values; no carried recipe names them) |
| the chamber's `outputFluid` path | `tested` — BN: 150 mB of `minecraft:water` in the output tank after one 200-tick `pink_slime_ball` |
| `dissolution_chamber/pink_slime_ingot` | `carried, crafted by type` — its 1,000 mB and four ingots are asserted by `verify_content.py`; the chamber's rate is proven by BN and by slices 5–8 |
| the augments on the factory | `ported, not exercised in game` — it seats through the shared `use_item_on`; the speed/efficiency arithmetic is unit-tested (`a_speed_augment_falls_the_bar_faster`, `an_efficiency_augment_shortens_the_bar_and_the_rest`) |
| a GUI screen, redstone modes, comparator, config | `not ported` — gaps 1, 2, 7, 8 |
| the range addons, the mob crusher, the animal machines | `not ported` — gap 9's remaining families |

### The changelog for the 0.2.9 update

> Supersedes the closed 0.2.8 PR of this entry. **New: the Mob Slaughter Factory and the pink slime
> set.** The factory is the machine the port's own gap list said `:pink_slime` had no source for: a
> 100-tick bar counting down, a 10,000 FE buffer at 400 FE an operation, and a working area of the
> block behind it expanded two blocks up, where the first Mob — alive, not permanently invulnerable
> — is removed for its health: an animal gives its health in meat and twenty times that in pink
> slime, anything else the other way round, with the jar's two different integer casts reproduced
> exactly. Both tanks are 8,000 mB, drain-only, reported by the declared fluid capability, and both
> drain in game. `industrialforegoing:meat` is declared beside the other fluid keys. The pink slime
> item, its ingot and the pink slime block ship with their textures, models, recipes (nine slime to
> a block and back), their `c:ingots/pink_slime` and `c:block/storage_blocks/pink_slime` tags and
> the `ag_hus` item group, and the dissolution chamber's 25-recipe table now holds the jar's **only
> `outputFluid` recipe** — 300 mB of pink slime and a glass pane for one pink slime ball and 150 mB
> of water. Proven in game: the factory placed facing its placer is empty; a bar with no mob kills
> nothing and spends no FE (the jar's `WorkAction(1f, 0)`; the brief's "FE still spent" was wrong);
> a cow gives 10 mB of meat and 200 mB of pink slime for exactly 400 FE; the same box with no FE
> leaves a villager alive; 5,000 FE then kills it for 400 mB of meat and 20 mB of pink slime; both
> tanks drain through the capability; the factory's own shaped recipe crafts from the package's tags
> with every ingredient consumed; the pink slime block crafts from nine pink slime, gives nine back
> shapeless and drops itself; a broken factory's item carries both tanks and its buffer back; and the
> chamber's pink slime ball puts 150 mB of `minecraft:water` in its output tank. `src/runtime.rs` is
> unchanged, byte for byte; `src/slaughter.rs` is a sixth pure module. The chamber's tick now pads a
> short slot view instead of skipping the machine. The remaining families (the mob crusher, the
> animal machines, the generators, the conveyors, the GUI) are the next slices.

## Skills

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices,rust-review (slice 9: `brainstorming` to choose the factory from deviations 19 and 21 and weigh the brief's "FE still spent" against `SlaughterFactoryTile.work()`, `writing-plans` for SLICE-9-PLAN.md, `test-driven-development` for the eight red rules and nine red glue tests (`149 passed, 17 failed`, every one `not yet implemented`), `systematic-debugging` for the chamber that stopped advancing its bar (seven probes in clean worlds, all green; the quirk and the padding fix are recorded in the slice 9 section) and for the two craft-script defects the game found (a stack of two `stacksTo(1)` items leaves the second cell empty; a chamber that has run a recipe is not ticked any more in that world), and `verification-before-completion` before recording; `rust-best-practices` for the sixth pure module and its glue, `rust-review` for the after-the-fact audit of `src/mobslaughter.rs`; slice 8: `brainstorming` to choose the fluid laser base from the port's own coverage row and weigh the brief's "a biome the ether recipe names" against the jar's empty whitelist, `writing-plans` for SLICE-8-PLAN.md, `test-driven-development` for the eight red rules and the five red glue tests, `systematic-debugging` for the two in-game gates this slice cost — the launcher's content-hash cache reporting "up to date" and the depth-band differential that replaced a biome counterfactual the artefact does not have — and `verification-before-completion` before recording; `rust-best-practices` for the fifth pure module and the glue, `rust-review` for the after-the-fact audit of `src/fluidbase.rs`; slice 7: `brainstorming` to choose the laser drill from the gap list and weigh the brief's "the ore the base sits on" against the jar's biome/depth rule, `writing-plans` for SLICE-7-PLAN.md, `test-driven-development` for the 14 red rules and the 9 red glue tests, `systematic-debugging` for the six in-game gates this slice cost — the ingredient-object decode (`recipes 0/10`), the biome zoom, the datapack/tag reload, the placement aim, the message/log window, and the loot condition shape — and `verification-before-completion` before recording; `rust-best-practices` for the new pure module and the glue, `rust-review` for the after-the-fact audit of `src/laserdrill.rs`)

`cargo fmt --package industrialforegoing --check`, `cargo clippy --all-targets -- -D warnings` and
`cargo test` were run before hand-back (slice 9: `FMT-CLEAN`, 0 warnings, **166 passed**).
`#[allow(private_interfaces, reason = "…")]` marks the injected module's own private
`Game`/`ModBlockEntity` parameters (the same exception `botanypots`, `powah`, `charginggadgets` and
`mininggadgets` record).

## Verification

**[`VALIDATION.md`](VALIDATION.md) is the ladder for the delivered 0.2.8 package** at the
campaign pin `0a875da…` (the red test run, fmt, clippy, 148 tests, two `cmp`-identical builds,
`check`, both `fits` runs, the content verifier with its selftest, and the in-game legs — green,
control and five selftests — with their exit statuses). The tables below are the earlier ladders,
kept as history.

### The SDK ladder (2026-11-18, the 0.2.0 migration; historical)

Pin **`0b14f8acfd97fa2c750e42795249dcc981178ca4`**, mixins
**`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`**, launcher 0.2.78, Rust 1.97.0 (the SDK
declares 1.99.1), every step inside a `ports/tools/build-slot.sh` lease with
`CARGO_INCREMENTAL=0`. Evidence: `ports/_shared/evidence/industrialforegoing/`.

| Step | Command | Result | Evidence |
|---|---|---|---|
| New glue, red | `cargo test` with the three new helpers `todo!()` | **18 passed, 4 failed** (the four new glue tests) | `sdk-test-red.log` |
| Tests, green | `cargo test` | **22 passed**, 0 failed (18 carried runtime + 4 glue) | `sdk-test.log` |
| Formatting | `cargo fmt --check` | clean | `sdk-fmt.log` |
| Lints | `cargo clippy --all-targets -- -D warnings` | exit 0, 0 warnings | `sdk-clippy.log` |
| Package | `rm -f Cargo.lock && cargo horizon-mod build` | `target/horizon-mod/industrialforegoing.hmod`, exit 0 | `sdk-build.log` |
| Registry check | `cargo horizon-mod check industrialforegoing.hmod` | `industrialforegoing 0.2.0 (native-mod, format native-1/0b14f8a/e59da90, for 26.3)`, **28,979 B**, sha256 `c10182f3…52ed7`, **exit 0** | `sdk-check.log` |
| What it uses | `cargo horizon-mod check industrialforegoing.hmod --uses` (+ `uses.json` from the zip) | 272 surface entries, exit 0 | `sdk-uses.txt` |
| Fits the pinned build | `cargo horizon-mod fits industrialforegoing.hmod --build nightly` | `0b14f8a  made for`, exit 0 | `sdk-fits.log` |
| `runtime.rs` untouched | `cmp <(unzip -p industrialforegoing.hmod src/runtime.rs) src/runtime.rs` | identical, `7b377432…41b4d` | `sdk-sha256.txt` |
| Content untouched | `diff -r` of `assets`+`data` against the retired package's | no difference | `sdk-sha256.txt` |
| Skills line | `python3 ports/tools/check-skills.py --notes ports/industrialforegoing` | SKILLS GATE: PASS | `sdk-skills.log` |
| e2e | launcher-built game + `tools/e2e.sh` | **`PASS` + `EXIT:0`** in 0.2.1, control `EXIT:1`, selftest `EXIT:1` (`evidence/industrialforegoing-e2e/`) | `e2e-launcher*.log` |

### The retired `.hnmod` ladder (2026-11-09, kept as history)

Pin **`1d63e0a5a37a7a708eb0b32b49bf0dfeb28ed8a9`**, mixins
**`5a64af938ec86b00d53d16a408aabc0038fb3e57`** (schema 6); checkouts
`ports/_shared/pins/horizon-1d63e0a` and `.../horizon-mixins-5a64af9`. Every
build/test/e2e ran through `ports/tools/build-slot.sh` with
`CARGO_INCREMENTAL=0`; no bare cargo, no `--target-dir`.

| Step | Evidence | Result |
|---|---|---|
| runtime TDD (red) | `test-red.log` | **1 passed, 17 failed** — every body `todo!()` (the one passing test only reads struct fields) |
| runtime tests (green) | `test-green.log` | **18 passed**, 0 failed |
| fmt | `fmt.log` | `cargo fmt --all -- --check` exit 0 |
| clippy | `clippy.log` | `--all-targets --locked --offline -- -D warnings`, exit 0, `Finished` |
| stage | `stage.log` | `stage_changed: true`, 2 modules, 9 injections (`horizon-mixins plan` output: 9) |
| check | `horizon-check.log` | `cargo_exit: 0`, **0 warnings** |
| embedded test | `horizon-test-cargo.log` | `game::industrialforegoing::tests`: **18 passed**; 0 failed; 1240 filtered out |
| fast build | `horizon-build.log` | `cargo_exit: 0`, 0 warnings |
| e2e (content as a dev mod) | `e2e.log` | **PASS**, `EXIT:0` |
| e2e (one file only: `EMBEDDED=1`, no `--dev-mod`) | `e2e-embedded.log` | **PASS**, `EXIT:0`; the control leg (same binary, `.horizon-cache` unusable) shows the ids unknown |
| fail-capability | `e2e-selftest.log`, `e2e-embedded-selftest.log` | `SELFTEST=1` → **FAIL** + `EXIT:1` (`the water is 200, expected 201`) |
| log checker | `check-e2e-logs.py industrialforegoing --strict` | `logs=4 ok=4 missing-exit=0 bad-exit=0` |
| `.hmod` build/check | `package.log` | 7,890 B, `8f8e16c6…c26ff`, 18 entries, “no code: content only” |
| `.hnmod` (content embedded) | `package.log` | 18,058 B, `f28c8c42…414b`, 14 entries incl. `mixins/industrialforegoing/data/industrialforegoing-content.hmod` |
| package gate | `upstream_ready.py --include-blocked` | **READY** (plain run is BLOCKED only by the ledger row still reading `pending`; the orchestrator records it) |



### In-game transcript (quoted from `e2e.log`)

```
== A: a machine placed from its item is empty, with no sources ==
  Water Condensator at [202, 101, 0]: 0/10000 FE, water 0/16000 mB, sources 0, progress 89/100, facing west, t=147
  block industrialforegoing:water_condensator, 0/10000 FE, water 0/16000 mB, facing west
== B: two sources and 5,000 FE: 200 mB for 1,000 FE, cooldown 10 ==
  Water Condensator at [202, 101, 0]: 4000/10000 FE, water 200/16000 mB, sources 2, progress 7/100, facing west, t=190
== C: one source condenses nothing (the rule is two or more) ==
  Water Condensator at [202, 101, 0]: 5000/10000 FE, water 0/16000 mB, sources 1, progress 97/100, facing west, t=231
== D: no sources condense nothing ==
  Water Condensator at [202, 101, 0]: 5000/10000 FE, water 0/16000 mB, sources 0, progress 97/100, facing west, t=274
== E: without 1,000 FE two sources give half, for free, cooldown 50 ==
  Water Condensator at [202, 101, 0]: 999/10000 FE, water 100/16000 mB, sources 2, progress 47/100, facing west, t=316
== F: three sources condense 300 mB ==
  Water Condensator at [202, 101, 0]: 4000/10000 FE, water 300/16000 mB, sources 3, progress 7/100, facing west, t=358
== G: six sources condense 600 mB ==
  Water Condensator at [202, 101, 0]: 4000/10000 FE, water 600/16000 mB, sources 6, progress 7/100, facing west, t=399
== H: a nearly full tank stops at 16,000 mB and the FE is still spent ==
  Water Condensator at [202, 101, 0]: 4000/10000 FE, water 16000/16000 mB, sources 2, progress 7/100, facing west, t=441
== I: the tank is a real fluid handler: /drain takes water out ==
  Water Condensator at [202, 101, 0]: drained 400 mB of minecraft:water.
  Water Condensator at [202, 101, 0]: 3000/10000 FE, water 600/16000 mB, sources 0, progress 64/100, facing west, t=494
== J: left alone, the machine keeps condensing on its own bar ==
  Water Condensator at [202, 101, 0]: 4000/10000 FE, water 200/16000 mB, sources 2, progress 7/100, facing west, t=535
  Water Condensator at [202, 101, 0]: 0/10000 FE, water 1100/16000 mB, sources 2, progress 40/100, facing west, t=592
PASS: … a placed machine is empty with no sources and faces its placer; two sources condense 200 mB for exactly 1,000 FE and one condenses nothing; without 1,000 FE two sources give half for free; three and six sources give 300 and 600 mB; a nearly full tank stops at 16,000 mB while still spending the FE; the declared tank drains 400 mB of water through the fluid handler; and left alone the machine keeps condensing on its own 10-tick bar
EXIT:0
```

The run is fail-capable six ways: `SELFTEST=1` flips B's 200 mB to 201 and exits 1
(twice, with and without the embedded content); C and D are controls that must
not change a number (one source, no source); E is the free half-rate path; H
proves the tank clamps while the FE is still spent; and the embedded run's
`CONTROL=1` leg launches the same binary with `.horizon-cache` made unusable and
shows both ids unknown.

### Packaged artifacts

The current package (2026-11-21, SDK format) — one file, code and content together:

| File | Size | SHA-256 | Entries |
|---|---:|---|---:|
| `industrialforegoing.hmod` (0.2.9) | 318,727 B | `fa7300d371f22f81576f452d0c1994449d69ebaf0852c89a9491dcdc7479278d` | 409 |
| `industrialforegoing.hmod` (0.2.8) | 286,421 B | `f34f98ee0c57a700595160189162ee80856e962df12218d04f7ab7529fbbbfc9` | 371 |
| `industrialforegoing.hmod` (0.2.7) | 265,740 B | `68b63343357f9f9847bb5bf9be3cd44348aa0f90c98e449afbfc0af8ce5ec614` | 355 |
| `industrialforegoing.hmod` (0.2.6) | 225,305 B | `d9dbb2316b701dab2bf7877bf2361772d3f634113dd75291ac9e6e172b4869a4` | 305 |
| `industrialforegoing.hmod` (0.2.5) | 189,268 B | `0876ea235c4fd196113992cd743c116d513add5e408e723dc2ec7bf34c7b2bad` | 268 |
| `industrialforegoing.hmod` (0.2.4) | 153,549 B | `dd43090d0f7a96b01f82d842cf42cd2f59f45e9d2763c524d65187c320b9508f` | 230 |
| `industrialforegoing.hmod` (0.2.3) | 59,334 B | `819f00fc54dd6a2623406681494a3842f57331207d60e620b2f597ef66559e7c` | 41 |
| `industrialforegoing.hmod` (0.2.2) | 34,096 B | `2672a80085cb0a258239408750b625fbee497f760693e473d823c0b947404826` | 25 |
| `industrialforegoing.hmod` (0.2.1, never published) | 30,694 B | `4cc91337a2d92c023aad9d2bcfd9a83ef1c37c284bf1b33b8bbaeafebeaa1b55` | 25 |
| `industrialforegoing.hmod` (0.2.0, superseded) | 28,979 B | `c10182f32584e6522b274855f6c8331793421237112d87acf804a4fb17752ed7` | 25 |

0.2.4 holds `horizon-mod.toml`, `Cargo.toml`, `src/lib.rs`, `src/runtime.rs`, `uses.json`,
`LICENSE`, `LICENSE-NOTICE.md`, `README.md` and the **222** carried content files: the condensator's
17 and the four addon items' 9 from slices 1–3, plus slice 4's four frames (blockstate, block model,
item model, three textures, item definition, block declaration, loot table = 9 each), the 20 plain
items (texture, model, 26.3 definition, declaration = 4 each), 21 recipes, 21 advancements, the
`c:`/`industrialforegoing:` tags and the `core` creative tab.

The retired pair, kept in `../_shared/evidence/industrialforegoing/retired-hnmod-0.1.0/`:

| File | Size | SHA-256 | Entries |
|---|---:|---|---:|
| `content/industrialforegoing.hmod` | 7,890 B | `8f8e16c658be3baaa376e90b5690fac75405370cf80ebe78c350ad65205c26ff` | 18 |
| `industrialforegoing.hnmod` | 18,058 B | `f28c8c4213a568b698e557b555b64072486d29c29021f904fa48a84070f9414b` | 14 |

The `.hnmod` carried the content mod as
`mixins/industrialforegoing/data/industrialforegoing-content.hmod`
(7,890 B, the same SHA-256), which is how the retired format got one artifact per project.

## Deviations and gaps (slice 1)

1. **No GUI screen and no tooltip.** Upstream draws Titanium's tank, energy and
   progress bars in a 176-wide panel; the pin's screen helpers need a menu, and
   this machine has no slots, so slice 1 exposes the same numbers through
   `/industrialforegoing info` (precedent: `charginggadgets` draws its readouts
   as text; `/powah info`). The progress bar's client animation is absent.
2. **No redstone control modes.** Upstream's `RedstoneManager` offers
   ignore/on/once/with/without; the default action is `IGNORE`, which this port
   reproduces (the machine always runs). The modes are a gap.
3. ~~**No augments.**~~ **Closed for speed and efficiency in 0.2.3 (slice 3).** `titanium:augments`
   is registered and the machine has the parent's two augment slots, seated by right-clicking it;
   the speed augment moves the bar and the efficiency one shortens it, as Titanium's
   `IndustrialWorkingTile` does. Still open: the **processing** augment (a third type; this
   machine runs one operation a cycle, so it has nothing to multiply) and the addons' own
   dissolution-chamber recipes (gap 9's machine). The four parent items ship as `give`-only data.
4. **Titanium is mapped, not ported.** `titanium` is a `library` row; this slice
   reimplements only what the machine needs — the energy-storage arithmetic
   (capacity, unlimited IO), the drain-only tank and the progress bar's
   `workAmount` timing — against the pin's APIs. Everything else the library
   provides (tabs, config files, the container/screen framework, the augment
   system, `@Save` serialisation helpers) is a gap recorded here.
5. ~~**Breaking the machine loses its FE and water.**~~ **Closed in 0.2.2 (slice 2).** The 2026-11-21
   ruling gives a mod block's drop the vanilla route (`block_drops` + `minecraft:block_entity_data`),
   which this port now uses: a broken machine's item carries its FE, water and bar, and placing it
   restores them (clamped). Proven in game in phase L with a before/after differential.
6. ~~**The recipe cannot be crafted in this slice.**~~ **Closed in 0.2.2, and the tags are back in
   0.2.4.** The carried recipe's tag-or-item **list** ingredients were invalid at 26.3 (the loader
   logged `recipes: invalid ingredient …` for `#c:plastics`, `#c:gears/iron` and
   `#industrialforegoing:machine_frame/pity` and dropped the whole recipe), so 0.2.2 used the vanilla
   fallbacks (`minecraft:slime_ball`, `minecraft:iron_block` ×2). Slice 4 ships the items and tags
   those names refer to, so the recipe is the jar's again — `#c:plastics`, `#c:gears/iron`,
   `#industrialforegoing:machine_frame/pity`, each a **single tag** — and phase K crafts it from
   them, with a grid of the old stand-ins staying empty. The `neoforge:item_exists` gate the
   advancement carried (unknown to 26.3, so the file was left out) stays dropped.
7. **No comparator output and no block-entity update tag.** The power level and
   the tank are not exposed to comparators, and nothing is sent to clients
   (there is no client-side view to send it for).
8. **No config file.** The three values are the upstream defaults
   (`maxProgress` 100, `powerPerOperation` 1000, `maxStoredPower` 10000,
   `maxWaterTankSize` 16000); `config/` is not created.
9. ~~**One machine only.**~~ **Partly closed in 0.2.4 (slice 4) and again in 0.2.6 (slice 6).** The `ModuleCore` items and blocks
   ship now — the four machine frames, the iron/gold/diamond gears, plastic and the 16 coloured laser
   lenses — with the tags the pack's add-ons bind to. Still outside the slice: the jar's other
   blockstates and their machines (mycelial generators, the latex/plastic chain and its
   `fluid_extractor`/`latex_processing_unit`, the **dissolution chamber**, conveyors, black-hole units,
   the laser drills, …), the ranges addons, `fertilizer`/`pink_slime`/`straw`, and the patchouli
   book. **Slice 6 took the latex/plastic chain** (`fluid_extractor`, `latex_processing_unit`,
   `dryrubber`, plastic's own smelting recipe); **slice 7 the laser drill and its ore base**, and
   **slice 8 the fluid laser base** with its own fluid recipe type. `industrialforegoingsouls`'s
   blocker is closed (its one `laser_drill_ore` file loads under this package's type and runs);
   `mifa` remains `blocked(industrialforegoing:dissolution_chamber)` for the surfaces the chamber
   does not ship.
10. **The block declaration's `sounds` names a vanilla *block***, not a sound
    group id: `"sounds": "minecraft:iron"` was rejected at load
    (`industrialforegoing:water_condensator: sounds minecraft:iron is not a
    block`) and the whole content mod was dropped with it — the declaration uses
    `minecraft:iron_block`. A load-time content error is silent at the item level
    (the ids are simply unknown), so the e2e's first launch is what caught it.
11. **Not carried:** the `industrialforegoing:water`/fluid registrations (the
    machine uses `minecraft:water`), the patchouli book, the `META-INF`
    services, and any JEI/Jade integration (upstream's is Titanium-side).
12. **The in-game leg is green** — 0.2.2: `PASS` + `EXIT:0` on a launcher-built game at the
    campaign pin `0a875da` with phases A–L, control `EXIT:1`, `SELFTEST=1` `EXIT:1`,
    `SELFTEST=contents` `EXIT:1`, and the previous package failing phase L at `EXIT:1`
    (`evidence/industrialforegoing-e2e/`, `check-e2e-logs.py … --strict` green). The 0.2.1 run at
    `0b14f8a` (2026-11-20) was green for slice 1 only; 0.2.0 had no run and was refused at load,
    see [The doubled Part](#the-doubled-part-021-2026-11-20).
13. **The commands are typed now.** `run_mod_command` gets a parsed
    `CommandInvocation` (`<pos:block_pos>` resolves `~`/relative coordinates too)
    instead of the raw line, so `/industrialforegoing info 202 101 0` behaves as
    before and `/industrialforegoing info ~ ~ ~1` works as well — a widening, not
    a change. Permission levels are the same (`info` for everyone, the four
    setters at level 2).
14. **The augment slots are a saved list, not an item handler.** Upstream keeps two
    `InventoryComponent` slots; this machine has no menu (gap 1) and no item capability, so the
    two slots are the `titanium:augments` values themselves in the block entity's compound —
    what the arithmetic reads. A hopper cannot push an addon in, and the GUI's slot screen is
    absent; right-clicking the machine is the only seat. Two further small differences:
    `/industrialforegoing info` shows the scaled bar maximum as soon as an efficiency addon is
    seated, where upstream's `maxProgress` only changes after a completed operation (the
    arithmetic is identical — the in-flight cycle is counted on the old length in neither);
    and a refused second addon consumes the click (upstream falls through to the item's own
    `useOn`), which changes nothing for a plain addon.
15. **The four addon items are `give`-only.** `speed_addon_tier_1/2` and `efficiency_addon_tier_1/2`
    are the parent's own items with their `titanium:augments` defaults, but their recipes are
    dissolution-chamber recipes — that machine is a later slice — so nothing crafts them yet and
    they are not in a creative tab.
16. **The machine frames' rarities are vanilla slots, not Titanium's custom ones** (slice 4).
    `CustomRarity` registers four enum extensions (green/aqua/light-purple/gold); 26.3's declaration
    schema has `common`/`uncommon`/`rare`/`epic` only, so the four tiers map onto those **in order**
    (pity common, simple uncommon, advanced rare, supreme epic) and the item colours differ. Cosmetic
    only; recorded in [Slice 4](#slice-4--the-modulecore-item-and-block-set-024-2026-11-21).
17. ~~**Three machine frames and 16 lenses are `give`-only or craft-from-each-other**~~ **Closed in
    0.2.5 (slice 5), and `plastic` with it in 0.2.6 (slice 6).** `machine_frame_simple/advanced/
    supreme` and every lens are made by the dissolution chamber from the jar's own recipes, and the
    four addon items too (their chamber recipes were `give`-only in slice 3). `plastic` is now
    smelted from `dryrubber`, which the latex chain makes: phase AK's unit produces the rubber and
    phase AL's furnace turns it into one plastic.
18. **`c:gears` (the umbrella tag) has no consumer in this package.** It is carried whole (a pack's
    add-on may ask for "any gear"); its three sub-tags are each exercised by a craft.

## Provenance (upstream analysis)

Repository `https://github.com/Buuz135/Industrial-Foregoing`, commit
**`ce2852db4b34af3ab1b9637cff0b327d4bb561d5`** (`1.21`, 2026-07-04,
`mod_version=3.6`, `minecraft_version=1.21`); the Titanium library
(`InnovativeOnlineIndustries/Titanium`) for `ProgressBarComponent`,
`EnergyStorageComponent` and `FluidTankComponent`. Both shallow clones were
deleted after the findings were written. Files read and behaviour extracted:
[`FINDINGS.md`](FINDINGS.md). The jar stays as licence evidence in
`ports/_shared/licence-cache/industrialforegoing-1.21-3.6.39.jar` (provenance:
`ports/_shared/carry-work/industrialforegoing/SOURCE.md`). Clean-room rewrite:
no Java was copied or translated and no jar was decompiled.

Slice 3 re-read two upstream trees (shallow clones in `/tmp/opencode/`, deleted after the findings
were recorded above and in the slice section):- **Industrial Foregoing `ce2852db`** (`1.21`): `item/addon/AddonItem.java` (the `useOn` seat),
  `item/addon/SpeedAddonItem.java` / `EfficiencyAddonItem.java` (`1 + tier`, `1 - tier * 0.1`,
  `Properties().stacksTo(16)`), `block/tile/IndustrialWorkingTile.java` (the bar's
  `progressIncrease`, `maxProgress = floor(100 * efficiency)` after a work and
  `setProgress(maxProgress * workAmount)`), `block/tile/IndustrialMachineTile.java`
  (`canAcceptAugment`, the two-slot augment inventory), `module/ModuleCore.java` (tiers 1 and 2 of
  each addon).
- **Titanium `e53e8695`**: `item/AugmentWrapper.java` (the `titanium:augments` component, a
  `Map<String, Float>`, read through the stack's effective component), `api/augment/AugmentTypes.java`
  (`Speed`, `Efficiency`; the 1.21-era build also has `Processing`, not in the parent's working
  tile this machine uses), `component/progress/ProgressBarComponent.java` (the fall by
  `progressIncrease`, the finish-and-reset order).

Clean-room: the rewrite is this port's own Rust against the pin's APIs; the Java was read for
behaviour only, and where the pin cannot express something (no item handler on a menu-less machine)
the deviation is recorded above instead of faked.

Slice 6 re-read the same repository at `ce2852db` (`block/core/FluidExtractorBlock.java`,
`block/core/tile/FluidExtractorTile.java` — its `work()`, its `EXTRACTION` map,
`maxLatexTankSize`, `block/core/tile/LatexProcessingUnitTile.java` — its two tanks, its nine output
slots, `AMOUNT_LATEX`/`AMOUNT_WATER` and `canIncrease`, `config/machine/core/FluidExtractorConfig.java`,
`LatexProcessingUnitConfig.java`, `recipe/FluidExtractorRecipe.java` — its codec, `findRecipe`'s
named-before-default order and the nine `init` recipes, `block/tile/IndustrialWorkingTile.java`'s
`work()`/`onFinishWork` order and `IndustrialProcessingTile.java`'s bar). The jar's own JSON
(blockstates, models, the two machines' textures, their loot tables, the nine
`recipe/fluid_extractor/*.json`, the two shaped recipes and the plastic's smelting recipe) was the
authority for every carried file. The clone was deleted after the findings; no Java was copied or
translated.

Slice 5 re-read the same repository at `ce2852db` (`block/core/DissolutionChamberBlock.java`,
`block/core/tile/DissolutionChamberTile.java`, `recipe/DissolutionChamberRecipe.java`,
`config/machine/core/DissolutionChamberConfig.java`, `block/tile/IndustrialProcessingTile.java`,
`module/ModuleCore.java`'s `DISSOLUTION_CHAMBER`/`DISSOLUTION_SERIALIZER`/`DISSOLUTION_TYPE`) and
Titanium at `e53e869` (`component/progress/ProgressBarComponent.java`'s `tickBar`,
`component/progress/MultiProgressBarHandler.update`), plus `mifa`'s own jar for the recipe families its
add-on names. The jar's own JSON (blockstate, model, front texture, loot, 49 chamber recipe files) was
the authority for every carried file; 23 of the 49 are carried (the rest name outputs this package does
not ship). Both clones were deleted after the findings; no Java was copied or translated.

Slice 4 re-read the same repository at `ce2852db` for the `ModuleCore` set (`module/ModuleCore.java`:
the four `registerBlockWithItem` frames, `registerGeneric` gears/plastic and the `for (DyeColor value
: DyeColor.values())` lens loop; `block/MachineFrameBlock.java` for `ofFullCopy(IRON_BLOCK)` and
`droppingSelf`; `item/LaserLensItem.java` for `stacksTo(1)` and the composed name;
`utils/CustomRarity.java` for the four custom rarities; `recipe/provider/IndustrialRecipeProvider.java`
for the recolour recipes' 16-item alternative list). The jar's own JSON (blockstates, models,
textures, loot tables, recipes, advancements, tags) was the authority for every carried file; where
the jar and code disagreed the jar won (there is no inverted lens item). The clone was deleted after
the findings; no Java was copied or translated.

## Review repairs (PR #600)

1. Staging is the orchestrator's (horizon-mods#686).
2. *"Fix the upstream link in the description from relative
   `Buuz135/Industrial-Foregoing` to its absolute GitHub URL"* — done, with
   Buuz135 credited.
3. *"Neither … includes a LICENSE/copyright notice"* — `mixins/industrialforegoing/LICENSE`
   and `NOTICE.md` travel inside the `.hnmod`, and both sit at the embedded
   content archive's root.
4. *"`water_condensator.json` requires `#c:gears/iron`,
   `#industrialforegoing:machine_frame/pity` and `#c:plastics`, but the content
   contains no item tags or implementations … A standalone installation
   therefore cannot craft the sole machine"* — see **Upstream defects fixed**.
5. *"Replace the stale staging/template audits"* (`test_stage.py:30`,
   `test_template.py:43-54,62`, `audit/src/main.rs:38-39`) — `test_stage.py`
   compares the plan with this bundle's declared counts, `test_template.py` is
   documented as the synthetic fixture it is, and the audit accepts the
   cancellable embedded-content injection.

## Upstream defects fixed

The carried recipe references three ingredient **tags**: `#c:gears/iron`,
`#c:plastics` and `industrialforegoing:machine_frame/pity`. Upstream they are
supplied by Industrial Foregoing's own machine frames and by other mods; this
package carries neither, so as written the recipe can never match and the one
machine it ships is uncraftable.

**2026-11-21 (slice 2), corrected repair.** The first repair made each ingredient
a **list** — the tag or a vanilla fallback. That shape is invalid at 26.3: the
loader logs `recipes: invalid ingredient (unresolved tag or malformed input)`
for every list naming an absent tag and drops the whole recipe, so the machine
stayed uncraftable and the error only appeared in the game log. Each ingredient
is now the vanilla fallback **alone** (`minecraft:slime_ball`, `minecraft:iron_block`,
`minecraft:iron_block`), the only shape that both loads and crafts in a standalone
package; the recipe's advancement also loses the `neoforge:item_exists`
condition 26.3 does not know (`unknown load conditions … file left out`) and its
`has_item` criterion names `minecraft:slime_ball` instead of the absent
`#c:plastics`. A user notices that the Water Condensator can be made in a plain
crafting table and that its recipe advancement unlocks.

Evidence: the game's own log lines are quoted in
[Slice 2](#slice-2--contents-survive-the-break--the-recipe-crafts-022-2026-11-21);
phase K of the e2e crafts the recipe and asserts output, count, consumption and
an empty wrong grid.

### Verification after the repairs

- Artifact: `industrialforegoing.hnmod`, 20,872 B, SHA-256 `2f874866896b55efe6c1d73157666235143f462711014802a2c49ba83424a43c` (rebuilt twice, identical bytes; `tools/package.py`; the embedded content equals `content/industrialforegoing.hmod`).
- `build-slot.sh e2e --job … --port ports/industrialforegoing -- tools/e2e.sh`: PASS, log `ports/_shared/evidence/industrialforegoing/e2e-repair.log` ends `EXIT:0` (`check-e2e-logs.py industrialforegoing --strict` exit 0).
- `python3 ports/tools/upstream_ready.py industrialforegoing` and `check-skills.py --notes ports/industrialforegoing`: exit 0.
