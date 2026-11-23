#!/usr/bin/env python3
"""Carry slice 9's **Mob Slaughter Factory** and **pink slime set** from the unpacked jar.

The jar (`ports/_shared/carry-work/industrialforegoing/jar/`, unpacked from
`industrialforegoing-1.21-3.6.39.jar`) is the authority. The machine's blockstate, block
model, item model and its two textures are copied unchanged; the pink slime block and the two
pink slime items carry their own model and texture (the ingot's animation `.mcmeta` with it);
the item definitions, the block declarations, the loot tables, the shaped and shapeless recipes,
the two dissolution-chamber recipes and the names are written in the package's 26.3 dialect.

    python3 tools/assemble_slaughter.py            # write the files
    python3 tools/assemble_slaughter.py --check    # exit 1 if the tree differs
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
JAR = ROOT.parent / "_shared/carry-work/industrialforegoing/jar"
MOD = "industrialforegoing"
BLOCK = "mob_slaughter_factory"

# Copied byte for byte: the machine's own state file, both models, the two block faces (the
# back one is animated, so its `.mcmeta` travels with it), the pink slime block's state, models
# and texture, the two items' textures (the ingot's animated), and the plain-item parent model.
COPIES = [
    f"assets/{MOD}/blockstates/{BLOCK}.json",
    f"assets/{MOD}/models/block/{BLOCK}.json",
    f"assets/{MOD}/models/item/{BLOCK}.json",
    f"assets/{MOD}/textures/block/{BLOCK}_back.png",
    f"assets/{MOD}/textures/block/{BLOCK}_back.png.mcmeta",
    f"assets/{MOD}/textures/block/{BLOCK}_side.png",
    f"assets/{MOD}/blockstates/pink_slime_block.json",
    f"assets/{MOD}/models/block/pink_slime_block.json",
    f"assets/{MOD}/models/item/pink_slime_block.json",
    f"assets/{MOD}/textures/block/pink_slime_block.png",
    f"assets/{MOD}/textures/item/pink_slime.png",
    f"assets/{MOD}/textures/item/pink_slime_ingot.png",
    f"assets/{MOD}/textures/item/pink_slime_ingot.png.mcmeta",
]

# The two plain `RecipelessCustomItem`s and the block item: 26.3's item definition naming the
# model this package ships.
ITEMS = ["pink_slime", "pink_slime_ingot", "pink_slime_block"]

# The two planks of the jar's data this slice carries with the 26.3 upgrade: the machine's shaped
# recipe and unlock, the pink slime block's shaped recipe and the block-to-nine shapeless one.
CARRIED_DATA = [
    f"data/{MOD}/recipe/{BLOCK}.json",
    f"data/{MOD}/advancement/recipes/misc/{BLOCK}.json",
    f"data/{MOD}/recipe/pink_slime_block.json",
    f"data/{MOD}/advancement/recipes/misc/pink_slime_block.json",
    f"data/{MOD}/recipe/pink_slime.json",
    f"data/{MOD}/advancement/recipes/misc/pink_slime.json",
]

# The dissolution chamber's two pink-slime recipes: the jar's only `outputFluid` one and the ingot.
CHAMBER_RECIPES = ["pink_slime_ball", "pink_slime_ingot"]

# `new SlaughterFactoryBlock()`: `Properties.ofFullCopy(IRON_BLOCK)` and Titanium's `FOUR_WAY`
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

# `new PinkSlimeBlock(Properties.of().mapColor(GRASS).friction(0.8F).sound(SLIME_BLOCK)
# .noOcclusion())` — a `SlimeBlock` subclass. The template gives the hardness, the sound group,
# the friction and the missing occlusion; `friction` is declared too, because the clamp is on the
# declared property (0..=2). `SlimeBlock`'s bounce is a class behaviour a content declaration
# cannot express (deviation, recorded in `notes.md`).
SLIME_BLOCK = {
    "template": "minecraft:slime_block",
    "properties": {"friction": 0.8, "occludes": False},
}

# The names: the jar's own strings, read below (never retyped).
NAMES = [
    f"block.{MOD}.{BLOCK}",
    f"block.{MOD}.pink_slime_block",
    f"item.{MOD}.pink_slime",
    f"item.{MOD}.pink_slime_ingot",
    "itemGroup.industrialforegoing_ag_hus",
]

# Upstream's own item group for this machine (`ModuleAgricultureHusbandry.TAB_AG_HUS`).
AG_HUS_TAB = {
    "title": "itemGroup.industrialforegoing_ag_hus",
    "icon": f"{MOD}:{BLOCK}",
    "items": [f"{MOD}:{BLOCK}"],
}

# The pink slime items are `TAB_CORE` upstream.
CORE_TAB_ITEMS = [
    f"{MOD}:pink_slime_block",
    f"{MOD}:pink_slime",
    f"{MOD}:pink_slime_ingot",
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


def chamber_recipe(path: Path) -> bytes:
    """One `industrialforegoing:dissolution_chamber` recipe: conditions dropped, cells as strings."""
    value = json.loads(path.read_text())
    value.pop("neoforge:conditions", None)
    value["input"] = [upgrade_ingredient(cell) for cell in value.get("input", [])]
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


def plain_item_model(texture: str, display: dict) -> bytes:
    """A plain item's model: `generated` over the jar's texture, with the jar's display."""
    return pretty(
        {
            "parent": "minecraft:item/generated",
            "textures": {"layer0": f"{MOD}:item/{texture}"},
            "display": display,
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
    for name in CHAMBER_RECIPES:
        path = JAR / f"data/{MOD}/recipe/dissolution_chamber/{name}.json"
        out[ROOT / f"data/{MOD}/recipe/dissolution_chamber/{name}.json"] = chamber_recipe(path)
    # The machine: declaration, item definition, loot, name.
    out[ROOT / f"data/{MOD}/horizon/block/{BLOCK}.json"] = pretty(MACHINE_BLOCK)
    out[ROOT / f"assets/{MOD}/items/{BLOCK}.json"] = pretty(
        {"model": {"type": "minecraft:model", "model": f"{MOD}:item/{BLOCK}"}}
    )
    out[ROOT / f"data/{MOD}/loot_table/blocks/{BLOCK}.json"] = loot_table(BLOCK)
    # The pink slime block: declaration, item definition, loot.
    out[ROOT / f"data/{MOD}/horizon/block/pink_slime_block.json"] = pretty(SLIME_BLOCK)
    out[ROOT / f"data/{MOD}/loot_table/blocks/pink_slime_block.json"] = loot_table(
        "pink_slime_block"
    )
    # The pink slime items: 26.3 definitions, declarations and models.
    display = json.loads((JAR / f"assets/{MOD}/models/item/item.json").read_text())["display"]
    for item in ("pink_slime", "pink_slime_ingot"):
        out[ROOT / f"assets/{MOD}/items/{item}.json"] = pretty(
            {"model": {"type": "minecraft:model", "model": f"{MOD}:item/{item}"}}
        )
        out[ROOT / f"assets/{MOD}/models/item/{item}.json"] = plain_item_model(item, display)
        out[ROOT / f"data/{MOD}/horizon/item/{item}.json"] = pretty(
            {"template": "minecraft:paper"}
        )
    out[ROOT / f"assets/{MOD}/items/pink_slime_block.json"] = pretty(
        {"model": {"type": "minecraft:model", "model": f"{MOD}:item/pink_slime_block"}}
    )
    # The two convention tags the jar declares for the set.
    out[ROOT / "data/c/tags/item/ingots/pink_slime.json"] = pretty(
        {"values": [f"{MOD}:pink_slime_ingot"]}
    )
    out[ROOT / "data/c/tags/block/storage_blocks/pink_slime.json"] = pretty(
        {"values": [f"{MOD}:pink_slime_block"]}
    )
    # The names, and the item group the machine is in.
    jar_lang = json.loads((JAR / f"assets/{MOD}/lang/en_us.json").read_text())
    lang_path = ROOT / f"assets/{MOD}/lang/en_us.json"
    lang = json.loads(lang_path.read_text())
    lang.update({key: jar_lang[key] for key in NAMES})
    out[lang_path] = pretty(lang)
    out[ROOT / f"data/{MOD}/horizon/creative_tab/ag_hus.json"] = pretty(AG_HUS_TAB)
    out[ROOT / f"data/{MOD}/horizon/creative_tab/core.json"] = merged(
        ROOT / f"data/{MOD}/horizon/creative_tab/core.json", CORE_TAB_ITEMS, "items"
    )
    # Both new blocks are `requires_tool`/pickaxe-mineable in the jar's own tag.
    out[ROOT / "data/minecraft/tags/block/mineable/pickaxe.json"] = merged(
        ROOT / "data/minecraft/tags/block/mineable/pickaxe.json",
        [f"{MOD}:{BLOCK}", f"{MOD}:pink_slime_block"],
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
            f"checked {len(files)} slaughter files: "
            + ("identical" if differences == 0 else f"{differences} differ")
        )
        return 0 if differences == 0 else 1
    print(f"wrote {len(files)} slaughter files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
