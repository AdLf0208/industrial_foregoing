#!/usr/bin/env python3
"""Parse a `/industrialforegoing info` line of a **mob crusher** into shell variables.

Called by `tools/e2e.sh` as `eval "$(python3 tools/parse_crusher.py "$LINE")"`: it prints one
`NAME=value` line per field, all shell-quoted, plus `ok=1` when the line is a crusher's (and
`ok=0` with nothing else when it is not). The line is the machine's whole state, and the tank is
reported twice — as the block entity holds it and as the declared fluid capability answers:

    Mob Crusher at [250, 101, 0]: 4600/10000 FE, tank industrialforegoing:essence 40/32000 mB,
    capability industrialforegoing:essence 40 mB, progress 13/100, victims 0,
    output 1x minecraft:beef, 1x minecraft:leather, last xp 2, facing west,
    augments speed 1 efficiency 1.00, t=8123
"""
import re
import shlex
import sys

PATTERN = re.compile(
    r"Mob Crusher at \[(?P<x>-?\d+), (?P<y>-?\d+), (?P<z>-?\d+)\]: "
    r"(?P<energy>-?\d+)/(?P<efe>-?\d+) FE, "
    r"tank (?P<tank>empty|\S+) (?P<tank_mb>-?\d+)/(?P<tank_cap>-?\d+) mB, "
    r"capability (?P<cap_fluid>\S+) (?P<cap_mb>-?\d+) mB, "
    r"progress (?P<progress>-?\d+)/(?P<bar>-?\d+), "
    r"victims (?P<victims>-?\d+), "
    r"output (?P<output>.*?), "
    r"last xp (?P<last_xp>-?\d+), "
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
        print(f"C_{key.upper()}={shlex.quote(value)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
