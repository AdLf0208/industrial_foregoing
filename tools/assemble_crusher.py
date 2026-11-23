#!/usr/bin/env python3
"""Carry slice 11's **Mob Crusher** from the unpacked jar.

The jar (`ports/_shared/carry-work/industrialforegoing/jar/`, unpacked from
`industrialforegoing-1.21-3.6.39.jar`) is the authority. The machine's blockstate, block model,
item model and its one texture (the back face, animated, so its `.mcmeta` travels with it) are
copied unchanged; the item definition, the block declaration, the loot table, the shaped recipe,
its unlock and the name are written in the package's 26.3 dialect.

    python3 tools/assemble_crusher.py            # write the files
    python3 tools/assemble_crusher.py --check    # exit 1 if the tree differs
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
JAR = ROOT.parent / "_shared/carry-work/industrialforegoing/jar"
MOD = "industrialforegoing"
BLOCK = "mob_crusher"

# Copied byte for byte: the machine's own state file, both models and the back face, which is
# animated so its `.mcmeta` travels with it.
COPIES = [
    f"assets/{MOD}/blockstates/{BLOCK}.json",
    f"assets/{MOD}/models/block/{BLOCK}.json",
    f"assets/{MOD}/models/item/{BLOCK}.json",
    f"assets/{MOD}/textures/block/mob_relocator_back.png",
    f"assets/{MOD}/textures/block/mob_relocator_back.png.mcmeta",
]

# The two planks of the jar's data this slice carries with the 26.3 upgrade: the machine's shaped
# recipe and its unlock.
CARRIED_DATA = [
    f"data/{MOD}/recipe/{BLOCK}.json",
    f"data/{MOD}/advancement/recipes/misc/{BLOCK}.json",
]

# `new MobCrusherBlock()`: `Properties.ofFullCopy(IRON_BLOCK)` and Titanium's `FOUR_WAY`
# (`subfacing`, four values) — the same declaration the other iron-block machines carry.
MACHINE_BLOCK = {
    "template": "minecraft:iron_block",
    "properties": {
        "hardness": 5.0,
        "explosion_resistance": 6.0,
        "requires_tool": True,
        "sounds": "minecraft:iron_block",
    },
    "state_properties": {
        "subfacing": {"type": "enum", "values": ["north", "south", "east", "west"]},
    },
    "default_state": {"subfacing": "north"},
}

# The names: the jar's own strings, read below (never retyped).
NAMES = [
    f"block.{MOD}.{BLOCK}",
]


def pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def upgrade_ingredient(value: object) -> object:
    """One 1.21.1 ingredient in 26.3's spelling: `{"item": I}`/`{"tag": T}` -> `I`/`#T`."""
    if isinstance(value, list):
        # A list of alternatives is the shape 26.3 drops whole; no recipe of this slice has one.
        raise SystemExit(f"a recipe of this slice uses an alternative list: {value!r}")
    if isinstance(value, dict) and "tag" in value:
        return f"#{value['tag']}"
    if isinstance(value, dict) and "item" in value:
        return value["item"]
    return value


def upgraded_recipe(path: Path) -> bytes:
    """The jar's crafting recipe with NeoForge's condition dropped and its keys re-spelled."""
    value = json.loads(path.read_text())
    value.pop("neoforge:conditions", None)
    for key, ingredient in list(value.get("key", {}).items()):
        value["key"][key] = upgrade_ingredient(ingredient)
    if "ingredients" in value:
        value["ingredients"] = [upgrade_ingredient(cell) for cell in value["ingredients"]]
    return pretty(value)


def conditionless(path: Path) -> bytes:
    value = json.loads(path.read_text())
    value.pop("neoforge:conditions", None)
    return pretty(value)


def loot_table(name: str) -> bytes:
    """A block's self-drop in the package's 26.3 condition shape (a pool-level `type`)."""
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
    """One of the package's existing JSON files with `values` appended (sorted, no repeats)."""
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
        out[ROOT / name] = (
            upgraded_recipe(path) if "/recipe/" in name else conditionless(path)
        )
    # The machine: declaration, item definition, loot, name.
    out[ROOT / f"data/{MOD}/horizon/block/{BLOCK}.json"] = pretty(MACHINE_BLOCK)
    out[ROOT / f"assets/{MOD}/items/{BLOCK}.json"] = pretty(
        {"model": {"type": "minecraft:model", "model": f"{MOD}:item/{BLOCK}"}}
    )
    out[ROOT / f"data/{MOD}/loot_table/blocks/{BLOCK}.json"] = loot_table(BLOCK)
    jar_lang = json.loads((JAR / f"assets/{MOD}/lang/en_us.json").read_text())
    lang_path = ROOT / f"assets/{MOD}/lang/en_us.json"
    lang = json.loads(lang_path.read_text())
    lang.update({key: jar_lang[key] for key in NAMES})
    out[lang_path] = pretty(lang)
    # The machine joins the same Agriculture & Husbandry tab the slaughter factory opened.
    out[ROOT / f"data/{MOD}/horizon/creative_tab/ag_hus.json"] = merged(
        ROOT / f"data/{MOD}/horizon/creative_tab/ag_hus.json", [f"{MOD}:{BLOCK}"], "items"
    )
    # `Properties.ofFullCopy(IRON_BLOCK)` is `requires_tool`: the jar's own pickaxe entry.
    out[ROOT / "data/minecraft/tags/block/mineable/pickaxe.json"] = merged(
        ROOT / "data/minecraft/tags/block/mineable/pickaxe.json", [f"{MOD}:{BLOCK}"]
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
            f"checked {len(files)} crusher files: "
            + ("identical" if differences == 0 else f"{differences} differ")
        )
        return 0 if differences == 0 else 1
    print(f"wrote {len(files)} crusher files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
