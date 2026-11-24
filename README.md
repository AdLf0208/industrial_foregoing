# Industrial Foregoing — Horizon port (the Water Condensator, the Dissolution Chamber, the Core set and the animal and sewage machines)

A Horizon mod for **Industrial Foregoing 1.21-3.6.39**
(`Buuz135/Industrial-Foregoing`, **MIT**): the Water Condensator, a machine
with a 10,000 FE buffer and a 16,000 mB tank that condenses water out of the
air when at least two of its six sides are water source blocks; the
**Dissolution Chamber**, which runs its own recipe type (23 carried recipes:
the three higher machine frames, the 16 lenses and the four addon items) from
eight input slots, three output slots, two tanks of the mod's own fluids and a
10,000 FE buffer at 90 FE a tick; and the `ModuleCore` item and block set — the
four machine frames, the iron, gold and diamond gears, plastic and the 16
coloured laser lenses, with the tags the pack's add-ons bind to.

**LOCAL-ONLY** until the maintainer approves publishing (ARCHITECTURE.md §8).
MIT is licence rank 0 (permissive); approval is still required.

Industrial Foregoing is a framework-sized tech mod (~60 machines on the
Titanium library, plus add-ons); this port ships **one machine family with its
FE buffer** and the second machine its own add-ons depend on, not a framework. Provenance and hashes are in
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
| Recipe | craftable in a plain crafting table from `#c:plastics`, `#c:gears/iron` and `#industrialforegoing:machine_frame/pity` |
| Core set | four machine frames (placed, pickaxe-mined, they drop themselves), three gears, plastic and 16 recolourable lenses, with `industrialforegoing:machine_frame/*`, `c:gears`, `c:gears/*` and `c:plastics` |
| Lenses | any lens plus the colour's dye (`#c:dyes/<colour>`) makes that colour's lens |
| Chamber buffer | 10,000 FE (`DissolutionChamberConfig.maxStoredPower`), 90 FE a tick (`powerPerTick`) |
| Chamber tanks | 8,000 mB in, 16,000 mB out, `industrialforegoing:latex`/`:pink_slime`/`:ether_gas` — declared fluid keys; `/industrialforegoing fluid` fills the tank (the pin's fluid insert takes only water and lava) |
| Chamber recipe | its own type, `industrialforegoing:dissolution_chamber`: 0–8 item ingredients, an input fluid, a `processingTime`, an item output — matched only when the input slots hold **exactly** the recipe's item set |
| Chamber products | `machine_frame_simple/advanced/supreme`, the 16 lenses, `speed_addon_tier_1/2`, `efficiency_addon_tier_1/2` |

## The sewage chain (0.2.14)

The **Sewer** (`industrialforegoing:sewer`) is a 100-tick `RangeType.TOP` machine: it counts the
animals in the block above, fills its drain-only 8,000 mB `industrialforegoing:sewage` tank with
**50 mB an animal**, and spends 10 FE × (animals + 1) from its 10,000 FE buffer (the jar's own
`++amount`). The **Sewage Composter** (`industrialforegoing:sewage_composter`) takes 1,000 mB of
sewage in a fill-only 8,000 mB tank and, over a 100-tick bar at 40 FE a tick, inserts one
`industrialforegoing:fertilizer` into its 12-slot output-only container. `/industrialforegoing
sewage <pos> <mB>` writes the composter's tank (mod fluids have no insert path at this pin);
`info` reads the tank back through the declared capability. The jar's second sewer tank (essence,
fed by XP orbs and players) is not in this slice.

## The Plant Gatherer (0.2.16)

The **Plant Gatherer** (`industrialforegoing:plant_gatherer`) is an input-free machine with a
10,000 FE buffer, a 40-tick bar and a 15-slot output container. At range 0 it works the single cell
behind its `subfacing`: a mature crop is harvested for its own block drops (rolled from the crop's
loot table), the cell becomes air, and the operation spends 400 FE and rests `floor(40 × 0.3) = 12`
ticks. An immature crop, a full output or an empty buffer spends nothing and rests the full bar.
The sludge/ether tanks and the ether-bar replant path are not in this slice. Its shaped recipe is
carried from the jar and names `#c:plastics`, `#c:gears/gold` and `machine_frame/pity`.

## Layout

```
horizon-mod.toml                   the only manifest and the only identity (api 0.2, [native] pins)
Cargo.toml                         one crate; its only dependency is horizon-mod-api from .horizon/sdk/
src/lib.rs                         the one #[mod_api::horizon_mod] impl Hooks, the Machine Part,
                                   the block entity type, the energy/fluid declarations, the commands
src/runtime.rs                     pure: the bar, the attempt, the buffer, the augments (26 tests),
                                   byte-identical to the retired .hnmod port's runtime.rs
src/chamber.rs                     pure: the dissolution chamber's rules (20 tests) — tanks,
                                   the exact-set matcher, the bar, 90 FE a tick, the augments
src/dissolution.rs                 the chamber's game side: its declarations, the recipe table
                                   read through recipes_of_type, the tick, the readout
assets/industrialforegoing/        the carried blockstate, models, textures, item definition, lang
data/industrialforegoing/          the block/tab declarations, loot table, recipe, advancement;
data/minecraft/tags/block/         the pickaxe tag
industrialforegoing.hmod           the built package (code + content in one flat file)
CONTENT.md                         what is carried and what is written
FINDINGS.md                        jar inventory, upstream behaviour, slice boundary
notes.md                           the record: the SDK migration, deviations, gaps, verification
tools/assemble_content.py          the jar → assets/data carry pipeline
tools/verify_content.py            the cross-reference of the carried content (--selftest, --package)
tools/craft_recipe.py              crafts every shaped recipe in game (the tags, the frame, the gears)
tools/recolor.py                   crafts the 16 lens recolours in the player's 2x2, counts before/after
tools/parse_chamber.py             parses a chamber `info` line into shell variables
tools/e2e.sh                       the in-game scenario (needs a launcher-built game; see notes.md)
```

## Commands (harness controls)

```
/industrialforegoing info <x> <y> <z>                 FE, tanks, bar, recipe (both machines)
/industrialforegoing energy <x> <y> <z> <amount>      set the buffer
/industrialforegoing water <x> <y> <z> <amount>       set the tank
/industrialforegoing progress <x> <y> <z> <ticks>     set the bar (either machine)
/industrialforegoing drain <x> <y> <z> <amount>       drain through the tank capability
/industrialforegoing fluid <x> <y> <z> <fluid> <amt>  set the chamber's input tank (latex, pink_slime, …)
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
