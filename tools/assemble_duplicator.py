#!/usr/bin/env python3
"""Carry slice 12's **Mob Duplicator** and **Mob Imprisonment Tool** from the unpacked jar.

The jar (`ports/_shared/carry-work/industrialforegoing/jar/`, unpacked from
`industrialforegoing-1.21-3.6.39.jar`) is the authority. The machine's blockstate, block model, item
model and its two textures are copied unchanged; the tool's four textures and its layered item model
are copied too. The item definitions, block declaration, loot table, the two shaped recipes, their
unlocks, the names, the two empty blacklist tags and the creative-tab entries are written in the
package's 26.3 dialect.

    python3 tools/assemble_duplicator.py            # write the files
    python3 tools/assemble_duplicator.py --check    # exit 1 if the tree differs
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
JAR = ROOT.parent / "_shared/carry-work/industrialforegoing/jar"
MOD = "industrialforegoing"
BLOCK = "mob_duplicator"
TOOL = "mob_imprisonment_tool"

# Copied byte for byte: the machine's state file, both models and its two faces; the tool's
# layered model and its four layers.
COPIES = [
    f"assets/{MOD}/blockstates/{BLOCK}.json",
    f"assets/{MOD}/models/block/{BLOCK}.json",
    f"assets/{MOD}/models/item/{BLOCK}.json",
    f"assets/{MOD}/textures/block/mob_duplicator_side.png",
    f"assets/{MOD}/textures/block/mob_duplicator_top.png",
    f"assets/{MOD}/textures/item/mob_imprisonment_tool_color1.png",
    f"assets/{MOD}/textures/item/mob_imprisonment_tool_color2.png",
    f"assets/{MOD}/textures/item/mob_imprisonment_tool_dot.png",
    f"assets/{MOD}/textures/item/mob_imprisonment_tool_overlay.png",
]

# The jar's data this slice carries with the 26.3 upgrade: the two shaped recipes and their unlocks.
CARRIED_DATA = [
    f"data/{MOD}/recipe/{BLOCK}.json",
    f"data/{MOD}/recipe/{TOOL}.json",
    f"data/{MOD}/advancement/recipes/misc/{BLOCK}.json",
    f"data/{MOD}/advancement/recipes/misc/{TOOL}.json",
]

# `new MobDuplicatorBlock()`: `Properties.ofFullCopy(IRON_BLOCK)` and **no** RotationType
# (`MobDuplicatorBlock` does not override `getRotationType()`), so the blockstate's only variant is
# `""` and the declaration has no state property.
MACHINE_BLOCK = {
    "template": "minecraft:iron_block",
    "properties": {
        "hardness": 5.0,
        "explosion_resistance": 6.0,
        "requires_tool": True,
        "sounds": "minecraft:iron_block",
    },
}

# `new MobImprisonmentToolItem(...)`: `new Properties().stacksTo(1)`, no block, no default component.
TOOL_ITEM = {
    "template": "minecraft:paper",
    "max_stack_size": 1,
}

# The names: the jar's own strings, read below (never retyped).
NAMES = [
    f"block.{MOD}.{BLOCK}",
    f"item.{MOD}.{TOOL}",
    f"text.industrialforegoing.display.mob",
    f"text.industrialforegoing.display.health",
]


def pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def upgrade_ingredient(value: object) -> object:
    """One 1.21.1 ingredient in 26.3's spelling: `{"item": I}`/`{"tag": T}` -> `I`/`#T`."""
    if isinstance(value, list):
        # A list of alternatives is the shape 26.3 drops whole; neither recipe has one.
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
    # The machine: declaration, item definition, loot, name; the tool: item definition and model.
    out[ROOT / f"data/{MOD}/horizon/block/{BLOCK}.json"] = pretty(MACHINE_BLOCK)
    out[ROOT / f"assets/{MOD}/items/{BLOCK}.json"] = pretty(
        {"model": {"type": "minecraft:model", "model": f"{MOD}:item/{BLOCK}"}}
    )
    out[ROOT / f"data/{MOD}/horizon/item/{TOOL}.json"] = pretty(TOOL_ITEM)
    out[ROOT / f"assets/{MOD}/items/{TOOL}.json"] = pretty(
        {"model": {"type": "minecraft:model", "model": f"{MOD}:item/{TOOL}"}}
    )
    # The jar parent (`industrialforegoing:item/item`) is one of its own model files, not carried;
    # the four layers are vanilla's generated-item layers, so the 26.3 model names that parent.
    tool_model = json.loads((JAR / f"assets/{MOD}/models/item/{TOOL}.json").read_text())
    tool_model["parent"] = "minecraft:item/generated"
    out[ROOT / f"assets/{MOD}/models/item/{TOOL}.json"] = pretty(tool_model)
    out[ROOT / f"data/{MOD}/loot_table/blocks/{BLOCK}.json"] = loot_table(BLOCK)
    # The two blacklists are the jar's own empty entity-type tags; a pack may add to either.
    for tag in ("mob_duplicator_blacklist", "mob_imprisonment_tool_blacklist"):
        source = JAR / f"data/{MOD}/tags/entity_type/{tag}.json"
        out[ROOT / f"data/{MOD}/tags/entity_type/{tag}.json"] = pretty(json.loads(source.read_text()))
    jar_lang = json.loads((JAR / f"assets/{MOD}/lang/en_us.json").read_text())
    lang_path = ROOT / f"assets/{MOD}/lang/en_us.json"
    lang = json.loads(lang_path.read_text())
    lang.update({key: jar_lang[key] for key in NAMES})
    out[lang_path] = pretty(lang)
    # Both join the same Agriculture & Husbandry tab the crusher opened.
    out[ROOT / f"data/{MOD}/horizon/creative_tab/ag_hus.json"] = merged(
        ROOT / f"data/{MOD}/horizon/creative_tab/ag_hus.json",
        [f"{MOD}:{BLOCK}", f"{MOD}:{TOOL}"],
        "items",
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
            f"checked {len(files)} duplicator files: "
            + ("identical" if differences == 0 else f"{differences} differ")
        )
        return 0 if differences == 0 else 1
    print(f"wrote {len(files)} duplicator files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
