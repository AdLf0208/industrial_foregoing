#!/usr/bin/env python3
"""Parse a `/industrialforegoing info` line of a **mob slaughter factory** into shell variables.

Called by `tools/e2e.sh` as `eval "$(python3 tools/parse_slaughter.py "$LINE")"`: it prints one
`NAME=value` line per field, all shell-quoted, plus `ok=1` when the line is a factory's (and
`ok=0` with nothing else when it is not). The line is the machine's whole state, and the two
tanks are reported twice — as the block entity holds them and as the declared fluid capability
answers:

    Mob Slaughter Factory at [250, 101, 0]: 4600/10000 FE, tank industrialforegoing:meat
    10/8000 mB, capability industrialforegoing:meat 10 mB, tank industrialforegoing:pink_slime
    200/8000 mB, capability industrialforegoing:pink_slime 200 mB, progress 14/100, victims 0,
    facing west, augments speed 1 efficiency 1.00, t=8123
"""
import re
import shlex
import sys

PATTERN = re.compile(
    r"Mob Slaughter Factory at \[(?P<x>-?\d+), (?P<y>-?\d+), (?P<z>-?\d+)\]: "
    r"(?P<energy>-?\d+)/(?P<efe>-?\d+) FE, "
    r"tank (?P<meat>empty|\S+) (?P<meat_mb>-?\d+)/(?P<meat_cap>-?\d+) mB, "
    r"capability (?P<meat_cap_fluid>\S+) (?P<meat_cap_mb>-?\d+) mB, "
    r"tank (?P<slime>empty|\S+) (?P<slime_mb>-?\d+)/(?P<slime_cap>-?\d+) mB, "
    r"capability (?P<slime_cap_fluid>\S+) (?P<slime_cap_mb>-?\d+) mB, "
    r"progress (?P<progress>-?\d+)/(?P<bar>-?\d+), "
    r"victims (?P<victims>-?\d+), "
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
        print(f"S_{key.upper()}={shlex.quote(value)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
