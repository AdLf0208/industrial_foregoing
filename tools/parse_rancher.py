#!/usr/bin/env python3
"""Parse a `/industrialforegoing info` line of an **animal rancher** into shell variables.

Called by `tools/e2e.sh` as `eval "$(python3 tools/parse_rancher.py "$LINE")"`: it prints one
`NAME=value` line per field, all shell-quoted, plus `ok=1` when the line is a rancher's (and `ok=0`
with nothing else when it is not). The line is the machine's whole state, and the tank is reported
twice — as the block entity holds it and as the declared fluid capability answers:

    Animal Rancher at [300, 101, 0]: 9600/10000 FE, tank industrialforegoing:milk 1000/8000 mB,
    capability industrialforegoing:milk 1000 mB, animals 1 (sheep 0, milkable 1, squid 0),
    output empty, progress 33/100, facing west, augments speed 1 efficiency 1.00, t=8123
"""
import re
import shlex
import sys

PATTERN = re.compile(
    r"Animal Rancher at \[(?P<x>-?\d+), (?P<y>-?\d+), (?P<z>-?\d+)\]: "
    r"(?P<energy>-?\d+)/(?P<efe>-?\d+) FE, "
    r"tank (?P<tank>[\w:]+) (?P<tank_mb>-?\d+)/(?P<tank_cap>-?\d+) mB, "
    r"capability (?P<cap_fluid>\S+) (?P<cap_mb>-?\d+) mB, "
    r"animals (?P<animals>-?\d+) "
    r"\(sheep (?P<sheep>-?\d+), milkable (?P<milkable>-?\d+), squid (?P<squid>-?\d+)\), "
    r"output (?P<output>.*?), "
    r"progress (?P<progress>-?\d+)/(?P<bar>-?\d+), "
    r"facing (?P<facing>\S+), "
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
        print(f"R_{key.upper()}={shlex.quote(value)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
