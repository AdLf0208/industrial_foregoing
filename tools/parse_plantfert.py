#!/usr/bin/env python3
"""Parse a `/industrialforegoing info` line of a **plant fertilizer** (slice 16).

Called by `tools/e2e.sh` as `eval "$(python3 tools/parse_plantfert.py "$LINE")"`: one shell-quoted
`NAME=value` per field and `ok=1`, or `ok=0` for another machine's line. The pointed block's crop
`age` is part of the line, so the in-game leg asserts the bonemeal's exact step from the game's own
readout as well as from `data get block`.
"""
import re
import shlex
import sys

PATTERN = re.compile(
    r"Plant Fertilizer at \[(?P<x>-?\d+), (?P<y>-?\d+), (?P<z>-?\d+)\]: "
    r"(?P<energy>-?\d+)/(?P<efe>-?\d+) FE, "
    r"input (?P<input>.*?), "
    r"progress (?P<progress>-?\d+)/(?P<bar>-?\d+), "
    r"pointing \[(?P<px>-?\d+), (?P<py>-?\d+), (?P<pz>-?\d+)\], "
    r"crop age (?P<crop_age>\S+), "
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
        print(f"PF_{key.upper()}={shlex.quote(value)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
