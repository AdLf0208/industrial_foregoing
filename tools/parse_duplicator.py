#!/usr/bin/env python3
"""Parse a `/industrialforegoing info` line of a **mob duplicator** into shell variables.

Called by `tools/e2e.sh` as `eval "$(python3 tools/parse_duplicator.py "$LINE")"`: it prints one
`NAME=value` line per field, all shell-quoted, plus `ok=1` when the line is a duplicator's (and
`ok=0` with nothing else when it is not). The line is the machine's whole state, and the tank is
reported twice — as the block entity holds it and as the declared fluid capability answers:

    Mob Duplicator at [280, 101, 0]: 10000/50000 FE, tank industrialforegoing:essence 120/8000 mB,
    capability industrialforegoing:essence 120 mB, tool minecraft:cow, needs 120 mB, nearby 0,
    progress 61/62, augments speed 1 efficiency 1.00, t=8123
"""
import re
import shlex
import sys

PATTERN = re.compile(
    r"Mob Duplicator at \[(?P<x>-?\d+), (?P<y>-?\d+), (?P<z>-?\d+)\]: "
    r"(?P<energy>-?\d+)/(?P<efe>-?\d+) FE, "
    r"tank (?P<tank>empty|\S+) (?P<tank_mb>-?\d+)/(?P<tank_cap>-?\d+) mB, "
    r"capability (?P<cap_fluid>\S+) (?P<cap_mb>-?\d+) mB, "
    r"tool (?P<tool>\S+), "
    r"needs (?P<needs>-?\d+) mB, "
    r"nearby (?P<nearby>-?\d+), "
    r"progress (?P<progress>-?\d+)/(?P<bar>-?\d+), "
    r"augments speed (?P<speed>-?\d+) efficiency (?P<efficiency>\d+\.\d+), t=(?P<t>-?\d+)$"
)


def main() -> int:
    line = sys.argv[1] if len(sys.argv) > 1 else ""
    match = PATTERN.search(line)
    if not match:
        print("ok=0")
        return 0
    print("ok=1")
    for key, value in match.groupdict().items():
        print(f"D_{key.upper()}={shlex.quote(value)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
