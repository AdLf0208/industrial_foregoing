#!/usr/bin/env python3
"""Carry slice 7's laser files from the unpacked jar into the package.

The jar (`ports/_shared/carry-work/industrialforegoing/jar/`, unpacked from
`industrialforegoing-1.21-3.6.39.jar`) is the authority for every file here. The
script copies the assets, models and loot tables unchanged, re-spells the two
shaped recipes to 26.3's dialect (bare ingredient strings and `#tags`, no
`neoforge:conditions`), keeps the ten `laser_drill_ore` recipes with their
`neoforge:conditions` dropped (26.3 does not know them, and a condition left in
drops the file), writes the four `c:` tags the recipes and tag outputs need, and
merges the two block names and the two pickaxe entries.

    python3 tools/assemble_laser.py            # write the files
    python3 tools/assemble_laser.py --check    # exit 1 if the tree differs
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
JAR = ROOT.parent / "_shared/carry-work/industrialforegoing/jar"
MOD = "industrialforegoing"

DRILL = "laser_drill"
BASE = "ore_laser_base"

# Copied byte for byte: blockstates, models, item definitions and loot tables.
COPIES = [
    f"assets/{MOD}/blockstates/{BASE}.json",
    f"assets/{MOD}/blockstates/{DRILL}.json",
    f"assets/{MOD}/models/block/{BASE}.json",
    f"assets/{MOD}/models/block/{DRILL}.json",
    f"assets/{MOD}/models/item/{BASE}.json",
    f"assets/{MOD}/models/item/{DRILL}.json",
    f"assets/{MOD}/textures/block/laser_base_side.png",
    f"assets/{MOD}/textures/block/laser_base_bottom.png",
    f"assets/{MOD}/textures/block/laser_base_top.png",
    f"assets/{MOD}/textures/block/laser_drill_front.png",
    f"assets/{MOD}/textures/block/laser_drill_top.png",
    f"assets/{MOD}/textures/block/laser_drill_bottom.png",
    f"assets/{MOD}/textures/block/laser_drill_back.png",
    f"assets/{MOD}/textures/block/laser_drill_back.png.mcmeta",
]

# The 10 of the jar's 42 ore recipes whose output this package (or vanilla) has. The other
# 32 name other mods' ores (cinnabar, sodalite, fluorite, …) or raw-material tags of metals
# no package here ships.
ORE_RECIPES = [
    "ancient_debris.json",
    "coal_ore.json",
    "diamond_ore.json",
    "emerald_ore.json",
    "glowstone.json",
    "lapis_ore.json",
    "nether_quartz_ore.json",
    "redstone_ore.json",
    "raw_materials/iron.json",
    "raw_materials/gold.json",
]

# The two machines' shaped recipes, re-spelled: 26.3's loader drops a recipe whose ingredient
# is an object or a list naming an absent tag (slice 2's measured defect), so an ingredient is
# a bare id or one `#tag`.
SHAPED = {
    f"data/{MOD}/recipe/{BASE}.json": {
        "type": "minecraft:crafting_shaped",
        "category": "misc",
        "key": {
            "b": "#c:ores/iron",
            "f": "minecraft:diamond_pickaxe",
            "g": "#c:gears/diamond",
            "m": f"#{MOD}:machine_frame/advanced",
            "p": "#c:plastics",
            "r": "#c:dusts/redstone",
        },
        "pattern": ["pfp", "bmb", "grg"],
        "result": {"count": 1, "id": f"{MOD}:{BASE}"},
    },
    f"data/{MOD}/recipe/{DRILL}.json": {
        "type": "minecraft:crafting_shaped",
        "category": "misc",
        "key": {
            "b": "minecraft:piston",
            "f": "#c:gears/diamond",
            "g": "#c:gears/gold",
            "m": f"#{MOD}:machine_frame/simple",
            "p": "#c:plastics",
            "r": "minecraft:redstone",
        },
        "pattern": ["pfp", "bmb", "grg"],
        "result": {"count": 1, "id": f"{MOD}:{DRILL}"},
    },
}

# The convention tags the two recipes and the ore recipes' tag outputs name. 26.3's base data
# ships no `c:` item tags at all (measured in slice 4), so this package declares the values.
TAGS = {
    "data/c/tags/item/ores/iron.json": [
        "minecraft:iron_ore",
        "minecraft:deepslate_iron_ore",
    ],
    "data/c/tags/item/dusts/redstone.json": ["minecraft:redstone"],
    "data/c/tags/item/raw_materials/iron.json": ["minecraft:raw_iron"],
    "data/c/tags/item/raw_materials/gold.json": ["minecraft:raw_gold"],
}

# The block declarations: the iron-block template upstream's `ofFullCopy(IRON_BLOCK)` gives the
# ore base (state-free) and the drill (`SIX_WAY` on `facing`).
DECLARATIONS = {
    f"data/{MOD}/horizon/block/{BASE}.json": {
        "properties": {
            "explosion_resistance": 6.0,
            "hardness": 5.0,
            "requires_tool": True,
            "sounds": "minecraft:iron_block",
        },
        "template": "minecraft:iron_block",
    },
    f"data/{MOD}/horizon/block/{DRILL}.json": {
        "default_state": {"facing": "north"},
        "properties": {
            "explosion_resistance": 6.0,
            "hardness": 5.0,
            "requires_tool": True,
            "sounds": "minecraft:iron_block",
        },
        "state_properties": {
            "facing": {
                "type": "enum",
                "values": ["north", "south", "east", "west", "up", "down"],
            }
        },
        "template": "minecraft:iron_block",
    },
}

# 26.3's item definition: the item is the block's model.
ITEM_DEFINITIONS = {
    f"assets/{MOD}/items/{id}.json": {
        "model": {"model": f"{MOD}:item/{id}", "type": "minecraft:model"}
    }
    for id in (BASE, DRILL)
}

# `BlockBehaviour.Properties` names, as the jar's lang file writes them.
NAMES = {
    f"block.{MOD}.{BASE}": "Ore Laser Base",
    f"block.{MOD}.{DRILL}": "Laser Drill",
}


def loot_table(name: str) -> bytes:
    """A machine's self-drop, in the shape the package's other eight blocks use.

    The jar's pool carries `"conditions": [{"condition": "minecraft:survives_explosion"}]`.
    The engine reads a condition object's `type` (26.3's shape), so that object's kind is
    empty and the pool never drops — the block vanished when broken, measured 2026-11-22. The
    eight machines carried before this slice use a pool-level `"condition": {"type": …}`,
    which the engine does not read, i.e. an unconditional self-drop; this slice writes the
    same shape for consistency.
    """
    return pretty(
        {
            "type": "minecraft:block",
            "pools": [
                {
                    "bonus_rolls": 0.0,
                    "entries": [
                        {"type": "minecraft:item", "name": f"{MOD}:{name}"}
                    ],
                    "rolls": 1.0,
                    "condition": {"type": "minecraft:survives_explosion"},
                }
            ],
            "random_sequence": f"{MOD}:blocks/{name}",
        }
    )


def pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def ore_recipe(name: str) -> bytes:
    """One `laser_drill_ore` file with its NeoForge condition dropped."""
    path = JAR / f"data/{MOD}/recipe/laser_drill_ore/{name}"
    value = json.loads(path.read_text())
    value.pop("neoforge:conditions", None)
    return pretty(value)


def recipe_advancement(name: str) -> bytes:
    """A machine's recipe advancement with the item-exists condition dropped."""
    path = JAR / f"data/{MOD}/advancement/recipes/misc/{name}"
    value = json.loads(path.read_text())
    value.pop("neoforge:conditions", None)
    return pretty(value)


def ore_advancement(name: str) -> bytes:
    """A `laser_drill_ore` recipe's unlock advancement, its NeoForge condition dropped.

    The two raw-material files carry a `neoforge:tag_empty` gate; 26.3 does not know it and
    leaves the whole advancement out (measured: `unknown load conditions [neoforge:tag_empty]`).
    """
    path = JAR / f"data/{MOD}/advancement/laser_drill_ore/{name}"
    value = json.loads(path.read_text())
    value.pop("neoforge:conditions", None)
    return pretty(value)


def lang() -> bytes:
    """The package's English names with the two laser blocks merged in, sorted."""
    path = ROOT / f"assets/{MOD}/lang/en_us.json"
    value = json.loads(path.read_text())
    value.update(NAMES)
    return pretty(value)


def pickaxe() -> bytes:
    """`minecraft:mineable/pickaxe` with both machines in it."""
    path = ROOT / "data/minecraft/tags/block/mineable/pickaxe.json"
    value = json.loads(path.read_text())
    for id in (BASE, DRILL):
        wanted = f"{MOD}:{id}"
        if wanted not in value["values"]:
            value["values"].append(wanted)
    value["values"].sort()
    return pretty(value)


def planned() -> dict[Path, bytes]:
    """Every file this slice writes, by path."""
    out: dict[Path, bytes] = {}
    for name in COPIES:
        out[ROOT / name] = (JAR / name).read_bytes()
    for name, shaped in SHAPED.items():
        out[ROOT / name] = pretty(shaped)
    for name in (BASE, DRILL):
        out[ROOT / f"data/{MOD}/loot_table/blocks/{name}.json"] = loot_table(name)
    for name in ORE_RECIPES:
        out[ROOT / f"data/{MOD}/recipe/laser_drill_ore/{name}"] = ore_recipe(name)
    for name in (BASE, DRILL):
        out[ROOT / f"data/{MOD}/advancement/recipes/misc/{name}.json"] = (
            recipe_advancement(f"{name}.json")
        )
    for name in ORE_RECIPES:
        out[ROOT / f"data/{MOD}/advancement/laser_drill_ore/{name}"] = ore_advancement(
            name
        )
    for name, shaped in DECLARATIONS.items():
        out[ROOT / name] = pretty(shaped)
    for name, definition in ITEM_DEFINITIONS.items():
        out[ROOT / name] = pretty(definition)
    for name, values in TAGS.items():
        out[ROOT / name] = pretty({"values": values})
    out[ROOT / f"assets/{MOD}/lang/en_us.json"] = lang()
    out[ROOT / "data/minecraft/tags/block/mineable/pickaxe.json"] = pickaxe()
    return out


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="exit 1 if the tree differs")
    args = parser.parse_args()
    if not JAR.is_dir():
        print(f"missing the unpacked jar: {JAR}", file=sys.stderr)
        return 2
    files = planned()
    differences = 0
    for path, content in files.items():
        if args.check:
            if not path.is_file() or path.read_bytes() != content:
                print(f"differs: {path.relative_to(ROOT)}")
                differences += 1
            continue
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
    if args.check:
        print(
            f"checked {len(files)} laser files: "
            + ("identical" if differences == 0 else f"{differences} differ")
        )
        return 0 if differences == 0 else 1
    print(f"wrote {len(files)} laser files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
