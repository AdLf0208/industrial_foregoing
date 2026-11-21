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
and slice 2 (0.2.2, 2026-11-21) adds the machine's **contents surviving a break** and makes its
**carried recipe actually craft**. The current pin is nightly
**`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`** + mixins
**`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`**. **0.2.0 — the version merged in the registry —
could not load at all**: it attached `Machine` as both the block's Part and the block entity
type's Part, which the engine refuses ("`industrialforegoing::Machine is attached to
industrialforegoing:water_condensator twice`"), and it had no in-game run to catch that. 0.2.1
removes the redundant `block_part` call (see [The doubled Part](#the-doubled-part-021-2026-11-20));
0.2.2 carries that repair **and never shipped before**, so it is the first version of this mod
that both loads and keeps its contents. See
[Slice 2](#slice-2--contents-survive-the-break--the-recipe-crafts-022-2026-11-21). The gap list
at the end is the rest of the mod.

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
| augments, comparator, config file | `not ported` — gaps 3, 7, 8 |
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

## Skills

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices,rust-port

`cargo fmt`, `cargo clippy --all-targets -- -D warnings` and `cargo test` were
run before hand-back. `#[allow(private_interfaces, reason = "…")]` marks the
injected module's own private `Game`/`ModBlockEntity` parameters (the same
exception `botanypots`, `powah`, `charginggadgets` and `mininggadgets` record).

## Verification

**[`VALIDATION.md`](VALIDATION.md) is the ladder for the delivered 0.2.2 package** at the
campaign pin `0a875da…` (tests, fmt, clippy, two `cmp`-identical builds, `check`, both `fits`
runs, the 0.2.0 load refusal rebuilt at this pin, and the six in-game legs with their exit
statuses). The tables below are the earlier ladders, kept as history.

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
| `industrialforegoing.hmod` (0.2.2) | 34,096 B | `2672a80085cb0a258239408750b625fbee497f760693e473d823c0b947404826` | 25 |
| `industrialforegoing.hmod` (0.2.1, never published) | 30,694 B | `4cc91337a2d92c023aad9d2bcfd9a83ef1c37c284bf1b33b8bbaeafebeaa1b55` | 25 |
| `industrialforegoing.hmod` (0.2.0, superseded) | 28,979 B | `c10182f32584e6522b274855f6c8331793421237112d87acf804a4fb17752ed7` | 25 |

It holds `horizon-mod.toml`, `Cargo.toml`, `src/lib.rs`, `src/runtime.rs`, `uses.json`,
`LICENSE`, `LICENSE-NOTICE.md`, `README.md` and the 17 carried content files — the whole mod, in
the single file a registry entry names.

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
3. **No augments.** Speed, efficiency and processing augments are Titanium
   components; none is ported, so every machine runs at the base rate.
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
6. ~~**The recipe cannot be crafted in this slice.**~~ **Closed in 0.2.2.** The carried recipe's
   tag-or-item **list** ingredients were invalid at 26.3 (the loader logged
   `recipes: invalid ingredient …` for `#c:plastics`, `#c:gears/iron` and
   `#industrialforegoing:machine_frame/pity` and dropped the whole recipe), so the machine could
   not be crafted at all. The three ingredients are their vanilla fallbacks now
   (`minecraft:slime_ball`, `minecraft:iron_block`, `minecraft:iron_block`); phase K crafts it in a
   crafting table, asserts the output and the consumption, and a wrong grid stays empty. The
   `neoforge:item_exists` gate the advancement carried (also unknown to 26.3, so the file was left
   out) is dropped. When the frames/plastics are ported, the ingredients can name the tags again —
   as **single** tags, not lists.
7. **No comparator output and no block-entity update tag.** The power level and
   the tank are not exposed to comparators, and nothing is sent to clients
   (there is no client-side view to send it for).
8. **No config file.** The three values are the upstream defaults
   (`maxProgress` 100, `powerPerOperation` 1000, `maxStoredPower` 10000,
   `maxWaterTankSize` 16000); `config/` is not created.
9. **One machine only.** The jar's other 71 blockstates (frames, mycelial
   generators, the latex/plastic chain, conveyors, black-hole units, the laser
   drills, …) are outside the slice; `industrialforegoingsouls` and `mifa`
   (the pack's add-ons) stay untouched.
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
