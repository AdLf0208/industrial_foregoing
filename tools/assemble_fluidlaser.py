#!/usr/bin/env python3
"""Carry slice 8's fluid laser base files from the unpacked jar into the package.

The jar (`ports/_shared/carry-work/industrialforegoing/jar/`, unpacked from
`industrialforegoing-1.21-3.6.39.jar`) is the authority. The blockstate, both models
and the side texture are copied unchanged; the item definition, the 26.3-dialect
shaped recipe, the recipe advancement and the loot table are written the way the
package's other machines write them; the three `laser_drill_fluid` files and their
two unlock advancements are carried with their NeoForge conditions dropped (26.3 does
not know them, and a condition left in drops the file); and the block name and the
pickaxe entry are merged into the existing files.

    python3 tools/assemble_fluidlaser.py            # write the files
    python3 tools/assemble_fluidlaser.py --check    # exit 1 if the tree differs
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
JAR = ROOT.parent / "_shared/carry-work/industrialforegoing/jar"
MOD = "industrialforegoing"
BLOCK = "fluid_laser_base"

# Copied byte for byte. `laser_base_bottom.png` and the `base_block` parent are already
# in the package (slice 7), so only this machine's own side texture is new.
COPIES = [
    f"assets/{MOD}/blockstates/{BLOCK}.json",
    f"assets/{MOD}/models/block/{BLOCK}.json",
    f"assets/{MOD}/models/item/{BLOCK}.json",
    f"assets/{MOD}/textures/block/{BLOCK}_side.png",
]

# `FluidLaserBaseBlock.registerRecipe`: the jar's `pfp / bmb / grg`, re-spelled for 26.3
# (bare ids and single `#tags`; an ingredient object or a tag list is dropped whole).
SHAPED = {
    f"data/{MOD}/recipe/{BLOCK}.json": {
        "type": "minecraft:crafting_shaped",
        "category": "misc",
        "key": {
            "b": "minecraft:bucket",
            "f": "minecraft:diamond_pickaxe",
            "g": "#c:gears/diamond",
            "m": f"#{MOD}:machine_frame/advanced",
            "p": "#c:plastics",
            "r": "minecraft:redstone",
        },
        "pattern": ["pfp", "bmb", "grg"],
        "result": {"count": 1, "id": f"{MOD}:{BLOCK}"},
    },
}

# The three `laser_drill_fluid` files, and the two the jar gives an unlock advancement.
FLUID_RECIPES = ["ether.json", "lava.json", "oil.json"]
FLUID_ADVANCEMENTS = ["ether.json", "lava.json"]

# The block declaration: iron-block template upstream's `ofFullCopy(IRON_BLOCK)` gives a
# state-free block.
DECLARATIONS = {
    f"data/{MOD}/horizon/block/{BLOCK}.json": {
        "properties": {
            "explosion_resistance": 6.0,
            "hardness": 5.0,
            "requires_tool": True,
            "sounds": "minecraft:iron_block",
        },
        "template": "minecraft:iron_block",
    },
}

ITEM_DEFINITION = {
    f"assets/{MOD}/items/{BLOCK}.json": {
        "model": {"model": f"{MOD}:item/{BLOCK}", "type": "minecraft:model"}
    }
}

NAMES = {f"block.{MOD}.{BLOCK}": "Fluid Laser Base"}


def pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def loot_table() -> bytes:
    """The machine's self-drop in the package's 26.3 condition shape."""
    return pretty(
        {
            "type": "minecraft:block",
            "pools": [
                {
                    "bonus_rolls": 0.0,
                    "entries": [{"type": "minecraft:item", "name": f"{MOD}:{BLOCK}"}],
                    "rolls": 1.0,
                    "condition": {"type": "minecraft:survives_explosion"},
                }
            ],
            "random_sequence": f"{MOD}:blocks/{BLOCK}",
        }
    )


def conditionless(path: Path) -> bytes:
    """One carried recipe or advancement with its NeoForge condition dropped."""
    value = json.loads(path.read_text())
    value.pop("neoforge:conditions", None)
    return pretty(value)


def lang() -> bytes:
    path = ROOT / f"assets/{MOD}/lang/en_us.json"
    value = json.loads(path.read_text())
    value.update(NAMES)
    return pretty(value)


def pickaxe() -> bytes:
    path = ROOT / "data/minecraft/tags/block/mineable/pickaxe.json"
    value = json.loads(path.read_text())
    wanted = f"{MOD}:{BLOCK}"
    if wanted not in value["values"]:
        value["values"].append(wanted)
    value["values"].sort()
    return pretty(value)


def planned() -> dict[Path, bytes]:
    out: dict[Path, bytes] = {}
    for name in COPIES:
        out[ROOT / name] = (JAR / name).read_bytes()
    for name, shaped in SHAPED.items():
        out[ROOT / name] = pretty(shaped)
    out[ROOT / f"data/{MOD}/loot_table/blocks/{BLOCK}.json"] = loot_table()
    out[ROOT / f"data/{MOD}/advancement/recipes/misc/{BLOCK}.json"] = conditionless(
        JAR / f"data/{MOD}/advancement/recipes/misc/{BLOCK}.json"
    )
    for name in FLUID_RECIPES:
        out[ROOT / f"data/{MOD}/recipe/laser_drill_fluid/{name}"] = conditionless(
            JAR / f"data/{MOD}/recipe/laser_drill_fluid/{name}"
        )
    for name in FLUID_ADVANCEMENTS:
        out[ROOT / f"data/{MOD}/advancement/laser_drill_fluid/{name}"] = conditionless(
            JAR / f"data/{MOD}/advancement/laser_drill_fluid/{name}"
        )
    for name, shaped in DECLARATIONS.items():
        out[ROOT / name] = pretty(shaped)
    for name, definition in ITEM_DEFINITION.items():
        out[ROOT / name] = pretty(definition)
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
            f"checked {len(files)} fluid-laser files: "
            + ("identical" if differences == 0 else f"{differences} differ")
        )
        return 0 if differences == 0 else 1
    print(f"wrote {len(files)} fluid-laser files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
