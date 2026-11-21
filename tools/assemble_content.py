#!/usr/bin/env python3
"""Assemble the port's `assets/` and `data/` from the pack's Industrial Foregoing jar.

The jar is the ATM10 file
(`ports/_shared/licence-cache/industrialforegoing-1.21-3.6.39.jar`, SHA-1
`a653f5c2…77fd` = the pack record), kept as the licence and provenance
evidence. The blockstates, models, textures, loot tables, recipes and
advancements come out of the jar, re-spelled only where 26.3 differs:

* `{"tag": T}` / `{"item": I}` ingredients -> `"#T"` / `"I"`;
* a **list** of alternative ingredients (the lens recolours' "any lens") ->
  the port's own `#industrialforegoing:laser_lens` tag, because 26.3 rejects a
  list naming a tag and drops the whole recipe (slice 2, on this port's own
  condensator recipe);
* a loot pool's `conditions` list -> one `condition` with a typed `type`, and
  the inner `{"condition": …}` -> `{"type": …}`;
* the unknown `neoforge:conditions` gate dropped (the item exists whenever this
  mod does; 26.3 drops the file for it).

The block/item declarations, the 26.3 item definitions and the lang subset are
written here. Rerun it to rebuild (`assets/` and `data/` are deleted first):

    python3 tools/assemble_content.py
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import shutil
import zipfile

ROOT = Path(__file__).resolve().parents[1]
JAR = ROOT.parent / "_shared/licence-cache/industrialforegoing-1.21-3.6.39.jar"
# Since the 2026-11-18 SDK migration `assets/` and `data/` sit at the port root,
# beside `src/` and the one `horizon-mod.toml`; the script rebuilds them there
# and never writes the manifest (the port's own `[native]` one is hand-kept).
CONTENT = ROOT
MOD = "industrialforegoing"
# The pack's Modrinth SHA-1 for industrialforegoing-1.21-3.6.39.jar.
JAR_SHA1 = "a653f5c2b56f51c82d008b473f2b89aeff5747fd"

# Carried byte-for-byte: the jar's own assets for the block and the addons.
CARRIED_ASSETS = [
    "assets/industrialforegoing/blockstates/water_condensator.json",
    "assets/industrialforegoing/models/block/water_condensator.json",
    "assets/industrialforegoing/models/block/base_block.json",
    "assets/industrialforegoing/models/item/water_condensator.json",
    "assets/industrialforegoing/textures/block/water_condensator_side.png",
    "assets/industrialforegoing/textures/block/water_condensator_top.png",
    "assets/industrialforegoing/textures/block/base/side.png",
    "assets/industrialforegoing/textures/block/base/top.png",
    "assets/industrialforegoing/textures/block/base/bottom.png",
    # Slice 3: the four addon items' own textures.
    "assets/industrialforegoing/textures/item/speed_addon_1.png",
    "assets/industrialforegoing/textures/item/speed_addon_2.png",
    "assets/industrialforegoing/textures/item/efficiency_addon_1.png",
    "assets/industrialforegoing/textures/item/efficiency_addon_2.png",
]

# The four `MachineFrameBlock`s (`ModuleCore.PITY`..`SUPREME`): the jar's own
# blockstate is state-free, its block model is `block/orientable` over the
# tier's three base textures, and its item model is the block model. Rarities
# are Titanium's custom enum extensions (green/aqua/light-purple/gold); the
# port maps the four tiers onto vanilla's four rarity slots in order.
FRAMES = [
    # (id, tier texture directory, vanilla rarity)
    ("machine_frame_pity", "pity", "common"),
    ("machine_frame_simple", "simple", "uncommon"),
    ("machine_frame_advanced", "advanced", "rare"),
    ("machine_frame_supreme", "supreme", "epic"),
]

# The 16 `LaserLensItem`s, in `DyeColor` order — the order of the jar's
# `laser_lens<0-15>` textures and of `ModuleCore.LASER_LENS`.
COLOURS = [
    ("white", "White"),
    ("orange", "Orange"),
    ("magenta", "Magenta"),
    ("light_blue", "Light Blue"),
    ("yellow", "Yellow"),
    ("lime", "Lime"),
    ("pink", "Pink"),
    ("gray", "Gray"),
    ("light_gray", "Light Gray"),
    ("cyan", "Cyan"),
    ("purple", "Purple"),
    ("blue", "Blue"),
    ("brown", "Brown"),
    ("green", "Green"),
    ("red", "Red"),
    ("black", "Black"),
]

# The plain items this slice ships (`ModuleCore.IRON_GEAR`.. and `PLASTIC`).
PLAIN_ITEMS = ["iron_gear", "gold_gear", "diamond_gear", "plastic"]

# Carried with the 26.3 upgrade (see the module docstring).
CARRIED_DATA = [
    "data/industrialforegoing/loot_table/blocks/water_condensator.json",
    "data/industrialforegoing/recipe/water_condensator.json",
    "data/industrialforegoing/advancement/recipes/misc/water_condensator.json",
]
# The frames' own loot tables and their pity frame's recipe: `droppingSelf` and
# a shaped crafting recipe. The other three frames' recipes are
# dissolution-chamber ones (a later slice).
CARRIED_FRAME_DATA = [
    "data/industrialforegoing/loot_table/blocks/machine_frame_pity.json",
    "data/industrialforegoing/loot_table/blocks/machine_frame_simple.json",
    "data/industrialforegoing/loot_table/blocks/machine_frame_advanced.json",
    "data/industrialforegoing/loot_table/blocks/machine_frame_supreme.json",
    "data/industrialforegoing/recipe/machine_frame_pity.json",
    "data/industrialforegoing/advancement/recipes/misc/machine_frame_pity.json",
]
# The gears' and the plastic's recipes: the three gears are shaped crafting; the
# plastic's is `smelting dryrubber`, and dryrubber is the latex chain this slice
# does not carry, so that recipe stays out (a recipe naming an absent item is
# dropped whole).
GEAR_DATA = [
    "data/industrialforegoing/recipe/iron_gear.json",
    "data/industrialforegoing/recipe/gold_gear.json",
    "data/industrialforegoing/recipe/diamond_gear.json",
    "data/industrialforegoing/advancement/recipes/misc/iron_gear.json",
    "data/industrialforegoing/advancement/recipes/misc/gold_gear.json",
    "data/industrialforegoing/advancement/recipes/misc/diamond_gear.json",
]
# The 16 recolour recipes (`RecolorRecipe`: any lens + the colour's dye) and
# their advancements (which carry no conditions of their own).
def lens_data(colour: str) -> list[str]:
    """One colour's recolour recipe and its advancement."""
    return [
        f"data/industrialforegoing/recipe/laser_lens_{colour}_recolor.json",
        f"data/industrialforegoing/advancement/recipes/misc/laser_lens_{colour}_recolor.json",
    ]


# The tags the jar declares for these ids, carried as they are: the pack's
# add-ons bind to them (`mifa` reads `c:gears/*`, `industrialforegoingsouls`
# reads `c:plastics`, `c:gears/diamond` and `machine_frame/advanced`).
CARRIED_TAGS = [
    "data/c/tags/item/gears.json",
    "data/c/tags/item/gears/iron.json",
    "data/c/tags/item/gears/gold.json",
    "data/c/tags/item/gears/diamond.json",
    "data/c/tags/item/plastics.json",
]
CARRIED_FRAME_TAGS = [
    f"data/industrialforegoing/tags/{kind}/machine_frame/{tier}.json"
    for kind in ("block", "item")
    for tier in ("pity", "simple", "advanced", "supreme")
]

# The jar's strings this port uses, read from the jar (never retyped).
LANG_KEYS = [
    "block.industrialforegoing.water_condensator",
    "itemGroup.industrialforegoing_resource_production",
    "itemGroup.industrialforegoing_core",
    "text.industrialforegoing.tooltip.power_optional",
]

# Slice 3: the parent's own four addon items (`ModuleCore` registers tiers 1 and
# 2 of each), with the `titanium:augments` value its item class sets
# (`SpeedAddonItem`: 1 + tier; `EfficiencyAddonItem`: 1 - tier * 0.1). Their
# recipes are dissolution-chamber ones, so this slice ships the items alone.
ADDONS = [
    # (id, component key, value, the jar's texture name)
    ("speed_addon_tier_1", "Speed", 2.0, "speed_addon_1"),
    ("speed_addon_tier_2", "Speed", 3.0, "speed_addon_2"),
    ("efficiency_addon_tier_1", "Efficiency", 0.9, "efficiency_addon_1"),
    ("efficiency_addon_tier_2", "Efficiency", 0.8, "efficiency_addon_2"),
]

# The names upstream's `getDescriptionId` composes from `item.industrialforegoing.addon`
# + `.speed`/`.efficiency` + `.tier` + the number, written as one key per item.
ADDON_NAMES = {
    "speed_addon_tier_1": "Addon: Speed Tier 1",
    "speed_addon_tier_2": "Addon: Speed Tier 2",
    "efficiency_addon_tier_1": "Addon: Efficiency Tier 1",
    "efficiency_addon_tier_2": "Addon: Efficiency Tier 2",
}

# 26.3's item model definition: the jar's own `models/item` file, named by it.
ITEM_DEFINITION = {"model": {"type": "minecraft:model", "model": f"{MOD}:item/water_condensator"}}


def addon_definition(item: str) -> dict:
    """26.3's item model definition for one addon item."""
    return {"model": {"type": "minecraft:model", "model": f"{MOD}:item/{item}"}}


def item_definition(item: str) -> dict:
    """26.3's item model definition naming the jar's own `models/item/<item>`."""
    return {"model": {"type": "minecraft:model", "model": f"{MOD}:item/{item}"}}


def plain_item_model(texture: str, display: dict) -> dict:
    """A `RecipelessCustomItem`/`LaserLensItem` model: the jar's own points at
    `builtin/generated` through `industrialforegoing:item/item`, which 26.3
    spells `minecraft:item/generated`; the display transforms are the jar's."""
    return {
        "parent": "minecraft:item/generated",
        "textures": {"layer0": f"{MOD}:item/{texture}"},
        "display": display,
    }


def addon_model(texture: str) -> dict:
    """The addon model: the jar's own points at `builtin/generated`, which 26.3
    spells `minecraft:item/generated`; the layer0 texture is the jar's."""
    return {"parent": "minecraft:item/generated", "textures": {"layer0": f"{MOD}:item/{texture}"}}


def addon_item(kind: str, value: float) -> dict:
    """One addon item: `Properties().stacksTo(16)` and its one component."""
    return {
        "template": "minecraft:stick",
        "max_stack_size": 16,
        "components": {"titanium:augments": {kind: value}},
    }


def plain_item(lens: bool) -> dict:
    """One of `ModuleCore`'s plain items: `new Item.Properties()` (a lens adds
    only `stacksTo(1)`). The template is a neutral vanilla item — paper, which
    has no use, no tool stats and no fuel value — so the declaration changes
    nothing else."""
    item = {"template": "minecraft:paper"}
    if lens:
        item["max_stack_size"] = 1
    return item


# The machine is `new WaterCondensatorBlock()`: `Properties.ofFullCopy(IRON_BLOCK)`
# (hardness 5.0, blast resistance 6.0, correct tool for drops, the iron block's
# sound group — the declaration names a vanilla *block*, not a sound id), a
# full cube, and Titanium's `RotationType.FOUR_WAY` — a `subfacing` state
# property with four values, which the jar's blockstate selects on. Declared
# here because `minecraft:iron_block` has no states of its own.
BLOCK = {
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


def frame_block(rarity: str) -> dict:
    """A `MachineFrameBlock`: `Properties.ofFullCopy(IRON_BLOCK)` with no state
    property (its blockstate's one variant is keyed `""`), and a
    `MachineFrameItem` whose rarity is Titanium's custom one."""
    return {
        "template": "minecraft:iron_block",
        "properties": {
            "hardness": 5.0,
            "explosion_resistance": 6.0,
            "requires_tool": True,
            "sounds": "minecraft:iron_block",
        },
        "components": {"minecraft:rarity": rarity},
    }


# Upstream's own item groups (`TabList.TAB_RESOURCE` and `TAB_CORE`).
TAB = {
    "title": "itemGroup.industrialforegoing_resource_production",
    "icon": f"{MOD}:water_condensator",
    "items": [f"{MOD}:water_condensator"],
}
CORE_TAB = {
    "title": "itemGroup.industrialforegoing_core",
    "icon": f"{MOD}:machine_frame_pity",
    "items": [f"{MOD}:{frame}" for frame, _, _ in FRAMES]
    + [f"{MOD}:{colour}_laser_lens" for colour, _ in COLOURS]
    + [f"{MOD}:{item}" for item in PLAIN_ITEMS],
}


def upgrade_ingredient(value):
    """One 1.21.1 ingredient in 26.3's spelling.

    A **list** is 1.21.1's "one of these" (the lens recolours name all 16
    lenses): 26.3 rejects a list whole — `invalid ingredient (unresolved tag or
    malformed input)` — so the port's own tag carries the same set, one tag per
    ingredient as slice 2's repair requires.
    """
    if isinstance(value, list):
        return f"#{MOD}:laser_lens"
    if isinstance(value, dict) and "tag" in value:
        return f"#{value['tag']}"
    if isinstance(value, dict) and "item" in value:
        return value["item"]
    return value


def upgrade_recipe(recipe: dict) -> dict:
    """The carry pipeline's 1.21.1 -> 26.3 recipe upgrade."""
    # `neoforge:conditions` is not a 26.3 recipe condition; the item exists
    # whenever this mod does, so the gate says nothing here.
    recipe.pop("neoforge:conditions", None)
    for key, value in list(recipe.get("key", {}).items()):
        recipe["key"][key] = upgrade_ingredient(value)
    if "ingredients" in recipe:
        recipe["ingredients"] = [upgrade_ingredient(v) for v in recipe["ingredients"]]
    if "ingredient" in recipe:
        recipe["ingredient"] = upgrade_ingredient(recipe["ingredient"])
    return recipe


def upgrade_advancement(advancement: dict) -> dict:
    """26.3 has no `neoforge:conditions`, so the file was left out entirely."""
    advancement.pop("neoforge:conditions", None)
    return advancement


def upgrade_loot(loot: dict) -> dict:
    """The carry pipeline's loot upgrade: one `condition` object per pool."""
    for pool in loot.get("pools", []):
        conditions = pool.pop("conditions", None)
        if conditions:
            condition = dict(conditions[0])
            condition["type"] = f"minecraft:{condition.pop('condition').split(':')[-1]}"
            pool["condition"] = condition
    return loot


def write(path: Path, data) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(data, bytes):
        path.write_bytes(data)
    else:
        path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")


def carried_data(archive: zipfile.ZipFile, name: str, upgraded: bool) -> bytes:
    """One carried data file, upgraded when it is not already 26.3's shape."""
    raw = archive.read(name)
    if not upgraded:
        return raw
    data = json.loads(raw)
    if name.startswith("data/industrialforegoing/loot_table/"):
        data = upgrade_loot(data)
    elif name.startswith("data/industrialforegoing/recipe/"):
        data = upgrade_recipe(data)
    elif name.startswith("data/industrialforegoing/advancement/"):
        data = upgrade_advancement(data)
    return (json.dumps(data, indent=2) + "\n").encode()


def main() -> None:
    raw = JAR.read_bytes()
    digest = hashlib.sha1(raw).hexdigest()
    if digest != JAR_SHA1:
        raise SystemExit(f"jar SHA-1 {digest} is not the pack record {JAR_SHA1}")
    archive = zipfile.ZipFile(JAR)
    for folder in ("assets", "data"):
        shutil.rmtree(CONTENT / folder, ignore_errors=True)

    for name in CARRIED_ASSETS:
        write(CONTENT / name, archive.read(name))
    for name in CARRIED_TAGS:
        write(CONTENT / name, archive.read(name))
    for name in CARRIED_DATA + CARRIED_FRAME_DATA + GEAR_DATA + CARRIED_FRAME_TAGS:
        write(CONTENT / name, carried_data(archive, name, upgraded=True))
    for colour, _ in COLOURS:
        for name in lens_data(colour):
            write(CONTENT / name, carried_data(archive, name, upgraded=True))

    lang = json.loads(archive.read("assets/industrialforegoing/lang/en_us.json"))
    strings = {key: lang[key] for key in LANG_KEYS}
    strings.update(
        {f"item.industrialforegoing.{item}": name for item, name in ADDON_NAMES.items()}
    )
    # The frames' own block names, and the 24 items' keys. `LaserLensItem.getName`
    # composes `item.industrialforegoing.laser_lens` with the colour's vanilla
    # name ("Red"), so each lens gets its own resolved string here.
    for frame, _, _ in FRAMES:
        strings[f"block.industrialforegoing.{frame}"] = lang[f"block.industrialforegoing.{frame}"]
    for item in PLAIN_ITEMS:
        strings[f"item.industrialforegoing.{item}"] = lang[f"item.industrialforegoing.{item}"]
    for colour, name in COLOURS:
        strings[f"item.industrialforegoing.{colour}_laser_lens"] = f"{name} Laser Lens"
    write(CONTENT / f"assets/{MOD}/lang/en_us.json", strings)

    # The 26.3 item definitions and models of the block and the addons.
    write(CONTENT / f"assets/{MOD}/items/water_condensator.json", ITEM_DEFINITION)
    for item, kind, value, texture in ADDONS:
        write(CONTENT / f"assets/{MOD}/items/{item}.json", addon_definition(item))
        write(CONTENT / f"assets/{MOD}/models/item/{item}.json", addon_model(texture))
        write(CONTENT / f"data/{MOD}/horizon/item/{item}.json", addon_item(kind, value))

    # The frames: the jar's blockstate, block model, item model and three
    # textures each, 26.3's item definition, and the block declaration.
    for frame, tier, rarity in FRAMES:
        for name in (
            f"assets/{MOD}/blockstates/{frame}.json",
            f"assets/{MOD}/models/block/{frame}.json",
            f"assets/{MOD}/models/item/{frame}.json",
            *(f"assets/{MOD}/textures/block/base/{tier}/{face}.png"
              for face in ("top", "side", "bottom")),
        ):
            write(CONTENT / name, archive.read(name))
        write(CONTENT / f"assets/{MOD}/items/{frame}.json", item_definition(frame))
        write(CONTENT / f"data/{MOD}/horizon/block/{frame}.json", frame_block(rarity))

    # The 20 plain items: the jar's texture, a `generated` model carrying the
    # jar's own display transforms, 26.3's definition and the declaration.
    display = json.loads(archive.read(f"assets/{MOD}/models/item/item.json"))["display"]
    for item in PLAIN_ITEMS:
        model = json.loads(archive.read(f"assets/{MOD}/models/item/{item}.json"))
        texture = model["textures"]["layer0"].split("/")[-1]
        write(CONTENT / f"assets/{MOD}/textures/item/{texture}.png",
              archive.read(f"assets/{MOD}/textures/item/{texture}.png"))
        write(CONTENT / f"assets/{MOD}/models/item/{item}.json",
              plain_item_model(texture, display))
        write(CONTENT / f"assets/{MOD}/items/{item}.json", item_definition(item))
        write(CONTENT / f"data/{MOD}/horizon/item/{item}.json", plain_item(lens=False))
    for colour, _ in COLOURS:
        item = f"{colour}_laser_lens"
        model = json.loads(archive.read(f"assets/{MOD}/models/item/{item}.json"))
        texture = model["textures"]["layer0"].split("/")[-1]
        write(CONTENT / f"assets/{MOD}/textures/item/{texture}.png",
              archive.read(f"assets/{MOD}/textures/item/{texture}.png"))
        write(CONTENT / f"assets/{MOD}/models/item/{item}.json",
              plain_item_model(texture, display))
        write(CONTENT / f"assets/{MOD}/items/{item}.json", item_definition(item))
        write(CONTENT / f"data/{MOD}/horizon/item/{item}.json", plain_item(lens=True))

    write(CONTENT / f"data/{MOD}/horizon/block/water_condensator.json", BLOCK)
    write(CONTENT / f"data/{MOD}/horizon/creative_tab/resource_production.json", TAB)
    write(CONTENT / f"data/{MOD}/horizon/creative_tab/core.json", CORE_TAB)

    # The port's own tags, written here: the "any lens" the recolour recipes
    # name, and the convention values of the tags those recipes and the frame's
    # recipe use, because 26.3's base data ships no `c:` item tags at all.
    write(
        CONTENT / f"data/{MOD}/tags/item/laser_lens.json",
        {"values": [f"{MOD}:{colour}_laser_lens" for colour, _ in COLOURS]},
    )
    write(CONTENT / "data/c/tags/item/ingots/iron.json", {"values": ["minecraft:iron_ingot"]})
    write(
        CONTENT / "data/c/tags/item/storage_blocks/redstone.json",
        {"values": ["minecraft:redstone_block"]},
    )
    for colour, _ in COLOURS:
        write(
            CONTENT / f"data/c/tags/item/dyes/{colour}.json",
            {"values": [f"minecraft:{colour}_dye"]},
        )

    write(
        CONTENT / "data/minecraft/tags/block/mineable/pickaxe.json",
        {"values": [f"{MOD}:water_condensator"] + [f"{MOD}:{frame}" for frame, _, _ in FRAMES]},
    )

    files = sorted(
        path
        for folder in ("assets", "data")
        for path in (CONTENT / folder).rglob("*")
        if path.is_file()
    )
    print(f"{len(files)} files under {CONTENT}/{{assets,data}}")
    for path in files:
        print(" ", path.relative_to(CONTENT))


if __name__ == "__main__":
    main()
