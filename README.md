# Industrial Foregoing — Horizon port (slice 2: the Water Condensator)

A Horizon mod for **Industrial Foregoing 1.21-3.6.39**
(`Buuz135/Industrial-Foregoing`, **MIT**): the Water Condensator, a machine
with a 10,000 FE buffer and a 16,000 mB tank that condenses water out of the
air when at least two of its six sides are water source blocks.

**LOCAL-ONLY** until the maintainer approves publishing (ARCHITECTURE.md §8).
MIT is licence rank 0 (permissive); approval is still required.

Industrial Foregoing is a framework-sized tech mod (~60 machines on the
Titanium library, plus add-ons); slice 1 is deliberately **one machine family
with its FE buffer**, not a framework. Provenance and hashes are in
`ports/_shared/carry-work/industrialforegoing/SOURCE.md`; the behaviour reading
and the slice boundary are in `FINDINGS.md`; the ladder, deviations and gaps are
in `notes.md`; the content decisions are in `CONTENT.md`.

## What it does

| | |
|---|---|
| Buffer | 10,000 FE (`WaterCondensatorConfig.maxStoredPower`), in and out on every side |
| Tank | 16,000 mB, water only, drain-only (the machine fills it) |
| Rule | fewer than 2 water source blocks beside it: nothing; 2 or more and ≥ 1,000 FE: `sources × 100` mB for 1,000 FE; 2 or more and < 1,000 FE: `sources × 50` mB for free |
| Bar | starts full (100), counts down a tick; after an attempt the next value is 10 (powered), 50 (unpowered) or 100 (idle), as Titanium's `workAmount` sets it |
| Placement | `subfacing` (Titanium's `FOUR_WAY`), the face turning toward the placer |
| Broken | the item carries the machine's FE, water and bar (vanilla `minecraft:block_entity_data`), and placing it restores them |
| Recipe | craftable in a plain crafting table (the absent convention tags are the vanilla fallbacks) |

## Layout

```
horizon-mod.toml                   the only manifest and the only identity (api 0.2, [native] pins)
Cargo.toml                         one crate; its only dependency is horizon-mod-api from .horizon/sdk/
src/lib.rs                         the one #[mod_api::horizon_mod] impl Hooks, the Machine Part,
                                   the block entity type, the energy/fluid declarations, the commands
src/runtime.rs                     pure: the bar, the attempt, the buffer (18 tests), byte-identical
                                   to the retired .hnmod port's runtime.rs
assets/industrialforegoing/        the carried blockstate, models, textures, item definition, lang
data/industrialforegoing/          the block/tab declarations, loot table, recipe, advancement;
data/minecraft/tags/block/         the pickaxe tag
industrialforegoing.hmod           the built package (code + content in one flat file)
CONTENT.md                         what is carried and what is written
FINDINGS.md                        jar inventory, upstream behaviour, slice boundary
notes.md                           the record: the SDK migration, deviations, gaps, verification
tools/assemble_content.py          the jar → assets/data carry pipeline
tools/e2e.sh                       the in-game scenario (needs a launcher-built game; see notes.md)
```

## Commands (harness controls)

```
/industrialforegoing info <x> <y> <z>                 FE, water, sources, bar
/industrialforegoing energy <x> <y> <z> <amount>      set the buffer
/industrialforegoing water <x> <y> <z> <amount>       set the tank
/industrialforegoing progress <x> <y> <z> <ticks>     set the bar
/industrialforegoing drain <x> <y> <z> <amount>       drain through the tank capability
```

Upstream ships no commands; these exist so the harness can read and set a
machine without a GUI (precedent `/powah info|set`, `/charginggadgets energy`).

## Build and checks

```sh
# from the campaign root; the slot runner blocks until a slot is free
ports/tools/build-slot.sh run --job IF-test --port ports/industrialforegoing --step test -- \
    bash -c 'cd ports/industrialforegoing && cargo test'
ports/tools/build-slot.sh run --job IF-lint --port ports/industrialforegoing --step lint -- \
    bash -c 'cd ports/industrialforegoing && cargo clippy --all-targets -- -D warnings'
ports/tools/build-slot.sh run --job IF-build --port ports/industrialforegoing --step build -- \
    bash -c 'cd ports/industrialforegoing && rm -f Cargo.lock && cargo horizon-mod build'
ports/tools/build-slot.sh run --job IF-check --port ports/industrialforegoing --step check -- \
    bash -c 'cd ports/industrialforegoing && cargo horizon-mod check industrialforegoing.hmod'
```

Never run a bare `cargo` outside a lease and never pass `--target-dir`; read
`ports/_shared/build-slots-usage.md`. The `.hmod` is the deliverable: copy it
from `target/horizon-mod/` to the port root after a build.
