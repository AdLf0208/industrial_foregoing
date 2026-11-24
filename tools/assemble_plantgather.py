#!/usr/bin/env python3
"""Carry slice 17's **Plant Gatherer** (`industrialforegoing:plant_gatherer`) from the jar.

The unpacked jar (`ports/_shared/carry-work/industrialforegoing/jar/`) is the authority: its
blockstate, both models, its three textures and its shaped recipe/advancement are carried. The 26.3
block declaration, item definition, loot table, creative-tab and pickaxe entries are written in the
package's dialect, exactly as the other machines' are.

    python3 tools/assemble_plantgather.py            # write the files
    python3 tools/assemble_plantgather.py --check    # exit 1 if the tree differs
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
JAR = ROOT.parent / "_shared/carry-work/industrialforegoing/jar"
MOD = "industrialforegoing"
BLOCK = "plant_gatherer"

COPIES = [
    f"assets/{MOD}/blockstates/{BLOCK}.json",
    f"assets/{MOD}/models/block/{BLOCK}.json",
    f"assets/{MOD}/models/item/{BLOCK}.json",
    # The block model's own textures, shared with the jar's crop recollector art.
    "assets/industrialforegoing/textures/block/crop_recolector_back.png",
    "assets/industrialforegoing/textures/block/crop_recolector_back.png.mcmeta",
    "assets/industrialforegoing/textures/block/crop_recolector_side.png",
]

CARRIED_DATA = [
    f"data/{MOD}/recipe/{BLOCK}.json",
    f"data/{MOD}/advancement/recipes/misc/{BLOCK}.json",
]

# `PlantGathererBlock` at `ce2852db`: `Properties.ofFullCopy(IRON_BLOCK)` and
# `RotationType.FOUR_WAY` (the jar's four `subfacing` variants, carried unchanged).
MACHINE_BLOCK = {
    "template": "minecraft:iron_block",
    "properties": {
        "hardness": 5.0,
        "explosion_resistance": 6.0,
        "requires_tool": True,
        "sounds": "minecraft:iron_block",
    },
    "default_state": {"subfacing": "north"},
    "state_properties": {
        "subfacing": {"type": "enum", "values": ["north", "south", "east", "west"]}
    },
}

NAMES = [f"block.{MOD}.{BLOCK}"]


def pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def upgrade_ingredient(value: object) -> object:
    if isinstance(value, list):
        raise SystemExit(f"an ingredient list this slice cannot carry: {value!r}")
    if isinstance(value, dict) and "tag" in value:
        return f"#{value['tag']}"
    if isinstance(value, dict) and "item" in value:
        return value["item"]
    return value


def upgraded_recipe(path: Path) -> bytes:
    value = json.loads(path.read_text())
    value.pop("neoforge:conditions", None)
    for key, ingredient in list(value.get("key", {}).items()):
        value["key"][key] = upgrade_ingredient(ingredient)
    return pretty(value)


def conditionless(path: Path) -> bytes:
    value = json.loads(path.read_text())
    value.pop("neoforge:conditions", None)
    return pretty(value)


def loot_table(name: str) -> bytes:
    return pretty(
        {
            "type": "minecraft:block",
            "pools": [
                {
                    "bonus_rolls": 0.0,
                    "entries": [{"type": "minecraft:item", "name": f"{MOD}:{name}"}],
                    "rolls": 1.0,
                    "condition": {"type": "minecraft:survives_explosion"},
                }
            ],
            "random_sequence": f"{MOD}:blocks/{name}",
        }
    )


def merged(path: Path, values: list[str], key: str = "values") -> bytes:
    value = json.loads(path.read_text())
    for wanted in values:
        if wanted not in value[key]:
            value[key].append(wanted)
    value[key].sort()
    return pretty(value)


def planned() -> dict[Path, bytes]:
    out: dict[Path, bytes] = {}
    for name in COPIES:
        out[ROOT / name] = (JAR / name).read_bytes()
    for name in CARRIED_DATA:
        path = JAR / name
        out[ROOT / name] = upgraded_recipe(path) if "/recipe/" in name else conditionless(path)

    out[ROOT / f"data/{MOD}/horizon/block/{BLOCK}.json"] = pretty(MACHINE_BLOCK)
    out[ROOT / f"assets/{MOD}/items/{BLOCK}.json"] = pretty(
        {"model": {"type": "minecraft:model", "model": f"{MOD}:item/{BLOCK}"}}
    )
    out[ROOT / f"data/{MOD}/loot_table/blocks/{BLOCK}.json"] = loot_table(BLOCK)

    # The jar's own English name, read rather than retyped.
    jar_lang = json.loads((JAR / f"assets/{MOD}/lang/en_us.json").read_text())
    lang_path = ROOT / f"assets/{MOD}/lang/en_us.json"
    lang = json.loads(lang_path.read_text())
    lang.update({key: jar_lang[key] for key in NAMES})
    out[lang_path] = pretty(lang)

    # The machine joins the Agriculture & Husbandry tab and is pickaxe-mineable (`requires_tool`).
    out[ROOT / f"data/{MOD}/horizon/creative_tab/ag_hus.json"] = merged(
        ROOT / f"data/{MOD}/horizon/creative_tab/ag_hus.json",
        [f"{MOD}:{BLOCK}"],
        "items",
    )
    out[ROOT / "data/minecraft/tags/block/mineable/pickaxe.json"] = merged(
        ROOT / "data/minecraft/tags/block/mineable/pickaxe.json",
        [f"{MOD}:{BLOCK}"],
    )
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
            f"checked {len(files)} plant-gatherer files: "
            + ("identical" if differences == 0 else f"{differences} differ")
        )
        return 0 if differences == 0 else 1
    print(f"wrote {len(files)} plant-gatherer files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
