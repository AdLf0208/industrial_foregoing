#!/usr/bin/env python3
"""Check the carried content of the Industrial Foregoing package, against itself.

Every rule this slice is about is a cross-reference inside `assets/` and `data/`:
a recipe that names an absent item or tag is dropped whole by 26.3 (slice 2's
`#c:plastics` list), a block with `requires_tool` and no `minecraft:mineable/*`
entry drops nothing, and an id with no lang key shows the raw key in game. The
`src/lib.rs` `content` tests pin the same rules on the tree; this tool is the
JSON-level pass and can read the **delivered `.hmod`**, i.e. the bytes that ship.

    python3 tools/verify_content.py                       # the port tree
    python3 tools/verify_content.py --package industrialforegoing.hmod
    python3 tools/verify_content.py --selftest            # prove it can fail

Exit 0 when every check passes, 1 with one line per failure.
"""

from __future__ import annotations

import argparse
import io
import json
from pathlib import Path
import sys
import zipfile

ROOT = Path(__file__).resolve().parents[1]
MOD = "industrialforegoing"
FRAMES = [
    "machine_frame_pity",
    "machine_frame_simple",
    "machine_frame_advanced",
    "machine_frame_supreme",
]
COLOURS = [
    "white",
    "orange",
    "magenta",
    "light_blue",
    "yellow",
    "lime",
    "pink",
    "gray",
    "light_gray",
    "cyan",
    "purple",
    "blue",
    "brown",
    "green",
    "red",
    "black",
]
PLAIN_ITEMS = ["iron_gear", "gold_gear", "diamond_gear", "plastic", "dryrubber"]
# Slice 5: the dissolution chamber and the file of its own recipe type.
CHAMBER = "dissolution_chamber"
# Slice 6: the latex chain's two machines, their own recipe type's files, and their lengths.
LATEX = ["fluid_extractor", "latex_processing_unit"]
EXTRACTOR_RECIPES = 9
# Slice 7: the laser drill and its ore base, and the ore recipes of their own type.
LASER = ["laser_drill", "ore_laser_base"]
ORE_RECIPES = 10
# Slice 8: the fluid laser base and the three files of its own recipe type.
FLUID_LASER = ["fluid_laser_base"]
FLUID_RECIPES = 3
FLUID_OUTPUTS = {"minecraft:lava", "industrialforegoing:ether_gas", "pneumaticcraft:oil"}
BRAND_FLUIDS = {"industrialforegoing:latex", "industrialforegoing:pink_slime",
                "industrialforegoing:ether_gas", "industrialforegoing:essence"}
# Slice 9: the mob slaughter factory and the pink slime set.
SLAUGHTER = ["mob_slaughter_factory"]
# Slice 11: the mob crusher (its own tank, output container and kill hooks).
CRUSHER = ["mob_crusher"]
PINK_ITEMS = ["pink_slime", "pink_slime_ingot"]
PINK_BLOCK = "pink_slime_block"


class Files:
    """The package's files, from a directory or the delivered archive."""

    def __init__(self, path: Path):
        self.path = path
        if path.is_dir():
            self.names = sorted(
                name.relative_to(path).as_posix()
                for folder in ("assets", "data")
                for name in (path / folder).rglob("*")
                if name.is_file()
            )
            self._zip: zipfile.ZipFile | None = None
        else:
            self._zip = zipfile.ZipFile(path)
            self.names = sorted(
                name
                for name in self._zip.namelist()
                if (name.startswith("assets/") or name.startswith("data/"))
                and not name.endswith("/")
            )

    def has(self, name: str) -> bool:
        return name in self.names

    def read(self, name: str) -> bytes:
        if self._zip is not None:
            return self._zip.read(name)
        return (self.path / name).read_bytes()

    def json(self, name: str):
        return json.loads(self.read(name))


def namespace(item_id: str) -> str:
    return item_id.split(":", 1)[0]


def declared_ids(files: Files) -> set[str]:
    """Every `<id>:<path>` the package declares as an item or a block item."""
    out = set()
    for name in files.names:
        for kind in ("item", "block"):
            prefix = f"data/{MOD}/horizon/{kind}/"
            if name.startswith(prefix) and name.endswith(".json"):
                out.add(f"{MOD}:{name[len(prefix):-len('.json')]}")
    return out


def region(files: Files, prefix: str) -> list[str]:
    """The package's JSON files under `prefix`."""
    return [n for n in files.names if n.startswith(prefix) and n.endswith(".json")]


def pickaxe_values(files: Files) -> list[str]:
    """The ids in `minecraft:mineable/pickaxe`."""
    name = "data/minecraft/tags/block/mineable/pickaxe.json"
    return files.json(name).get("values", []) if files.has(name) else []


def tag_id(name: str) -> str:
    """`data/<ns>/tags/item/<path>.json` -> `<ns>:<path>`."""
    _, ns, _, _, rest = name.split("/", 4)
    return f"{ns}:{rest[: -len('.json')]}"


def check(files: Files) -> list[str]:
    failures: list[str] = []
    items = declared_ids(files)
    blocks = {
        f"{MOD}:{name[len(f'data/{MOD}/horizon/block/'):-len('.json')]}"
        for name in region(files, f"data/{MOD}/horizon/block/")
    }
    tag_files = [name for name in region(files, "data/") if "/tags/item/" in name]
    tag_values = {name: files.json(name).get("values", []) for name in tag_files}
    tags = {tag_id(name) for name in tag_files}
    declared = items | blocks

    def resolves(spec: str, where: str) -> None:
        """One ingredient or tag value: a bare id or a `#tag`."""
        if spec.startswith("#"):
            tag = spec[1:]
            if namespace(tag) == "minecraft":
                return
            if tag not in tags:
                failures.append(f"{where} names the tag {spec}, which the package does not declare")
                return
            name = f"data/{namespace(tag)}/tags/item/{tag.split(':', 1)[1]}.json"
            if not tag_values.get(name):
                failures.append(f"{where}: {spec} resolves to nothing (its file has no values)")
            return
        if namespace(spec) == "minecraft":
            return
        if spec not in declared:
            failures.append(f"{where} names {spec}, which the package does not declare")

    # The slice's ids are declared, named and modelled.
    expected = (
        [f"{MOD}:{CHAMBER}"]
        + [f"{MOD}:{machine}" for machine in LATEX]
        + [f"{MOD}:{machine}" for machine in LASER]
        + [f"{MOD}:{machine}" for machine in SLAUGHTER]
        + [f"{MOD}:{machine}" for machine in CRUSHER]
        + [f"{MOD}:{PINK_BLOCK}"]
        + [f"{MOD}:{item}" for item in PINK_ITEMS]
        + [f"{MOD}:{frame}" for frame in FRAMES]
        + [f"{MOD}:{item}" for item in PLAIN_ITEMS]
        + [f"{MOD}:{colour}_laser_lens" for colour in COLOURS]
    )
    for item_id in expected:
        path = item_id.split(":", 1)[1]
        if item_id not in declared:
            failures.append(f"{item_id} is not declared")
            continue
        for name in (
            f"assets/{MOD}/items/{path}.json",
            f"assets/{MOD}/models/item/{path}.json",
        ):
            if not files.has(name):
                failures.append(f"{item_id} has no {name}")
        lang = files.json(f"assets/{MOD}/lang/en_us.json")
        kind = "block" if item_id in blocks else "item"
        key = f"{kind}.{item_id.split(':', 1)[0]}.{item_id.split(':', 1)[1]}"
        if not lang.get(key):
            failures.append(f"{key} has no English name")

    # The chamber: declared, named, modelled, with a blockstate and a loot table that drops it.
    if f"{MOD}:{CHAMBER}" not in blocks:
        failures.append(f"the dissolution chamber is not declared as a block")
    for name in (
        f"assets/{MOD}/blockstates/{CHAMBER}.json",
        f"data/{MOD}/loot_table/blocks/{CHAMBER}.json",
    ):
        if not files.has(name):
            failures.append(f"the dissolution chamber has no {name}")
        elif name.endswith("loot.json") and f"{MOD}:{CHAMBER}" not in json.dumps(files.json(name)):
            failures.append(f"{name} does not drop the chamber")

    for frame in FRAMES:
        loot = f"data/{MOD}/loot_table/blocks/{frame}.json"
        if not files.has(loot):
            failures.append(f"{frame} has no loot table")
        elif f"{MOD}:{frame}" not in json.dumps(files.json(loot)):
            failures.append(f"{loot} does not drop {frame}")
        if not files.has(f"assets/{MOD}/blockstates/{frame}.json"):
            failures.append(f"{frame} has no blockstate")

    # Every carried recipe: its result exists, and every ingredient resolves.
    recipes = [
        name
        for name in region(files, f"data/{MOD}/recipe/")
        if not name.startswith(f"data/{MOD}/recipe/fluid_extractor/")
    ]
    if len(recipes) < 24:
        failures.append(f"{len(recipes)} recipes carried, expected at least 24")
    for name in recipes:
        recipe = files.json(name)
        result = recipe.get("result", {}).get("id")
        if result and result not in declared:
            failures.append(f"{name} results in {result}, which the package does not declare")
        for key, value in recipe.get("key", {}).items():
            resolves(value, f"{name} key {key}")
        for value in recipe.get("ingredients", []):
            resolves(value, f"{name} ingredient")
        if "ingredient" in recipe:
            resolves(recipe["ingredient"], f"{name} ingredient")

    # The chamber's own recipe type: `input` cells are 26.3 strings, its `output` is a
    # shipped id, and its `inputFluid` names a fluid the brand declares.
    brand_fluids = BRAND_FLUIDS
    chamber_recipes = region(files, f"data/{MOD}/recipe/dissolution_chamber/")
    if len(chamber_recipes) < 23:
        failures.append(f"{len(chamber_recipes)} chamber recipes carried, expected at least 23")
    for name in chamber_recipes:
        recipe = files.json(name)
        if recipe.get("type") != f"{MOD}:{CHAMBER}":
            failures.append(f"{name} is not a {MOD}:{CHAMBER} recipe")
        output = recipe.get("output", {}).get("id")
        if output and output not in declared:
            failures.append(f"{name} results in {output}, which the package does not declare")
        for cell in recipe.get("input", []):
            if not isinstance(cell, str):
                failures.append(f"{name} has a non-string input cell {cell!r}")
            resolves(cell, f"{name} input")
        fluid = recipe.get("inputFluid", {}).get("fluid")
        if fluid not in brand_fluids:
            failures.append(f"{name} needs the fluid {fluid}, which the port does not declare")
        for key in ("outputFluid",):
            out = recipe.get(key)
            if out:
                fluid = out.get("id", out.get("fluid"))
                # A recipe's *output* may be a vanilla fluid (the pink slime ball drains water);
                # only what the tank is *filled* with has to be a key this port declares.
                if fluid not in brand_fluids and namespace(fluid or "minecraft:x") != "minecraft":
                    failures.append(f"{name} {key} names an undeclared fluid")

    # Slice 9: the mob slaughter factory — declared, named, modelled, with a blockstate, a
    # declaration, a loot table that drops it and a pickaxe entry; its shaped recipe names the
    # package's tags and results in the machine.
    for machine in SLAUGHTER + CRUSHER:
        item_id = f"{MOD}:{machine}"
        if item_id not in blocks:
            failures.append(f"the {machine} is not declared as a block")
        for name in (
            f"assets/{MOD}/blockstates/{machine}.json",
            f"assets/{MOD}/models/block/{machine}.json",
            f"assets/{MOD}/items/{machine}.json",
            f"data/{MOD}/horizon/block/{machine}.json",
        ):
            if not files.has(name):
                failures.append(f"the {machine} has no {name}")
        loot = f"data/{MOD}/loot_table/blocks/{machine}.json"
        if not files.has(loot):
            failures.append(f"the {machine} has no loot table")
        elif "\"type\": \"minecraft:survives_explosion\"" not in json.dumps(files.json(loot)):
            failures.append(f"{loot} is not in 26.3's condition shape, so it drops nothing")
        if item_id not in pickaxe_values(files):
            failures.append(f"the {machine} has requires_tool but no mineable/pickaxe entry")
    shaped = files.json(f"data/{MOD}/recipe/mob_slaughter_factory.json")
    keys = set(shaped.get("key", {}).values())
    for tag in ("#c:plastics", "#c:gears/gold", f"#{MOD}:machine_frame/pity"):
        if tag not in keys:
            failures.append(f"the mob slaughter factory recipe does not name {tag}")
    if shaped.get("result", {}).get("id") != f"{MOD}:mob_slaughter_factory":
        failures.append("the mob slaughter factory recipe does not result in the machine")

    # Slice 11: the mob crusher — the same machine checks, its own recipe (the jar's advanced
    # frame, gold gears and plastic), and its own saved-output declaration.
    crusher = files.json(f"data/{MOD}/recipe/{CRUSHER[0]}.json")
    crusher_keys = set(crusher.get("key", {}).values())
    for tag in ("#c:plastics", "#c:gears/gold", f"#{MOD}:machine_frame/advanced"):
        if tag not in crusher_keys:
            failures.append(f"the mob crusher recipe does not name {tag}")
    if crusher.get("result", {}).get("id") != f"{MOD}:{CRUSHER[0]}":
        failures.append("the mob crusher recipe does not result in the machine")
    if crusher.get("pattern") != ["PSP", "BMB", "GRG"]:
        failures.append("the mob crusher recipe is not the jar's PSP/BMB/GRG")

    # The pink slime set: the block (declared, modelled, drops itself) and the two items, their
    # recipes, the tags and the item group.
    if f"{MOD}:{PINK_BLOCK}" not in blocks:
        failures.append(f"{PINK_BLOCK} is not declared as a block")
    for name in (
        f"assets/{MOD}/blockstates/{PINK_BLOCK}.json",
        f"data/{MOD}/horizon/block/{PINK_BLOCK}.json",
        f"data/{MOD}/loot_table/blocks/{PINK_BLOCK}.json",
        f"assets/{MOD}/models/item/{PINK_BLOCK}.json",
    ):
        if not files.has(name):
            failures.append(f"the pink slime block has no {name}")
    declaration = files.json(f"data/{MOD}/horizon/block/{PINK_BLOCK}.json")
    if declaration.get("template") != "minecraft:slime_block":
        failures.append("the pink slime block is not declared on the slime block template")
    if declaration.get("properties", {}).get("friction") != 0.8:
        failures.append("the pink slime block is not declared with the jar's 0.8 friction")
    nine = files.json(f"data/{MOD}/recipe/pink_slime.json")
    if nine.get("type") != "minecraft:crafting_shapeless":
        failures.append("pink_slime.json is not shapeless")
    if nine.get("result") != {"count": 9, "id": f"{MOD}:pink_slime"}:
        failures.append("the pink slime block no longer gives nine pink slime")
    if nine.get("ingredients") != [f"{MOD}:{PINK_BLOCK}"]:
        failures.append("the nine pink slime recipe no longer takes the block")
    packed = files.json(f"data/{MOD}/recipe/{PINK_BLOCK}.json")
    if packed.get("result", {}).get("id") != f"{MOD}:{PINK_BLOCK}":
        failures.append("the pink slime block recipe does not result in the block")
    if set(packed.get("key", {}).values()) != {f"{MOD}:pink_slime"}:
        failures.append("the pink slime block recipe is not nine pink slime")
    for tag, value in (
        ("data/c/tags/item/ingots/pink_slime.json", f"{MOD}:pink_slime_ingot"),
        ("data/c/tags/block/storage_blocks/pink_slime.json", f"{MOD}:{PINK_BLOCK}"),
    ):
        if not files.has(tag):
            failures.append(f"the pink slime set has no {tag}")
        elif value not in files.json(tag).get("values", []):
            failures.append(f"{tag} does not name {value}")
    table = f"data/{MOD}/horizon/creative_tab/ag_hus.json"
    if not files.has(table):
        failures.append("the mob slaughter factory has no item group (ag_hus)")
    elif f"{MOD}:mob_slaughter_factory" not in files.json(table).get("items", []):
        failures.append("the ag_hus item group does not list the machine")

    # The chamber's own `outputFluid` path: the jar's only such recipe is the pink slime ball.
    ball = files.json(f"data/{MOD}/recipe/dissolution_chamber/pink_slime_ball.json")
    if balls := ball.get("outputFluid"):
        if balls.get("id") != "minecraft:water" or balls.get("amount") != 150:
            failures.append("the pink slime ball no longer drains 150 mB of water")
    else:
        failures.append("the pink slime ball lost its outputFluid, the only one carried")
    if ball.get("inputFluid", {}).get("fluid") != f"{MOD}:pink_slime":
        failures.append("the pink slime ball does not take this mod's pink slime")
    if ball.get("processingTime") != 200:
        failures.append("the pink slime ball is not the jar's 200-tick recipe")
    ingot = files.json(f"data/{MOD}/recipe/dissolution_chamber/pink_slime_ingot.json")
    if ingot.get("output", {}).get("id") != f"{MOD}:pink_slime_ingot":
        failures.append("the pink slime ingot recipe does not result in the ingot")
    if ingot.get("inputFluid", {}).get("amount") != 1000:
        failures.append("the pink slime ingot recipe is not the jar's 1,000 mB")

    # The latex chain: both machines declared, named, modelled, with a blockstate and a loot
    # table that drops them; the plastic's smelting recipe names a shipped item; and the
    # extractor's nine recipes of its own type input a 26.3 string and output a declared fluid.
    for machine in LATEX:
        item_id = f"{MOD}:{machine}"
        if item_id not in blocks:
            failures.append(f"the {machine} is not declared as a block")
        if not files.has(f"assets/{MOD}/blockstates/{machine}.json"):
            failures.append(f"the {machine} has no blockstate")
        loot = f"data/{MOD}/loot_table/blocks/{machine}.json"
        if not files.has(loot):
            failures.append(f"the {machine} has no loot table")
        elif item_id not in json.dumps(files.json(loot)):
            failures.append(f"{loot} does not drop {machine}")
        if not files.has(f"data/{MOD}/horizon/block/{machine}.json"):
            failures.append(f"the {machine} has no declaration")
        if f"{MOD}:{machine}" not in pickaxe_values(files):
            failures.append(f"the {machine} has requires_tool but no mineable/pickaxe entry")
    smelting = files.json(f"data/{MOD}/recipe/plastic.json")
    if smelting.get("type") != "minecraft:smelting":
        failures.append("plastic.json is not a smelting recipe")
    if smelting.get("ingredient") != f"{MOD}:dryrubber":
        failures.append("the plastic's smelting recipe does not name dryrubber")
    if smelting.get("result", {}).get("id") not in declared:
        failures.append("the plastic's smelting recipe results in an undeclared item")
    extractor_recipes = region(files, f"data/{MOD}/recipe/fluid_extractor/")
    if len(extractor_recipes) < EXTRACTOR_RECIPES:
        failures.append(
            f"{len(extractor_recipes)} fluid-extractor recipes carried, "
            f"expected at least {EXTRACTOR_RECIPES}"
        )
    for name in extractor_recipes:
        recipe = files.json(name)
        if recipe.get("type") != f"{MOD}:fluid_extractor":
            failures.append(f"{name} is not a {MOD}:fluid_extractor recipe")
        if not isinstance(recipe.get("input"), str):
            failures.append(f"{name} has a non-string input {recipe.get('input')!r}")
        else:
            resolves(recipe["input"], f"{name} input")
        if not isinstance(recipe.get("result"), str):
            failures.append(f"{name} has a non-string result {recipe.get('result')!r}")
        output = recipe.get("output", {}).get("id")
        if output not in BRAND_FLUIDS:
            failures.append(f"{name} outputs {output}, which the port does not declare")
        if not isinstance(recipe.get("breakChance"), (int, float)):
            failures.append(f"{name} has no breakChance")

    # The laser drill and its ore base: declared, named, modelled, with a blockstate and a loot
    # table that drops them; the drill's six-way state; and the ten ore recipes of their own
    # type, each with a resolvable output, a shipped catalyst and a depth band.
    for machine in LASER:
        item_id = f"{MOD}:{machine}"
        if item_id not in blocks:
            failures.append(f"the {machine} is not declared as a block")
        for name in (
            f"assets/{MOD}/blockstates/{machine}.json",
            f"data/{MOD}/loot_table/blocks/{machine}.json",
            f"data/{MOD}/horizon/block/{machine}.json",
        ):
            if not files.has(name):
                failures.append(f"the {machine} has no {name}")
        loot = f"data/{MOD}/loot_table/blocks/{machine}.json"
        if files.has(loot):
            table = files.json(loot)
            if item_id not in json.dumps(table):
                failures.append(f"{loot} does not drop {machine}")
            # The engine reads a condition object's `type`; the jar's
            # `{"condition": "minecraft:survives_explosion"}` has none and the pool never drops.
            condition = table.get("pools", [{}])[0].get("condition", {})
            if condition.get("type") != "minecraft:survives_explosion":
                failures.append(f"{loot} does not use the package's 26.3 condition shape")
        if f"{MOD}:{machine}" not in pickaxe_values(files):
            failures.append(f"the {machine} has requires_tool but no mineable/pickaxe entry")
    drill = files.json(f"data/{MOD}/horizon/block/laser_drill.json")
    if set(drill.get("state_properties", {}).get("facing", {}).get("values", [])) != {
        "north",
        "south",
        "east",
        "west",
        "up",
        "down",
    }:
        failures.append("the laser drill's facing is not SIX_WAY")
    for machine, tag in (("laser_drill", "simple"), ("ore_laser_base", "advanced")):
        recipe = files.json(f"data/{MOD}/recipe/{machine}.json")
        keys = set(recipe.get("key", {}).values())
        if not any(key.endswith(f"machine_frame/{tag}") for key in keys):
            failures.append(f"the {machine} recipe does not name a machine frame")
        if "#c:plastics" not in keys:
            failures.append(f"the {machine} recipe does not name #c:plastics")
    ore = region(files, f"data/{MOD}/recipe/laser_drill_ore/")
    if len(ore) != ORE_RECIPES:
        failures.append(f"{len(ore)} laser_drill_ore recipes carried, expected {ORE_RECIPES}")
    for name in ore:
        recipe = files.json(name)
        if recipe.get("type") != f"{MOD}:laser_drill_ore":
            failures.append(f"{name} is not a {MOD}:laser_drill_ore recipe")
        if "neoforge:conditions" in recipe:
            failures.append(f"{name} still carries a NeoForge condition 26.3 does not know")
        output = recipe.get("output", {})
        if "item" in output:
            resolves(output["item"], f"{name} output")
        elif "tag" in output:
            tag = output["tag"]
            declared_tag = f"data/{namespace(tag)}/tags/item/{tag.split(':', 1)[1]}.json"
            if not files.has(declared_tag) or not files.json(declared_tag).get("values"):
                failures.append(f"{name} yields {tag}, which resolves to nothing")
        else:
            failures.append(f"{name} has no output")
        if not recipe.get("catalyst"):
            failures.append(f"{name} has no catalyst")
        for rarity in recipe.get("rarity", []):
            if "depth_min" not in rarity or "weight" not in rarity:
                failures.append(f"{name} has a rarity without a depth or a weight")
    for name in region(files, f"data/{MOD}/advancement/laser_drill_ore/"):
        if "neoforge:conditions" in files.json(name):
            failures.append(f"{name} still carries a NeoForge condition 26.3 does not know")

    # The fluid laser base (slice 8): declared, named, modelled, with a blockstate, a
    # 26.3 loot table and a shaped recipe naming the package's tags; and its own three
    # recipe files, each with a real output fluid, a catalyst and a depth band.
    for machine in FLUID_LASER:
        item_id = f"{MOD}:{machine}"
        if item_id not in blocks:
            failures.append(f"the {machine} is not declared as a block")
        for name in (
            f"assets/{MOD}/blockstates/{machine}.json",
            f"data/{MOD}/loot_table/blocks/{machine}.json",
            f"data/{MOD}/horizon/block/{machine}.json",
            f"assets/{MOD}/items/{machine}.json",
        ):
            if not files.has(name):
                failures.append(f"the {machine} has no {name}")
        loot = f"data/{MOD}/loot_table/blocks/{machine}.json"
        if files.has(loot):
            condition = files.json(loot).get("pools", [{}])[0].get("condition", {})
            if condition.get("type") != "minecraft:survives_explosion":
                failures.append(f"{loot} does not use the package's 26.3 condition shape")
        if item_id not in pickaxe_values(files):
            failures.append(f"the {machine} has requires_tool but no mineable/pickaxe entry")
        shaped = files.json(f"data/{MOD}/recipe/{machine}.json")
        keys = set(shaped.get("key", {}).values())
        for tag in ("#c:plastics", "#c:gears/diamond", f"#{MOD}:machine_frame/advanced"):
            if tag not in keys:
                failures.append(f"the {machine} recipe does not name {tag}")
    fluid_recipes = region(files, f"data/{MOD}/recipe/laser_drill_fluid/")
    if len(fluid_recipes) != FLUID_RECIPES:
        failures.append(
            f"{len(fluid_recipes)} laser_drill_fluid recipes carried, expected {FLUID_RECIPES}"
        )
    outputs = set()
    for name in fluid_recipes:
        recipe = files.json(name)
        if recipe.get("type") != f"{MOD}:laser_drill_fluid":
            failures.append(f"{name} is not a {MOD}:laser_drill_fluid recipe")
        if "neoforge:conditions" in recipe:
            failures.append(f"{name} still carries a NeoForge condition 26.3 does not know")
        output = recipe.get("output", {})
        fluid = output.get("fluid")
        if fluid not in FLUID_OUTPUTS:
            failures.append(f"{name} outputs {fluid!r}, which is not one of the jar's fluids")
        else:
            outputs.add(fluid)
        if not output.get("amount"):
            failures.append(f"{name} has no output amount")
        if not recipe.get("catalyst"):
            failures.append(f"{name} has no catalyst")
        for rarity in recipe.get("rarity", []):
            if "depth_min" not in rarity or "weight" not in rarity:
                failures.append(f"{name} has a rarity without a depth or a weight")
    if outputs != FLUID_OUTPUTS:
        failures.append(f"the fluid recipes' outputs are {sorted(outputs)}, expected all three")
    ether = files.json(f"data/{MOD}/recipe/laser_drill_fluid/ether.json")
    gate = ether.get("entity_data", {}).get("entity", {}).get("type")
    if gate != "minecraft:wither":
        failures.append(f"the ether recipe's entity gate is {gate!r}, not minecraft:wither")
    for name in region(files, f"data/{MOD}/advancement/laser_drill_fluid/"):
        if "neoforge:conditions" in files.json(name):
            failures.append(f"{name} still carries a NeoForge condition 26.3 does not know")

    # Every tag value resolves, including the ones this port writes for the
    # convention tags 26.3's base data does not ship.
    for name, values in tag_values.items():
        for value in values:
            resolves(value if isinstance(value, str) else value.get("id", ""), name)

    # The condensator is the jar's again: three tags, not the stand-ins.
    condensator = files.json(f"data/{MOD}/recipe/water_condensator.json")
    keys = set(condensator.get("key", {}).values())
    for tag in ("#c:plastics", "#c:gears/iron", f"#{MOD}:machine_frame/pity"):
        if tag not in keys:
            failures.append(f"the condensator recipe does not name {tag}")
    for stand_in in ("minecraft:slime_ball", "minecraft:iron_block"):
        if stand_in in keys:
            failures.append(f"the condensator recipe still uses the stand-in {stand_in}")

    # A frame drops nothing without a mineable entry: `requires_tool` is true.
    pickaxe = pickaxe_values(files)
    for frame in FRAMES:
        if f"{MOD}:{frame}" not in pickaxe:
            failures.append(f"{frame} has requires_tool but no mineable/pickaxe entry")
    return failures


def selftest() -> int:
    """Prove the checker fails: drop a tag file, restore a stand-in, drop a recipe."""
    class Mutated(Files):
        def __init__(self, path: Path, drop: str | None, edit: dict[str, object]):
            super().__init__(path)
            self.edit = edit
            if drop is not None:
                self.names = [name for name in self.names if name != drop]

        def json(self, name: str):
            data = super().json(name)
            edit = self.edit.get(name)
            return edit(data) if edit else data

    cases = [
        ("a missing convention tag", "data/c/tags/item/plastics.json", {}),
        ("a missing declaration", f"data/{MOD}/horizon/item/iron_gear.json", {}),
        ("the chamber's glass-pane tag", "data/c/tags/item/glass_panes.json", {}),
        ("the chamber's declaration", f"data/{MOD}/horizon/block/{CHAMBER}.json", {}),
        ("the latex chain's cobblestone tag", "data/c/tags/item/cobblestones.json", {}),
        ("an extractor recipe", f"data/{MOD}/recipe/fluid_extractor/oak.json", {}),
        ("the laser's raw-iron tag", "data/c/tags/item/raw_materials/iron.json", {}),
        ("a laser ore recipe", f"data/{MOD}/recipe/laser_drill_ore/coal_ore.json", {}),
        ("the fluid laser base's ether recipe",
         f"data/{MOD}/recipe/laser_drill_fluid/ether.json", {}),
        (
            "a fluid recipe's NeoForge condition",
            None,
            {
                f"data/{MOD}/recipe/laser_drill_fluid/lava.json":
                    lambda value: value
                    | {"neoforge:conditions": [{"type": "neoforge:mod_loaded"}]}
            },
        ),
        (
            "an ore advancement's condition",
            None,
            {
                f"data/{MOD}/advancement/laser_drill_ore/raw_materials/iron.json":
                    lambda value: value
                    | {"neoforge:conditions": [{"type": "neoforge:tag_empty"}]}
            },
        ),
        ("the slaughter factory's declaration",
         f"data/{MOD}/horizon/block/mob_slaughter_factory.json", {}),
        ("the mob crusher's declaration",
         f"data/{MOD}/horizon/block/mob_crusher.json", {}),
        (
            "the mob crusher's pattern",
            None,
            {
                f"data/{MOD}/recipe/mob_crusher.json":
                    lambda value: value | {"pattern": ["PPP", "BMB", "GRG"]}
            },
        ),
        (
            "the mob crusher's essence fluid",
            None,
            {
                f"data/{MOD}/recipe/mob_crusher.json":
                    lambda value: value | {"result": {"count": 1, "id": "industrialforegoing:essence"}}
            },
        ),
        ("the pink slime ingot tag", "data/c/tags/item/ingots/pink_slime.json", {}),
        (
            "the pink slime block's eight-for-one",
            None,
            {
                f"data/{MOD}/recipe/pink_slime.json":
                    lambda value: value
                    | {"result": {"count": 8, "id": f"{MOD}:pink_slime"}}
            },
        ),
        (
            "the pink slime ball's outputFluid",
            None,
            {
                f"data/{MOD}/recipe/dissolution_chamber/pink_slime_ball.json":
                    lambda value: {k: v for k, v in value.items() if k != "outputFluid"}
            },
        ),
        (
            "the pink slime block's friction",
            None,
            {
                f"data/{MOD}/horizon/block/pink_slime_block.json":
                    lambda value: value
                    | {"properties": {"friction": 0.6, "occludes": False}}
            },
        ),
    ]
    failures = 0
    for label, drop, edit in cases:
        found = check(Mutated(ROOT, drop, edit))
        if found:
            print(f"selftest: {label} -> {len(found)} failure(s), e.g. {found[0]}")
        else:
            print(f"selftest: {label} was NOT caught", file=sys.stderr)
            failures += 1

    # The condensator's stand-ins: restore slice 2's shape and expect a failure.
    def stand_ins(recipe):
        recipe["key"]["P"] = "minecraft:slime_ball"
        return recipe

    found = check(
        Mutated(ROOT, None, {f"data/{MOD}/recipe/water_condensator.json": stand_ins})
    )
    if found:
        print(f"selftest: the slice-2 stand-ins -> {len(found)} failure(s), e.g. {found[0]}")
    else:
        print("selftest: the slice-2 stand-ins were NOT caught", file=sys.stderr)
        failures += 1
    print("selftest: OK" if failures == 0 else f"selftest: {failures} case(s) not caught")
    return 0 if failures == 0 else 1


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", type=Path, help="the delivered .hmod (default: the port tree)")
    parser.add_argument("--selftest", action="store_true", help="mutate a copy and expect failures")
    args = parser.parse_args()
    if args.selftest:
        return selftest()
    files = Files(args.package.resolve() if args.package else ROOT)
    failures = check(files)
    print(f"checked {files.path}: {len(files.names)} content files")
    if failures:
        for failure in failures:
            print(f"FAIL: {failure}", file=sys.stderr)
        return 1
    print(f"CONTENT OK: every id, tag, texture, name and recipe of this package resolves")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
