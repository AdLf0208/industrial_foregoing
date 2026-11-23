#!/usr/bin/env python3
"""Craft every shaped recipe this slice carries, in one crafting table.

Called by `tools/e2e.sh` with the open game's control socket and a placed
crafting table's position. Each case puts its declared grid into the table,
asserts the exact result id and count, takes it, and asserts the inventory
holds the result and none of the ingredients; a control case puts slice 2's
vanilla stand-ins into the condensator's grid and asserts the result slot stays
empty, which is the counterfactual this slice turns over.

The carried recipes, from the jar:

    water_condensator  PBP/LML/GRG  P=#c:plastics  B=water_bucket  L=piston
                                    M=#industrialforegoing:machine_frame/pity
                                    G=#c:gears/iron  R=redstone
    machine_frame_pity WIW/IRI/WIW  W=#minecraft:logs  I=#c:ingots/iron
                                    R=#c:storage_blocks/redstone
    iron/gold/diamond_gear  " P "/"P P"/" P "   P = the ingot

The tags are what makes these craft: the port declares the convention tags
(`c:gears/*`, `c:plastics`, `c:ingots/iron`, `c:storage_blocks/redstone`) with
their items, and `industrialforegoing:machine_frame/*` is the jar's own.

The harness's crafting table menu numbers its grid 100-108 row-major and its
result slot 200 (`horizon-ctl`'s `open-inventory` documents the same layout for
the player's 2x2).
"""
import json
import subprocess
import sys

GRID = [str(slot) for slot in range(100, 109)]
RESULT = "200"
MACHINE = "industrialforegoing:water_condensator"
FRAME = "industrialforegoing:machine_frame_pity"


def condenser_grid(fill: dict[str, str]) -> list[tuple[str, int, list[str]]]:
    """`PBP/LML/GRG`, with the three tag ingredients taken from `fill`.

    Two cells can name the same item (the control puts iron blocks in the frame
    cell and both gear cells): they are merged into one ingredient, so the
    harness gives one stack and right-clicks a copy into each cell.
    """
    cells = [
        (fill["P"], ["100", "102"]),
        ("minecraft:water_bucket", ["101"]),
        ("minecraft:piston", ["103", "105"]),
        (fill["M"], ["104"]),
        (fill["G"], ["106", "108"]),
        ("minecraft:redstone", ["107"]),
    ]
    merged: dict[str, list[str]] = {}
    for item, where in cells:
        merged.setdefault(item, []).extend(where)
    return [(item, len(where), where) for item, where in merged.items()]


# (label, expected result id or None for the control, expected count, ingredients)
CASES = [
    (
        "the condensator from the tags (#c:plastics, #c:gears/iron, machine_frame/pity)",
        MACHINE,
        1,
        condenser_grid(
            {
                "P": "industrialforegoing:plastic",
                "M": FRAME,
                "G": "industrialforegoing:iron_gear",
            }
        ),
    ),
    (
        "the control: slice 2's vanilla stand-ins leave the result empty",
        None,
        0,
        condenser_grid(
            {
                "P": "minecraft:slime_ball",
                "M": "minecraft:iron_block",
                "G": "minecraft:iron_block",
            }
        ),
    ),    (
        "the pity machine frame (WIW/IRI/WIW)",
        FRAME,
        1,
        [
            ("minecraft:oak_log", 4, ["100", "102", "106", "108"]),
            ("minecraft:iron_ingot", 4, ["101", "103", "105", "107"]),
            ("minecraft:redstone_block", 1, ["104"]),
        ],
    ),
    (
        "the iron gear",
        "industrialforegoing:iron_gear",
        1,
        [("minecraft:iron_ingot", 4, ["101", "103", "105", "107"])],
    ),
    (
        "the gold gear",
        "industrialforegoing:gold_gear",
        1,
        [("minecraft:gold_ingot", 4, ["101", "103", "105", "107"])],
    ),
    (
        "the diamond gear",
        "industrialforegoing:diamond_gear",
        1,
        [("minecraft:diamond", 4, ["101", "103", "105", "107"])],
    ),
    (
        "the mob slaughter factory (PDP/SMS/ARA, #c:gears/gold + machine_frame/pity)",
        "industrialforegoing:mob_slaughter_factory",
        1,
        [
            ("industrialforegoing:plastic", 2, ["100", "102"]),
            ("industrialforegoing:gold_gear", 1, ["101"]),
            # A sword and an axe stack to one, so each cell gets its own `give` (a single
            # stack of two leaves the second right-click with an empty cursor).
            ("minecraft:iron_sword", 1, ["103"]),
            ("minecraft:iron_sword", 1, ["105"]),
            ("industrialforegoing:machine_frame_pity", 1, ["104"]),
            ("minecraft:iron_axe", 1, ["106"]),
            ("minecraft:iron_axe", 1, ["108"]),
            ("minecraft:redstone", 1, ["107"]),
        ],
    ),
    (
        "the mob crusher (PSP/BMB/GRG, #c:gears/gold + machine_frame/advanced)",
        "industrialforegoing:mob_crusher",
        1,
        [
            ("industrialforegoing:plastic", 2, ["100", "102"]),
            # A sword stacks to one, so its cell gets its own `give`.
            ("minecraft:iron_sword", 1, ["101"]),
            ("minecraft:book", 2, ["103", "105"]),
            ("industrialforegoing:machine_frame_advanced", 1, ["104"]),
            ("industrialforegoing:gold_gear", 2, ["106", "108"]),
            ("minecraft:redstone", 1, ["107"]),
        ],
    ),
    (
        "the mob duplicator (PWP/CMC/ERE, #c:plastics + machine_frame/advanced)",
        "industrialforegoing:mob_duplicator",
        1,
        [
            ("industrialforegoing:plastic", 2, ["100", "102"]),
            ("minecraft:nether_wart", 1, ["101"]),
            ("minecraft:magma_cream", 2, ["103", "105"]),
            ("industrialforegoing:machine_frame_advanced", 1, ["104"]),
            ("minecraft:emerald", 2, ["106", "108"]),
            ("minecraft:redstone", 1, ["107"]),
        ],
    ),
    (
        "the Mob Imprisonment Tool ( P /PGP/ P , #c:plastics + a ghast tear)",
        "industrialforegoing:mob_imprisonment_tool",
        1,
        [
            ("industrialforegoing:plastic", 4, ["101", "103", "105", "107"]),
            ("minecraft:ghast_tear", 1, ["104"]),
        ],
    ),
    (
        "the pink slime block from nine pink slime",
        "industrialforegoing:pink_slime_block",
        1,
        [("industrialforegoing:pink_slime", 9, GRID)],
    ),    (
        "nine pink slime back out of the block (shapeless)",
        "industrialforegoing:pink_slime",
        9,
        [("industrialforegoing:pink_slime_block", 1, ["100"])],
    ),
]


def main():
    sock, ctl, x, y, z = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[5]

    def run(*args):
        print("+ ctl", *args, file=sys.stderr, flush=True)
        try:
            out = subprocess.run(
                [ctl, "--socket", sock, "--timeout", "90", *args],
                text=True,
                capture_output=True,
                timeout=120,
            )
        except subprocess.TimeoutExpired:
            raise SystemExit(f"ctl {' '.join(args)}: no reply in 120 s")
        try:
            return json.loads(out.stdout)
        except json.JSONDecodeError:
            raise SystemExit(f"ctl {' '.join(args)}: {out.stdout}{out.stderr}")

    def wait(ticks=4):
        run("wait", str(ticks))

    def slots():
        screen = run("screen")
        if "slots" not in screen:
            raise SystemExit(f"no menu is open: {screen}")
        return screen["slots"]

    def find(item):
        for slot in slots():
            if slot.get("item") == item and slot["slot"] < 100:
                return slot["slot"]
        occupied = [(s["slot"], s.get("item"), s.get("count")) for s in slots() if s.get("item")]
        got = run("inventory")["counts"]
        raise SystemExit(
            f"missing ingredient {item} in the inventory: screen {occupied}, counts {got}"
        )

    def open_table():
        run("look-at-block", x, y, z)
        wait()
        print("  crosshair ->", json.dumps(run("looking-at")), file=sys.stderr)
        print("  interact ->", json.dumps(run("interact", x, y, z)), file=sys.stderr)
        wait(6)
        screen = run("screen")
        if "slots" not in screen:
            raise SystemExit(f"the crafting table at {x},{y},{z} did not open: {screen}")

    def result():
        """The result stack, or None: the harness reports an empty slot as `item: null`."""
        stack = [s for s in slots() if s["slot"] == int(RESULT)]
        if not stack or not stack[0].get("item"):
            return None
        return stack[0]

    def dump(label):
        """Every occupied slot, for a failing run: the grid is what a mismatch is about."""
        occupied = [(s["slot"], s.get("item"), s.get("count")) for s in slots() if s.get("item")]
        print(f"  {label}: {occupied}", file=sys.stderr)

    def take_result():
        run("click-slot", RESULT)
        wait(6)

    def place(item, count, cells):
        """Put `item` into `cells`: one per cell, or the whole stack when it is
        a single item (a bucket, a frame, a redstone block)."""
        targets = cells
        source = find(item)
        run("click-slot", str(source))
        wait(1)
        if len(targets) == 1 and count == 1:
            run("click-slot", targets[0])
            wait(1)
        else:
            for target in targets:
                run("click-slot", target, "right")
                wait(1)
        run("click-slot", str(source))
        wait(2)

    def setup(ingredients):
        try:
            run("close")
        except SystemExit:
            pass
        wait(3)
        run("command", "clear @s")
        for item, count, _ in ingredients:
            run("give", item, str(count))
        wait(6)
        open_table()

    failures = []
    for label, expected, expected_count, ingredients in CASES:
        setup(ingredients)
        for item, count, cells in ingredients:
            place(item, count, cells)
        got = result()
        print(f"{label} -> {json.dumps(got)}")
        if expected is None:
            if got is not None:
                dump(label)
                failures.append(f"{label}: a grid of stand-ins crafted {got['item']}")
            continue
        if not got or got.get("item") != expected or got.get("count") != expected_count:
            dump(label)
            failures.append(f"{label}: expected {expected_count} x {expected}, got {got}")
            continue
        take_result()
        counts = run("inventory")["counts"]
        if counts.get(expected, 0) != expected_count:
            failures.append(f"{label}: the crafted {expected} did not reach the inventory ({counts.get(expected)})")
        for item, _, _ in ingredients:
            if counts.get(item, 0) != 0:
                failures.append(f"{label}: {item} was not consumed ({counts.get(item)} left)")
        # Every grid cell is empty again, except a crafting remainder: a water
        # bucket leaves the empty bucket behind (`Item.Properties.craftRemainder`).
        remainder = (
            "minecraft:bucket"
            if any(item == "minecraft:water_bucket" for item, _, _ in ingredients)
            else None
        )
        left = [s for s in slots() if s["slot"] in range(100, 109) and s.get("item")]
        unexpected = [s for s in left if s["item"] != remainder]
        if unexpected:
            failures.append(f"{label}: the grid still holds {unexpected}")
        if remainder and not any(s["item"] == remainder for s in left):
            failures.append(f"{label}: the water bucket left no {remainder} behind")

    try:
        run("close")
        wait(3)
    except SystemExit:
        pass
    if failures:
        for failure in failures:
            print(f"FAIL: {failure}", file=sys.stderr)
        raise SystemExit(1)
    print(f"crafted {len([c for c in CASES if c[1]])} shaped recipes, and the stand-in grid stayed empty")


if __name__ == "__main__":
    main()
