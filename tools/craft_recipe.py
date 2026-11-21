#!/usr/bin/env python3
"""Craft the Water Condensator from its carried recipe, then show a wrong grid stays empty.

Called by `tools/e2e.sh` with the open game's control socket and the crafting table's
position. The carried recipe is shaped:

    P B P
    L M L
    G R G

with `P` = `#c:plastics` or `minecraft:slime_ball`, `B` = `minecraft:water_bucket`,
`L` = `minecraft:piston`, `M` = `#industrialforegoing:machine_frame/pity` or
`minecraft:iron_block`, `G` = `#c:gears/iron` or `minecraft:iron_block` and
`R` = `minecraft:redstone` — the tags upstream relies on are not in this package, so
the vanilla fallbacks are what makes the machine craftable standalone (the "Upstream
defects fixed" entry in `notes.md`).

The harness's crafting table menu numbers its grid 100-108 row-major and its result slot
200 (`horizon-ctl`'s `open-inventory` documents the same layout for the player's 2x2).
The check asserts the exact result id and count, that taking it reaches the inventory,
that every ingredient was consumed, and that a wrong grid (redstone replaced by glowstone)
leaves the result slot empty. Exits 1 with the observed slot on the first mismatch.
"""
import json
import subprocess
import sys

GRID = [str(slot) for slot in range(100, 109)]
RESULT = "200"
MACHINE = "industrialforegoing:water_condensator"
# (item, count, cells row-major in the pattern above)
INGREDIENTS = [
    ("minecraft:slime_ball", 2, ["100", "102"]),
    ("minecraft:water_bucket", 1, ["101"]),
    ("minecraft:piston", 2, ["103", "105"]),
    ("minecraft:iron_block", 3, ["104", "106", "108"]),
    ("minecraft:redstone", 1, ["107"]),
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
        raise SystemExit(f"missing ingredient {item} in the inventory")

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

    def place(item, whole, targets):
        """Put `item` into `targets`; `whole` left-clicks a stack into one cell."""
        source = find(item)
        run("click-slot", str(source))
        wait(1)
        if whole:
            run("click-slot", targets[0])
            wait(1)
        else:
            for target in targets:
                run("click-slot", target, "right")
                wait(1)
        run("click-slot", str(source))
        wait(2)

    def setup(extra):
        try:
            run("close")
        except SystemExit:
            pass
        wait(3)
        run("command", "clear @s")
        for item, count in extra:
            run("give", item, str(count))
        wait(6)
        open_table()

    failures = []

    setup([(item, count) for item, count, _ in INGREDIENTS])
    for item, _, cells in INGREDIENTS:
        place(item, whole=len(cells) == 1 and item == "minecraft:water_bucket", targets=cells)
    got = result()
    print("the declared grid ->", json.dumps(got))
    if not got or got.get("item") != MACHINE or got.get("count") != 1:
        dump("the declared grid")
        failures.append(f"the declared recipe did not craft one {MACHINE}: {got}")
    else:
        take_result()
        counts = run("inventory")["counts"]
        if not counts.get(MACHINE):
            failures.append("the crafted machine did not reach the inventory")
        for item, _, _ in INGREDIENTS:
            if counts.get(item, 0) != 0:
                failures.append(f"{item} was not consumed ({counts.get(item)} left)")

    # The control: the same grid with the redstone swapped for glowstone must match nothing.
    setup(
        [
            ("minecraft:slime_ball", 2),
            ("minecraft:water_bucket", 1),
            ("minecraft:piston", 2),
            ("minecraft:iron_block", 3),
            ("minecraft:glowstone_dust", 1),
        ]
    )
    for item, cells in (
        ("minecraft:slime_ball", ["100", "102"]),
        ("minecraft:water_bucket", ["101"]),
        ("minecraft:piston", ["103", "105"]),
        ("minecraft:iron_block", ["104", "106", "108"]),
        ("minecraft:glowstone_dust", ["107"]),
    ):
        place(item, whole=item == "minecraft:water_bucket", targets=cells)
    got = result()
    print("glowstone instead of redstone ->", json.dumps(got))
    if got is not None:
        failures.append(f"a wrong grid crafted {got['item']}")

    try:
        run("close")
        wait(3)
    except SystemExit:
        pass
    if failures:
        for failure in failures:
            print(f"FAIL: {failure}", file=sys.stderr)
        raise SystemExit(1)
    print(f"recipe PASS: the declared grid crafts one {MACHINE} and consumes every ingredient")


if __name__ == "__main__":
    main()
