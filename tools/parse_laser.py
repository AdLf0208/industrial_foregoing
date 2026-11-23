#!/usr/bin/env python3
"""Parse one `/industrialforegoing info` line of the laser drill or its ore base.

Prints shell assignments (`LA_PROGRESS=…`, `LD_ENERGY=…`) and `ok=1`, or `ok=0`
when the line is not one of them. The e2e `eval`s the answer, so every value is
shell-quoted.

    python3 tools/parse_laser.py 'Ore Laser Base at [232, 101, 0]: progress …'
"""

from __future__ import annotations

import re
import shlex
import sys

BASE = re.compile(
    r"^Ore Laser Base at (?P<pos>\[[^\]]*\]): progress (?P<progress>-?\d+)/(?P<max>-?\d+), "
    r"depth (?P<depth>-?\d+), biome (?P<biome>\S+), dimension (?P<dimension>\S+), "
    r"recipe (?P<recipe>.*?), weight (?P<weight>-?\d+), "
    r"recipes (?P<rules>\d+)/(?P<files>\d+), lens (?P<lens>.*?), "
    r"output (?P<output>.*?), augments speed (?P<speed>-?\d+) "
    r"efficiency (?P<efficiency>[\d.]+), t=(?P<t>-?\d+)$"
)

FLUID = re.compile(
    r"^Fluid Laser Base at (?P<pos>\[[^\]]*\]): tank (?P<tank_fluid>\S+) "
    r"(?P<tank_mb>-?\d+)/(?P<tank_cap>-?\d+) mB, capability (?P<cap_fluid>\S+) "
    r"(?P<cap_mb>-?\d+) mB, progress (?P<progress>-?\d+)/(?P<max>-?\d+), "
    r"depth (?P<depth>-?\d+), biome (?P<biome>\S+), dimension (?P<dimension>\S+), "
    r"recipe (?P<recipe>.*?), recipes (?P<rules>\d+)/(?P<files>\d+), "
    r"lens (?P<lens>.*?), augments speed (?P<speed>-?\d+) "
    r"efficiency (?P<efficiency>[\d.]+), t=(?P<t>-?\d+)$"
)

DRILL = re.compile(
    r"^Laser Drill at (?P<pos>\[[^\]]*\]): (?P<energy>-?\d+)/(?P<capacity>-?\d+) FE, "
    r"progress (?P<progress>-?\d+)/(?P<max>-?\d+), target (?P<target>\[[^\]]*\]|none), "
    r"augments speed (?P<speed>-?\d+) efficiency (?P<efficiency>[\d.]+), "
    r"facing (?P<facing>\S+), t=(?P<t>-?\d+)$"
)


def emit(values: dict[str, object]) -> None:
    for key, value in values.items():
        print(f"{key}={shlex.quote(str(value))}")


def main() -> int:
    line = sys.argv[1] if len(sys.argv) > 1 else ""
    if match := BASE.match(line):
        emit(
            {
                "ok": 1,
                "LA_POS": match["pos"],
                "LA_PROGRESS": match["progress"],
                "LA_MAX": match["max"],
                "LA_DEPTH": match["depth"],
                "LA_BIOME": match["biome"],
                "LA_DIM": match["dimension"],
                "LA_RECIPE": match["recipe"],
                "LA_WEIGHT": match["weight"],
                "LA_RULES": match["rules"],
                "LA_FILES": match["files"],
                "LA_LENS": match["lens"],
                "LA_OUTPUT": match["output"],
                "LA_SPEED": match["speed"],
                "LA_EFFICIENCY": match["efficiency"],
                "LA_T": match["t"],
            }
        )
        return 0
    if match := FLUID.match(line):
        emit(
            {
                "ok": 1,
                "LF_POS": match["pos"],
                "LF_TANK_FLUID": match["tank_fluid"],
                "LF_TANK_MB": match["tank_mb"],
                "LF_TANK_CAP": match["tank_cap"],
                "LF_CAP_FLUID": match["cap_fluid"],
                "LF_CAP_MB": match["cap_mb"],
                "LF_PROGRESS": match["progress"],
                "LF_MAX": match["max"],
                "LF_DEPTH": match["depth"],
                "LF_BIOME": match["biome"],
                "LF_DIM": match["dimension"],
                "LF_RECIPE": match["recipe"],
                "LF_RULES": match["rules"],
                "LF_FILES": match["files"],
                "LF_LENS": match["lens"],
                "LF_SPEED": match["speed"],
                "LF_EFFICIENCY": match["efficiency"],
                "LF_T": match["t"],
            }
        )
        return 0
    if match := DRILL.match(line):
        emit(
            {
                "ok": 1,
                "LD_POS": match["pos"],
                "LD_ENERGY": match["energy"],
                "LD_CAPACITY": match["capacity"],
                "LD_PROGRESS": match["progress"],
                "LD_MAX": match["max"],
                "LD_TARGET": match["target"],
                "LD_SPEED": match["speed"],
                "LD_EFFICIENCY": match["efficiency"],
                "LD_FACING": match["facing"],
                "LD_T": match["t"],
            }
        )
        return 0
    print("ok=0")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
