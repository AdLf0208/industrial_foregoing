#!/usr/bin/env python3
"""Carry slice 15's **sewage chain** (`sewer`, `sewage_composter`, `fertilizer`) from the jar.

The unpacked jar (`ports/_shared/carry-work/industrialforegoing/jar/`) is the authority. The two
machines' four-variant blockstates, their block and item models and their textures are copied byte
for byte; their recipes, unlocks, the `c:fertilizers` tag, their 26.3 declarations, item
definitions, loot tables, names and creative-tab entries are written in the package's dialect.

    python3 tools/assemble_sewage.py            # write the files
    python3 tools/assemble_sewage.py --check    # exit 1 if the tree differs
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
JAR = ROOT.parent / "_shared/carry-work/industrialforegoing/jar"
MOD = "industrialforegoing"
BLOCKS = ("sewer", "sewage_composter")
ITEMS = ("fertilizer",)

COPIES = []
for block in BLOCKS:
    COPIES += [
        f"assets/{MOD}/blockstates/{block}.json",
        f"assets/{MOD}/models/block/{block}.json",
        f"assets/{MOD}/models/item/{block}.json",
    ]
COPIES += [
    f"assets/{MOD}/textures/block/sewer_side.png",
    f"assets/{MOD}/textures/block/sewer_top.png",
    f"assets/{MOD}/textures/block/sewage_composter_side.png",
    f"assets/{MOD}/textures/item/fertilizer.png",
]

CARRIED_DATA = []
for block in BLOCKS:
    CARRIED_DATA += [
        f"data/{MOD}/recipe/{block}.json",
        f"data/{MOD}/advancement/recipes/misc/{block}.json",
    ]

# Both machines: `Properties.ofFullCopy(IRON_BLOCK)` (checked in their block classes at ce2852db)
# and Titanium's `RotationType.FOUR_WAY`.
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

NAMES = [
    f"block.{MOD}.sewer",
    f"block.{MOD}.sewage_composter",
    f"item.{MOD}.fertilizer",
    f"item.{MOD}.sewage_bucket",
    f"fluid_type.{MOD}.sewage",
    f"block.sewage",
]


def pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def upgrade_ingredient(value: object) -> object:
    if isinstance(value, list):
        # 26.3 drops a recipe with an alternative list whole; neither machine recipe has one.
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

    for block in BLOCKS:
        out[ROOT / f"data/{MOD}/horizon/block/{block}.json"] = pretty(MACHINE_BLOCK)
        out[ROOT / f"assets/{MOD}/items/{block}.json"] = pretty(
            {"model": {"type": "minecraft:model", "model": f"{MOD}:item/{block}"}}
        )
        out[ROOT / f"data/{MOD}/loot_table/blocks/{block}.json"] = loot_table(block)

    # The fertilizer's item definition and model. The jar's model parents to `industrialforegoing:item/item`,
    # a base model this package does not carry; like every other plain item here it uses
    # `minecraft:item/generated` with the same display transforms as the port's dry rubber.
    model = json.loads((ROOT / f"assets/{MOD}/models/item/dryrubber.json").read_text())
    model["textures"] = {"layer0": f"{MOD}:item/fertilizer"}
    out[ROOT / f"data/{MOD}/horizon/item/fertilizer.json"] = pretty({"template": "minecraft:paper"})
    out[ROOT / f"assets/{MOD}/items/fertilizer.json"] = pretty(
        {"model": {"type": "minecraft:model", "model": f"{MOD}:item/fertilizer"}}
    )
    out[ROOT / f"assets/{MOD}/models/item/fertilizer.json"] = pretty(model)

    # `data/c/tags/item/fertilizers.json`: the jar's two values in the package's tag dialect.
    jar_tag = json.loads((JAR / "data/c/tags/item/fertilizers.json").read_text())
    out[ROOT / "data/c/tags/item/fertilizers.json"] = pretty({"values": sorted(jar_tag["values"])})

    # The jar's own English strings, read rather than retyped.
    jar_lang = json.loads((JAR / f"assets/{MOD}/lang/en_us.json").read_text())
    lang_path = ROOT / f"assets/{MOD}/lang/en_us.json"
    lang = json.loads(lang_path.read_text())
    lang.update({key: jar_lang[key] for key in NAMES})
    out[lang_path] = pretty(lang)

    # Both machines and the fertilizer join the Agriculture & Husbandry tab.
    out[ROOT / f"data/{MOD}/horizon/creative_tab/ag_hus.json"] = merged(
        ROOT / f"data/{MOD}/horizon/creative_tab/ag_hus.json",
        [f"{MOD}:sewer", f"{MOD}:sewage_composter", f"{MOD}:fertilizer"],
        "items",
    )
    # `Properties.ofFullCopy(IRON_BLOCK)` is `requires_tool`: the jar's own pickaxe entries.
    out[ROOT / "data/minecraft/tags/block/mineable/pickaxe.json"] = merged(
        ROOT / "data/minecraft/tags/block/mineable/pickaxe.json",
        [f"{MOD}:{block}" for block in BLOCKS],
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
            f"checked {len(files)} sewage-chain files: "
            + ("identical" if differences == 0 else f"{differences} differ")
        )
        return 0 if differences == 0 else 1
    print(f"wrote {len(files)} sewage-chain files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
