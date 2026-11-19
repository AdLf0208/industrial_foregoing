# Industrial Foregoing — jar inventory and slice-1 findings

Jar: `ports/_shared/licence-cache/industrialforegoing-1.21-3.6.39.jar`
(SHA-1 `a653f5c2b56f51c82d008b473f2b89aeff5747fd` = the pack record's
`modrinthHash` in `ports/_shared/licence-cache/atm10-modlist.json`; SHA-256
`abeb644675398e7729eb5f3637e1dfbe080e0dbdfcced1b19bd4ab595ccdd4bb`).
`META-INF/neoforge.mods.toml`: `modId = "industrialforegoing"`,
`version = "1.21-3.6.39"`, `license = "MIT"`, author `Buuz135`; the jar carries
no separate LICENSE/NOTICE file. Extraction for inspection:
`ports/_shared/carry-work/industrialforegoing/jar/` (kept as evidence).

## What the jar actually contains (measured, not from the name)

| Half | Count | Notes |
|---|---:|---|
| `assets/industrialforegoing/blockstates/` | 72 | machines, frames, mycelial generators, conveyors, black-hole units/tanks |
| `assets/industrialforegoing/models/block/` | 153 | many use `industrialforegoing:block/base_block` |
| `assets/industrialforegoing/models/item/` | 220 | |
| `assets/industrialforegoing/textures/block/` | 146 | |
| `assets/industrialforegoing/textures/item/` | 106 | |
| `assets/industrialforegoing/textures/gui/` | 8 | |
| `assets/industrialforegoing/lang/en_us.json` | 683 keys | 100 `block.industrialforegoing.*` names |
| `data/industrialforegoing/recipe/` | 214 | shaped/shapeless/serializer recipes |
| `data/industrialforegoing/loot_table/` | 71 | |
| `data/industrialforegoing/advancement/` | 212 | mostly recipe unlocks |
| `data/industrialforegoing/tags/` | 14 | machine-frame tiers, blacklists, item groups |
| `data/c/tags/` | 17 | convention tags the recipes name (`c:plastics`, `c:gears/iron`, …) |
| `data/minecraft/tags/block/mineable/pickaxe.json` | 1 | every machine is pickaxe-mineable |

Block families seen in the blockstates (complete list, 72 ids): `animal_baby_separator`,
`animal_feeder`, `animal_rancher`, `biofuel_generator`, `bioreactor`, `block_breaker`,
`block_placer`, `conveyor`, `dark_glass`, `dissolution_chamber`, `dye_mixer`,
`enchantment_applicator`, `enchantment_extractor`, `enchantment_factory`,
`enchantment_sorter`, `fermentation_station`, `fluid_collector`, `fluid_extractor`,
`fluid_laser_base`, `fluid_placer`, `fluid_sieving_machine`, `hydroponic_bed`,
`infinity_backpack_block`, `infinity_charger`, `laser_drill`, `latex_processing_unit`,
the four `machine_frame_*`, `marine_fisher`, `material_stonework_factory`,
`mechanical_dirt`, `mob_crusher`, `mob_detector`, `mob_duplicator`,
`mob_slaughter_factory`, the sixteen `mycelial_*`, `ore_laser_base`,
`pink_slime_block`, `pitiful_generator`, `plant_fertilizer`, `plant_gatherer`,
`plant_sower`, `potion_brewer`, `resourceful_furnace`, `sewage_composter`, `sewer`,
`simulated_hydroponic_bed`, `sludge_refiner`, `spores_recreator`, `stasis_chamber`,
`transporter`, `washing_factory`, `water_condensator`, `wither_builder`.

This is a framework-sized mod (a Titanium library, ~60 machines, an add-on
ecosystem). Slice 1 is **one machine family**, chosen from the jar and the
upstream behaviour — not a framework.

## Slice chosen: the Water Condensator (`industrialforegoing:water_condensator`)

**Why this machine.** It is the smallest honest `machines`-tier unit in the jar:
one block, one block entity, one **FE buffer** (10,000 FE), one **fluid tank**
(16,000 mB) and one pure rule — it condenses water out of the air when at least
two of its six neighbours are water source blocks. No item slots, no world
mutation, no recipes beyond its own craft, no entity/dimension work. Its whole
behaviour can be asserted as numbers through a readout, which makes the e2e
fail-capable, and its "two or more water sources" rule gives a differential
(1 source vs 2) that is impossible to fake.

**Upstream behaviour** (read, not copied): `WaterCondensatorTile` (extends
`IndustrialWorkingTile`, which owns the progress bar and the once-per-finish
`work()` call) at commit `ce2852db4b34af3ab1b9637cff0b327d4bb561d5` of
`https://github.com/Buuz135/Industrial-Foregoing` (`1.21` branch, 2026-07-04,
`mod_version=3.6`; the pack jar is `3.6.39`, the same behaviour), plus
`WaterCondensatorConfig` and Titanium's `ProgressBarComponent`,
`EnergyStorageComponent` and `FluidTankComponent`:

- **Buffer:** capacity 10,000 FE (`maxStoredPower`), receive/extract unlimited
  (the three-int `EnergyStorageComponent(capacity, x, y)` overload makes maxIO =
  capacity; `10`/`20` are GUI pixels, not rates).
- **Tank:** 16,000 mB (`maxWaterTankSize`), water only, `Action.DRAIN` (the
  machine only ever outputs; internal `fillForced` bypasses that).
- **Progress bar:** starts full (100 `maxProgress`), counts **down** one a tick,
  and when it reaches 0 runs `work()` once, extracts what the work returned and
  sets progress to `floor(100 * workAmount)`. So the cooldown after an attempt
  is the returned fraction of 100 ticks: 10 ticks after a powered operation,
  50 after an unpowered one, 100 after an idle one.
- **`work()`:** count water source blocks on all six sides (`FluidState.isSource`
  — a source block, not flowing water, not a waterlogged block). With **fewer
  than 2**: nothing (workAmount 1 → 100-tick cooldown, 0 FE). With **2 or
  more** and at least 1,000 FE: add `sources * 100` mB and spend 1,000 FE
  (workAmount 0.1 → 10-tick cooldown). With 2 or more and **under 1,000 FE**:
  add `sources * 50` mB and spend nothing (workAmount 0.5 → 50-tick cooldown).
- A full tank still consumes the 1,000 FE (`fillForced` clamps; the machine does
  not check the room first), which the port reproduces.
- The block is `Properties.ofFullCopy(Blocks.IRON_BLOCK)` (hardness 5.0, blast
  resistance 6.0, correct tool for drops, iron sounds) with Titanium's
  `RotationType.FOUR_WAY` (`subfacing` state property, 4 values) and the
  `IndustrialBlockItem` in the `industrialforegoing_resource_production` tab.
- No GUI model change beyond the progress/tank/energy bars; no redstone
  behaviour is set (the default `RedstoneAction.IGNORE` makes `shouldWork()` true).

## Slice boundary (and what is deliberately outside it)

In: the block + its content (blockstate, models, textures, item definition,
loot table, recipe, advancement, pickaxe tag, lang subset), the block entity,
the declared FE buffer and water tank, the tick rule above, the placement
rotation, and four harness commands (`info`/`energy`/`water`/`progress`, plus
`drain` to prove the tank is a real handler).

Out (recorded as gaps in `notes.md`): the GUI screen (tank/energy/progress bars
and tooltip), redstone control modes, speed/efficiency/processing augments,
the Titanium plugin/config surface, every other machine, the machine-frame
tiers, ores/worldgen, fluids of its own, the latex/plastic chain, and the
Titanium library itself — `titanium` is a `library` row mapped to Horizon
APIs/gaps, so the slice reimplements only the two components it needs (energy
storage arithmetic and a drain-only tank) against the pin's APIs.

The recipe is carried but its `c:plastics` and `industrialforegoing:machine_frame/pity`
ingredients are other unported parts of the mod, so it cannot be crafted in this
slice; the block is obtained with `/give` (and the e2e does exactly that).
