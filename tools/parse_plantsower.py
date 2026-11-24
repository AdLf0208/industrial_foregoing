#!/usr/bin/env python3
"""Parse a `/industrialforegoing info` line of a **plant sower** (slice 18).

Called by `tools/e2e.sh` as `eval "$(python3 tools/parse_plantsower.py "$LINE")"`: one
shell-quoted `NAME=value` per field and `ok=1`, or `ok=0` for another machine's line.
"""
import re
import shlex
import sys

PATTERN = re.compile(
    r"Plant Sower at \[(?P<x>-?\d+), (?P<y>-?\d+), (?P<z>-?\d+)\]: "
    r"(?P<energy>-?\d+)/(?P<efe>-?\d+) FE, "
    r"progress (?P<progress>-?\d+)/(?P<bar>-?\d+), "
    r"pointing \[(?P<px>-?\d+), (?P<py>-?\d+), (?P<pz>-?\d+)\], "
    r"soil (?P<soil>\S+), "
    r"input (?P<input>.*?), "
    r"facing (?P<facing>\S+), "
    r"augments speed (?P<speed>-?\d+) efficiency (?P<efficiency>\d+\.\d+), t=(?P<t>-?\d+)$"
)


def count_in(items: str, wanted: str) -> str:
    """The count of `wanted` in a `data get block … Items` SNBT list, or `0`."""
    for entry in re.findall(r"\{[^{}]*\}", items):
        found = re.search(r'id: "([^"]+)"', entry)
        count = re.search(r"count: (\d+)", entry)
        if found and count and found.group(1) == wanted:
            return count.group(1)
    return "0"


def main() -> int:
    if len(sys.argv) > 1 and sys.argv[1] == "--count":
        print(count_in(sys.argv[2] if len(sys.argv) > 2 else "",
                       sys.argv[3] if len(sys.argv) > 3 else ""))
        return 0
    line = sys.argv[1] if len(sys.argv) > 1 else ""
    match = PATTERN.search(line)
    if not match:
        print("ok=0")
        return 0
    print("ok=1")
    for key, value in match.groupdict().items():
        print(f"PS_{key.upper()}={shlex.quote(value)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
