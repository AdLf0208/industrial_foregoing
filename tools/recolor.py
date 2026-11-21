#!/usr/bin/env python3
"""Craft the 16 laser-lens recolours in the player's own 2x2 grid.

Each carried recipe is `RecolorRecipe`: any lens + that colour's dye -> the
colour's lens (`laser_lens_<colour>_recolor.json`, shapeless). Upstream names
the first ingredient as a list of all 16 lens items, which 26.3 rejects whole,
so the port's own `industrialforegoing:laser_lens` tag carries the same set;
the dye is `c:dyes/<colour>`, which the port declares with the vanilla dye
(26.3's base data ships no `c:` item tags).

The player inventory's crafting grid is slots 100-103 row-major with the
result at 200 (`horizon-ctl open-inventory`), so a shapeless two-ingredient
recipe fits without a table. For every colour the check asserts the counts in
the player's inventory **before** (one lens, one dye) and **after** (the lens is
gone, the dye is gone, one lens of the new colour) and the result slot's exact
id — the brief's "counts before and after".

    python3 tools/recolor.py SOCK CTL [expected-count]

`expected-count` defaults to 1; `SELFTEST=core` passes 2 so the assertion can
fail (the red lens is the one the brief names).
"""
import json
import subprocess
import sys

LENS_GRID = "100"
DYE_GRID = "101"
RESULT = "200"
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


def main():
    sock, ctl = sys.argv[1], sys.argv[2]
    expected = int(sys.argv[3]) if len(sys.argv) > 3 else 1

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

    def counts():
        return run("inventory")["counts"]

    def slots():
        screen = run("screen")
        if "slots" not in screen:
            raise SystemExit(f"no menu is open: {screen}")
        return screen["slots"]

    def find(item):
        for slot in slots():
            if slot.get("item") == item and slot["slot"] < 100:
                return slot["slot"]
        raise SystemExit(f"missing {item} in the inventory: {slots()}")

    def result():
        stack = [s for s in slots() if s["slot"] == int(RESULT)]
        if not stack or not stack[0].get("item"):
            return None
        return stack[0]

    failures = []
    for colour in COLOURS:
        target = f"industrialforegoing:{colour}_laser_lens"
        dye = f"minecraft:{colour}_dye"
        # The recipe takes *any* lens (the port's `industrialforegoing:laser_lens`
        # tag), so the white recolour is fed a black lens: a white lens would
        # come back as itself and the consumption would be invisible.
        source = "black" if colour == "white" else "white"
        lens = f"industrialforegoing:{source}_laser_lens"
        try:
            run("close")
        except SystemExit:
            pass
        wait(2)
        run("command", "clear @s")
        run("give", lens, "1")
        run("give", dye, "1")
        wait(6)
        before = counts()
        if before.get(lens, 0) != 1:
            failures.append(f"the {source} lens was not given before the {colour} recolour")
        if before.get(dye, 0) != 1:
            failures.append(f"the {dye} was not given")
        run("open-inventory")
        wait(4)
        source_slot = find(lens)
        run("click-slot", str(source_slot))
        wait(1)
        run("click-slot", LENS_GRID)
        wait(1)
        source_slot = find(dye)
        run("click-slot", str(source_slot))
        wait(1)
        run("click-slot", DYE_GRID)
        wait(2)
        got = result()
        print(f"{colour}: {json.dumps(got)}", file=sys.stderr)
        if not got or got.get("item") != target or got.get("count") != 1:
            failures.append(f"the {colour} recolour did not craft {target}: {got}")
            run("close")
            continue
        run("click-slot", RESULT)
        wait(4)
        run("close")
        wait(2)
        after = counts()
        want = expected if colour == "red" else 1
        if after.get(target, 0) != want:
            failures.append(f"the {colour} lens count after the craft is {after.get(target, 0)}, expected {want}")
        for spent in (lens, dye):
            if after.get(spent, 0) != 0:
                failures.append(f"{spent} was not consumed ({after.get(spent, 0)} left)")
        print(f"  {colour}: {lens} 1->{after.get(lens, 0)}, "
              f"{dye} 1->{after.get(dye, 0)}, {target} 0->{after.get(target, 0)}", file=sys.stderr)

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}", file=sys.stderr)
        raise SystemExit(1)
    print(f"crafted all {len(COLOURS)} lens recolours and read the player's counts before and after each")


if __name__ == "__main__":
    main()
