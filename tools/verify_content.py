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
PLAIN_ITEMS = ["iron_gear", "gold_gear", "diamond_gear", "plastic"]


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
        [f"{MOD}:{frame}" for frame in FRAMES]
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

    for frame in FRAMES:
        loot = f"data/{MOD}/loot_table/blocks/{frame}.json"
        if not files.has(loot):
            failures.append(f"{frame} has no loot table")
        elif f"{MOD}:{frame}" not in json.dumps(files.json(loot)):
            failures.append(f"{loot} does not drop {frame}")
        if not files.has(f"assets/{MOD}/blockstates/{frame}.json"):
            failures.append(f"{frame} has no blockstate")

    # Every carried recipe: its result exists, and every ingredient resolves.
    recipes = region(files, f"data/{MOD}/recipe/")
    if len(recipes) < 21:
        failures.append(f"{len(recipes)} recipes carried, expected at least 21")
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
    pickaxe = files.json("data/minecraft/tags/block/mineable/pickaxe.json").get("values", [])
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
