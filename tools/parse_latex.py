#!/usr/bin/env python3
"""Parse a `/industrialforegoing info` line of the **latex chain** into shell variables.

Called by `tools/e2e.sh` as `eval "$(python3 tools/parse_latex.py "$LINE")"`: it prints one
`NAME=value` line per field, all shell-quoted, plus `ok=1` when the line is one of the two
machines (and `ok=0` with nothing else when it is not). Two shapes:

    Fluid Extractor at [202, 101, 0]: 4500/10000 FE, latex industrialforegoing:latex 6/1000 mB,
    progress 35/100, log minecraft:oak_log, recipe industrialforegoing:fluid_extractor/oak,
    hits 0/8, recipes 9/9, augments speed 1 efficiency 1.00, facing west, t=135

    Latex Processing Unit at [204, 101, 0]: 8000/10000 FE, latex industrialforegoing:latex
    250/16000 mB, water minecraft:water 500/16000 mB, progress 0/100, output industrialforegoing:dryrubber x1,
    capability industrialforegoing:latex 250 mB, water capability minecraft:water 500 mB,
    augments speed 1 efficiency 1.00, facing north, t=346
"""
import re
import shlex
import sys

EXTRACTOR = re.compile(
    r"Fluid Extractor at \[(?P<x>-?\d+), (?P<y>-?\d+), (?P<z>-?\d+)\]: "
    r"(?P<energy>-?\d+)/(?P<capacity>-?\d+) FE, "
    r"latex (?P<fluid>\S+) (?P<latex>-?\d+)/(?P<tank>-?\d+) mB, "
    r"progress (?P<progress>-?\d+)/(?P<bar>-?\d+), "
    r"log (?P<log>\S+), "
    r"recipe (?P<recipe>\S+), "
    r"hits (?P<hits>-?\d+)/(?P<hit_max>-?\d+), "
    r"recipes (?P<recipes>-?\d+)/(?P<recipes_total>-?\d+), "
    r"augments speed (?P<speed>-?\d+) efficiency (?P<efficiency>\d+\.\d+), "
    r"facing (?P<facing>\S+), t=(?P<t>-?\d+)$"
)

UNIT = re.compile(
    r"Latex Processing Unit at \[(?P<x>-?\d+), (?P<y>-?\d+), (?P<z>-?\d+)\]: "
    r"(?P<energy>-?\d+)/(?P<capacity>-?\d+) FE, "
    r"latex (?P<fluid>\S+) (?P<latex>-?\d+)/(?P<tank>-?\d+) mB, "
    r"water (?P<water_fluid>\S+) (?P<water>-?\d+)/(?P<water_tank>-?\d+) mB, "
    r"progress (?P<progress>-?\d+)/(?P<bar>-?\d+), "
    r"output (?P<output>.*?), "
    r"capability (?P<cap_fluid>\S+) (?P<cap_mb>-?\d+) mB, "
    r"water capability (?P<cap_water_fluid>\S+) (?P<cap_water_mb>-?\d+) mB, "
    r"augments speed (?P<speed>-?\d+) efficiency (?P<efficiency>\d+\.\d+), "
    r"facing (?P<facing>\S+), t=(?P<t>-?\d+)$"
)


def main() -> int:
    line = sys.argv[1] if len(sys.argv) > 1 else ""
    match = EXTRACTOR.search(line)
    kind = "EXTRACTOR"
    if not match:
        match = UNIT.search(line)
        kind = "UNIT"
    if not match:
        print("ok=0")
        return 0
    print("ok=1")
    print(f"L_KIND={kind}")
    for key, value in match.groupdict().items():
        print(f"L_{key.upper()}={shlex.quote(value)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
