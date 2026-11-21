#!/usr/bin/env python3
"""Assemble the slice-1 `content/` folder from the pack's Industrial Foregoing jar.

The jar is the ATM10 file
(`ports/_shared/licence-cache/industrialforegoing-1.21-3.6.39.jar`, SHA-1
`a653f5c2…77fd` = the pack record), kept as the licence and provenance
evidence. The blockstate, models, textures, loot table, recipe and advancement
come out of the jar, re-spelled only where 26.3 differs (the carry pipeline's
upgrade: `{"tag": T}` → `"#T"`, a loot `conditions` list → one `condition`, and
the `neoforge:conditions` gate dropped because the item exists whenever this
mod does). The block/tab declarations, the 26.3 item definition, the lang
subset and the pickaxe tag are written here.

Rerun it to rebuild `content/` (it deletes its own generated folders first):

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

# Carried byte-for-byte: the jar's own assets for this block.
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
# Carried with the 26.3 upgrade (see below).
CARRIED_DATA = [
    "data/industrialforegoing/loot_table/blocks/water_condensator.json",
    "data/industrialforegoing/recipe/water_condensator.json",
    "data/industrialforegoing/advancement/recipes/misc/water_condensator.json",
]
# The jar's strings this slice uses, read from the jar (never retyped).
LANG_KEYS = [
    "block.industrialforegoing.water_condensator",
    "itemGroup.industrialforegoing_resource_production",
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

# Upstream's own item group (`TabList.TAB_RESOURCE`, `industrialforegoing:resource_production`).
TAB = {
    "title": "itemGroup.industrialforegoing_resource_production",
    "icon": f"{MOD}:water_condensator",
    "items": [f"{MOD}:water_condensator"],
}

def upgrade_recipe(recipe: dict) -> dict:
    """The carry pipeline's 1.21.1 → 26.3 recipe upgrade, plus slice 2's repair."""
    # `neoforge:conditions` is not a 26.3 recipe condition; the item exists
    # whenever this mod does, so the gate says nothing here.
    recipe.pop("neoforge:conditions", None)
    for key, value in list(recipe.get("key", {}).items()):
        if isinstance(value, dict) and "tag" in value:
            recipe["key"][key] = f"#{value['tag']}"
        elif isinstance(value, dict) and "item" in value:
            recipe["key"][key] = value["item"]
    # Slice 2's `Upstream defects fixed` repair: the three ingredients upstream
    # names as tags belong to parts of the mod this package does not carry, and
    # 26.3 drops a recipe whose list names an absent tag — so each is its
    # vanilla fallback alone, the only shape that both loads and crafts
    # standalone. When the frames/plastics are ported these become the tags
    # again, one tag per ingredient.
    fallbacks = {
        "#c:plastics": "minecraft:slime_ball",
        "#c:gears/iron": "minecraft:iron_block",
        "#industrialforegoing:machine_frame/pity": "minecraft:iron_block",
    }
    for key, value in list(recipe.get("key", {}).items()):
        recipe["key"][key] = fallbacks.get(value, value)
    return recipe


def upgrade_advancement(advancement: dict) -> dict:
    """Slice 2's repair: 26.3 has no `neoforge:conditions`, so the file was left
    out entirely; its `has_item` criterion names the absent `#c:plastics` tag."""
    advancement.pop("neoforge:conditions", None)
    criterion = advancement.get("criteria", {}).get("has_item", {})
    for entry in criterion.get("conditions", {}).get("items", []):
        if entry.get("items") == "#c:plastics":
            entry["items"] = "minecraft:slime_ball"
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
    for name in CARRIED_DATA:
        data = archive.read(name)
        if name.endswith("loot_table/blocks/water_condensator.json"):
            data = (json.dumps(upgrade_loot(json.loads(data)), indent=2) + "\n").encode()
        elif name.endswith("recipe/water_condensator.json"):
            data = (json.dumps(upgrade_recipe(json.loads(data)), indent=2) + "\n").encode()
        elif "advancement/" in name:
            data = (json.dumps(upgrade_advancement(json.loads(data)), indent=2) + "\n").encode()
        write(CONTENT / name, data)

    lang = json.loads(archive.read("assets/industrialforegoing/lang/en_us.json"))
    strings = {key: lang[key] for key in LANG_KEYS}
    strings.update(
        {f"item.industrialforegoing.{item}": name for item, name in ADDON_NAMES.items()}
    )
    write(CONTENT / f"assets/{MOD}/lang/en_us.json", strings)
    write(CONTENT / f"assets/{MOD}/items/water_condensator.json", ITEM_DEFINITION)
    for item, kind, value, texture in ADDONS:
        write(CONTENT / f"assets/{MOD}/items/{item}.json", addon_definition(item))
        write(CONTENT / f"assets/{MOD}/models/item/{item}.json", addon_model(texture))
        write(CONTENT / f"data/{MOD}/horizon/item/{item}.json", addon_item(kind, value))
    write(CONTENT / f"data/{MOD}/horizon/block/water_condensator.json", BLOCK)
    write(CONTENT / f"data/{MOD}/horizon/creative_tab/resource_production.json", TAB)
    write(
        CONTENT / "data/minecraft/tags/block/mineable/pickaxe.json",
        {"values": [f"{MOD}:water_condensator"]},
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
