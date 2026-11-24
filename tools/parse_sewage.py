#!/usr/bin/env python3
"""Parse a `/industrialforegoing info` line of a **sewer** or **sewage composter**.

Called by `tools/e2e.sh` as `eval "$(python3 tools/parse_sewage.py "$LINE")"`: one shell-quoted
`NAME=value` per field and `ok=1`, or `ok=0` for another machine's line. The tank is reported twice
— as the block entity holds it and as the declared fluid capability answers.
"""
import re
import shlex
import sys

PREFIX = r"(?P<machine>Sewer|Sewage Composter) at \[(?P<x>-?\d+), (?P<y>-?\d+), (?P<z>-?\d+)\]: "
COMMON = (
    r"(?P<energy>-?\d+)/(?P<efe>-?\d+) FE, "
    r"tank (?P<tank>[\w:]+) (?P<tank_mb>-?\d+)/(?P<tank_cap>-?\d+) mB, "
    r"capability (?P<cap_fluid>\S+) (?P<cap_mb>-?\d+) mB, "
)
SEWER = re.compile(
    PREFIX + COMMON +
    r"animals (?P<animals>-?\d+), "
    r"progress (?P<progress>-?\d+)/(?P<bar>-?\d+), "
    r"facing (?P<facing>\S+), "
    r"augments speed (?P<speed>-?\d+) efficiency (?P<efficiency>\d+\.\d+), t=(?P<t>-?\d+)$"
)
COMPOSTER = re.compile(
    PREFIX + COMMON +
    r"output (?P<output>.*?), "
    r"progress (?P<progress>-?\d+)/(?P<bar>-?\d+), "
    r"facing (?P<facing>\S+), "
    r"augments speed (?P<speed>-?\d+) efficiency (?P<efficiency>\d+\.\d+), t=(?P<t>-?\d+)$"
)


def main() -> int:
    line = sys.argv[1] if len(sys.argv) > 1 else ""
    match = SEWER.search(line) or COMPOSTER.search(line)
    if not match:
        print("ok=0")
        return 0
    print("ok=1")
    for key, value in match.groupdict().items():
        print(f"SW_{key.upper()}={shlex.quote(value)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
