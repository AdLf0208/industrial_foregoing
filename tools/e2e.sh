#!/usr/bin/env bash
# In-game check for Industrial Foregoing (slices 1-2): the Water Condensator
# condenses water from the FE in its buffer, one 100 mB per water source block
# beside it (half, for free, without power), and nothing below two sources; its
# carried recipe crafts in a plain crafting table; and a machine broken with FE
# and water in it drops an item that puts them back.
#
# Every mod command is typed with `ctl chat "/cmd"` because `ctl command` runs
# the console path and bypasses the native `perform_command` hook.
#
# The End, in the void at x=200 (away from the central island, its dragon egg
# and the ambient item entities of a generated level). The machine stands at
# x=202 on the stone floor; the recipe leg uses a crafting table on its own
# platform at x=211, z=10 so the machine's column cannot hide it.
#
# Each leg resets the buffer, the tank and the bar through the harness
# commands, then forces the bar to 1 and waits two ticks: with the bar's
# cooldown (10 ticks when powered, 50 when not, 100 when idle) only the one
# attempt can run inside that window, so the numbers are exact.
#
# The package is one flat `.hmod` (code and content together), so the game must
# be one the launcher built with it — no `--dev-mod`, no embedded copy:
#
#   eval "$(ports/tools/launcher-game.sh env industrialforegoing)"
#   build-slot.sh run --job e2e-industrialforegoing --port ports/industrialforegoing \
#       --step e2e -- bash ports/industrialforegoing/tools/e2e.sh
#
# The **control leg** is the same script on `GAME=$CONTROL_GAME`: the same
# Nightly, unmodified, with no `libindustrialforegoing.so` and no
# `horizon-native-mods/industrialforegoing/`. It fails at the first assertion
# below (the machine item does not register), which is what makes the modded
# run evidence.
#
# SELFTEST=1 flips leg B's 200 mB assertion (it must FAIL), proving the
# scenario can fail. SELFTEST=contents does the same for leg L: it asserts
# 5,001 mB come back from the broken machine's item, so the run must fail at
# that assertion — the in-game counterfactual for the contents family.
set -Eeuo pipefail
export LC_ALL=C
[ -z "${IF_DEBUG:-}" ] || set -x

CLONE=${CLONE:-/home/adlf/Desktop/all_the_mods_10_rust_rewrite}
PORT=$CLONE/ports/industrialforegoing
CTL=${CTL:-}
GAME=${GAME:-}
RES=${RES:-${HORIZON_RES:-/home/adlf/Desktop/horizon/.local/horizon-client/26.3}}
SOCK=${SOCK:-/tmp/opencode/industrialforegoing-e2e.sock}
SELFTEST=${SELFTEST:-0}
KEEP=${KEEP:-0}
WORLD=IndustrialForegoingE2E
# Where the machine stands: the End's void, far from the central island.
X0=200
SX=$((X0 + 2))
FE_CAP=10000
WATER_CAP=16000

[ -n "$CTL" ] || {
    echo "CTL is unset: eval \"\$(ports/tools/launcher-game.sh env industrialforegoing)\"" >&2
    exit 1
}
[ -n "$GAME" ] || {
    echo "GAME is unset: the SDK package is compiled into the game by the launcher." >&2
    echo "  ports/tools/launcher-game.sh build industrialforegoing" >&2
    echo "  eval \"\$(ports/tools/launcher-game.sh env industrialforegoing)\"" >&2
    exit 1
}
[ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }

SCRATCH=$(mktemp -d /tmp/opencode/industrialforegoing-home.XXXXXX)
export HORIZON_HOME=$SCRATCH/game
export HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data
export XDG_CONFIG_HOME=$SCRATCH/config
export HORIZON_NO_AUDIO=1
mkdir -p "$(dirname "$SOCK")" "$XDG_CONFIG_HOME"
rm -f "$SOCK"

ctl() { "$CTL" --socket "$SOCK" --timeout 180 "$@"; }
say() { ctl chat "/$1" >/dev/null; }
fail() { echo "FAIL: $*" >&2; ctl logs --lines 60 >&2 || true; exit 1; }
on_error() { echo "FAIL: line $1: '$2' exited $?" >&2; ctl logs --lines 40 >&2 || true; }
trap 'on_error "$LINENO" "$BASH_COMMAND"' ERR

cleanup() {
    local status=$?
    ctl quit >/dev/null 2>&1 || true
    sleep 1
    if [ "$KEEP" = 1 ]; then
        echo "kept scratch: $SCRATCH" >&2
    else
        rm -rf "$SCRATCH" 2>/dev/null || true
    fi
    # Last line of every run: `check-e2e-logs.py` reads it as the verdict.
    echo "EXIT:$status"
}
trap cleanup EXIT

launch() { # launch [WORLD]
    local world=${1:-$WORLD}
    rm -f "$SOCK"
    cd "$SCRATCH"
    "$CTL" --socket "$SOCK" launch --game "$GAME" --world "$world" --seed 20261105 --creative \
        --offscreen --wait --resources "$RES" >/dev/null
    ctl wait-ready 120
    ctl wait 40
}

block_name() { ctl block "$1" "$2" "$3" | jq -r '.name // .block // empty'; }

# The End's void at X0: a stone floor and clear air above it. The player is
# teleported first, so the chunks the fills touch are loaded. The floor is laid
# before the machine is placed: a player standing over the void falls.
arena() {
    ctl command "execute in minecraft:the_end run tp @s $X0.5 104 0.5" >/dev/null
    local loaded=false
    for _ in $(seq 1 120); do
        ctl wait 5 >/dev/null
        if [ "$(ctl block "$X0" 100 0 | jq -r '.loaded // false')" = true ]; then
            loaded=true
            break
        fi
    done
    [ "$loaded" = true ] || fail "the End arena chunk never loaded"
    ctl command "execute in minecraft:the_end run fill $((X0 - 10)) 100 -14 $((X0 + 6)) 100 14 minecraft:stone" >/dev/null
    ctl wait 6 >/dev/null
    ctl command "execute in minecraft:the_end run fill $((X0 - 10)) 101 -14 $((X0 + 6)) 108 14 minecraft:air" >/dev/null
    ctl wait 6 >/dev/null
    ctl command "execute in minecraft:the_end run kill @e[type=!player]" >/dev/null
    ctl command "execute in minecraft:the_end run tp @s $X0.5 101 0.5" >/dev/null
    ctl wait 20
    [ "$(block_name "$X0" 100 0)" = "minecraft:stone" ] || fail "the arena floor is not stone"
    [ "$(block_name "$X0" 101 0)" = "minecraft:air" ] || fail "the arena air above the floor is not air"
}

# `/industrialforegoing info X Y Z`: the line, then the parsed fields in
# ENERGY, WATER, SOURCES, PROGRESS, T.
info() { # info X Y Z
    say "industrialforegoing info $1 $2 $3"
    ctl wait 12 >/dev/null
    INFO_LINE=$(ctl messages 12 | jq -r '.chat[]' | grep -m1 'Water Condensator at' || true)
    [ -n "$INFO_LINE" ] || fail "no /industrialforegoing info reply"
    eval "$(python3 "$PORT/tools/parse_info.py" "$INFO_LINE")"
    [ "${ok:-0}" = 1 ] || fail "the info line did not parse: $INFO_LINE"
    echo "  $INFO_LINE" >&2
}

# Put N water source blocks on the machine's sides (east, west, south, north,
# up, down — in that order) and clear the rest. The down one is the floor cell,
# restored to stone when it is not a source.
#
# The shell around the machine is cleared first: water from an earlier leg flows
# back into a face within a tick, and a face holding water it did not place makes
# the source count (and therefore the water condensed) wrong. The machine's own
# cell and the floor under it are never touched.
sources_n() { # sources_n N
    local want=$1 i=0 off
    ctl command "execute in minecraft:the_end run fill $((SX - 1)) 101 -3 $((SX - 1)) 103 3 minecraft:air" >/dev/null
    ctl command "execute in minecraft:the_end run fill $((SX + 1)) 101 -3 $((SX + 1)) 103 3 minecraft:air" >/dev/null
    ctl command "execute in minecraft:the_end run fill $((SX - 1)) 101 -1 $((SX + 1)) 103 -1 minecraft:air" >/dev/null
    ctl command "execute in minecraft:the_end run fill $((SX - 1)) 101 1 $((SX + 1)) 103 1 minecraft:air" >/dev/null
    ctl command "execute in minecraft:the_end run fill $((SX - 1)) 102 -1 $((SX + 1)) 103 1 minecraft:air" >/dev/null
    for off in "203 101 0" "201 101 0" "202 101 1" "202 101 -1" "202 102 0" "202 100 0"; do
        i=$((i + 1))
        if [ "$i" -le "$want" ]; then
            ctl command "execute in minecraft:the_end run setblock $off minecraft:water" >/dev/null
        elif [ "$off" = "202 100 0" ]; then
            ctl command "execute in minecraft:the_end run setblock $off minecraft:stone" >/dev/null
        fi
    done
    ctl wait 8 >/dev/null
}

# One attempt with the given state: park the bar at 100 first (so no attempt can
# fire while the tank and buffer are being reset), set the sources, the buffer
# and the tank, then force the bar to 1 and wait just long enough for exactly
# one attempt (the shortest cooldown is 10 ticks). Leaves ENERGY, WATER,
# SOURCES, PROGRESS and T set.
attempt() { # attempt SOURCES ENERGY WATER
    say "industrialforegoing progress $SX 101 0 100"
    sources_n "$1"
    say "industrialforegoing energy $SX 101 0 $2"
    say "industrialforegoing water $SX 101 0 $3"
    ctl wait 2 >/dev/null
    say "industrialforegoing progress $SX 101 0 1"
    ctl wait 2 >/dev/null
    info "$SX" 101 0
}

expect() { # expect NAME ACTUAL WANT
    [ "$2" = "$3" ] || fail "$1 is $2, expected $3"
}

# Put one machine item in the player's first hotbar slot and select it: `give`
# may fill a main-inventory slot that `select ITEM` cannot reach.
hold_machine() {
    ctl command "item replace entity @s hotbar.0 with industrialforegoing:water_condensator 1" >/dev/null
    ctl wait 4 >/dev/null
    ctl select 1 >/dev/null
    ctl wait 2 >/dev/null
}

# Place the machine at X Y Z from the held item and check the block appeared.
place_machine() { # place_machine X Y Z
    local x=$1 y=$2 z=$3
    ctl command "execute in minecraft:the_end run tp @s $((x - 2)).5 $y $z.5" >/dev/null
    ctl wait 6 >/dev/null
    hold_machine
    place_held "$x" "$y" "$z"
}

# Place the machine that is already in the hotbar (no `/give`): the drop a break leaves
# behind is the only source of that item, so this is what proves the item carries the
# machine's contents.
place_held() { # place_held X Y Z
    local x=$1 y=$2 z=$3
    ctl command "execute in minecraft:the_end run tp @s $((x - 2)).5 $y $z.5" >/dev/null
    ctl wait 6 >/dev/null
    ctl look-at-block "$x" $((y - 1)) "$z" up >/dev/null 2>&1 \
        || fail "cannot aim at the floor under $x,$y,$z"
    # The rotation reaches the server a tick after the aim: placing at once would
    # use the previous yaw (and the machine would face the wrong way).
    ctl wait 6 >/dev/null
    ctl place "$x" "$y" "$z" --item industrialforegoing:water_condensator >/dev/null \
        || fail "the held machine did not place at $x,$y,$z"
    ctl wait 6 >/dev/null
    [ "$(block_name "$x" "$y" "$z")" = "industrialforegoing:water_condensator" ] \
        || fail "the held machine did not place at $x,$y,$z (got '$(block_name "$x" "$y" "$z")')"
}

echo "== launch (The End arena, the one package carries content and code) =="
launch
arena
ctl command "item replace entity @s hotbar.0 with industrialforegoing:water_condensator 1" >/dev/null
ctl wait 4 >/dev/null
ctl inventory | jq -e '[.hotbar[], .main[] | select(.item == "industrialforegoing:water_condensator")] | length >= 1' >/dev/null \
    || fail "the machine item did not register: $(ctl inventory | jq -c '{hotbar,main}')"

echo "== A: a machine placed from its item is empty, with no sources =="
place_machine "$SX" 101 0
info "$SX" 101 0
expect "the buffer" "$ENERGY" 0
expect "the tank" "$WATER" 0
expect "the sources" "$SOURCES" 0
expect "the bar's maximum" "$PROGRESS_MAX" 100
# The player placed it from the west, looking east: Titanium's FOUR_WAY turns
# the machine's face toward its placer.
expect "the facing" "$FACING" "west"
echo "  block $(block_name "$SX" 101 0), $ENERGY/$FE_CAP FE, water $WATER/$WATER_CAP mB, facing $FACING"

echo "== B: two sources and 5,000 FE: 200 mB for 1,000 FE, cooldown 10 =="
attempt 2 5000 0
expect "the sources" "$SOURCES" 2
expect "the water" "$WATER" 200
expect "the buffer" "$ENERGY" 4000
if [ "$SELFTEST" = 1 ]; then
    echo "(selftest: asserting 201 mB)"
    expect "the water" "$WATER" 201
fi
# The chat round trip costs a few ticks, so the bar is a little below 10; the
# point is that it is the 10-tick powered cooldown, not 50 or 100.
[ "$PROGRESS" -le 10 ] && [ "$PROGRESS" -ge 1 ] \
    || fail "the powered cooldown is $PROGRESS, expected 1..10 of 100"
echo "  two sources: +$WATER mB, $ENERGY FE left, bar $PROGRESS/$PROGRESS_MAX"

echo "== C: one source condenses nothing (the rule is two or more) =="
attempt 1 5000 0
expect "the sources" "$SOURCES" 1
expect "the water" "$WATER" 0
expect "the buffer" "$ENERGY" 5000
echo "  one source: +0 mB, the buffer untouched"

echo "== D: no sources condense nothing =="
attempt 0 5000 0
expect "the sources" "$SOURCES" 0
expect "the water" "$WATER" 0
expect "the buffer" "$ENERGY" 5000

echo "== E: without 1,000 FE two sources give half, for free, cooldown 50 =="
attempt 2 999 0
expect "the sources" "$SOURCES" 2
expect "the water" "$WATER" 100
expect "the buffer" "$ENERGY" 999
[ "$PROGRESS" -le 50 ] && [ "$PROGRESS" -ge 1 ] \
    || fail "the unpowered cooldown is $PROGRESS, expected 1..50 of 100"
echo "  999 FE: +$WATER mB for free, bar $PROGRESS/$PROGRESS_MAX"

echo "== F: three sources condense 300 mB =="
attempt 3 5000 0
expect "the sources" "$SOURCES" 3
expect "the water" "$WATER" 300
expect "the buffer" "$ENERGY" 4000

echo "== G: six sources condense 600 mB =="
attempt 6 5000 0
expect "the sources" "$SOURCES" 6
expect "the water" "$WATER" 600
expect "the buffer" "$ENERGY" 4000
echo "  six sources: +$WATER mB, $ENERGY FE left"

echo "== H: a nearly full tank stops at 16,000 mB and the FE is still spent =="
attempt 2 5000 15900
expect "the water" "$WATER" 16000
expect "the buffer" "$ENERGY" 4000
echo "  tank $WATER/$WATER_CAP mB, the operation still cost 1,000 FE"

echo "== I: the tank is a real fluid handler: /drain takes water out =="
sources_n 0
say "industrialforegoing water $SX 101 0 1000"
ctl wait 4 >/dev/null
say "industrialforegoing drain $SX 101 0 400"
ctl wait 12 >/dev/null
DRAIN_LINE=$(ctl messages 12 | jq -r '.chat[]' | grep -m1 'drained' || true)
echo "  $DRAIN_LINE" >&2
case "$DRAIN_LINE" in
    *"drained 400 mB of minecraft:water"*) ;;
    *) fail "the tank refused a real drain: $DRAIN_LINE" ;;
esac
info "$SX" 101 0
expect "the water after the drain" "$WATER" 600

echo "== J: left alone, the machine keeps condensing on its own bar =="
attempt 2 5000 0
expect "the water after one attempt" "$WATER" 200
ctl wait 40 >/dev/null
info "$SX" 101 0
[ "$WATER" -ge 400 ] || fail "only $WATER mB after 40 unattended ticks, expected at least 400"
[ "$ENERGY" -le 4000 ] || fail "$ENERGY FE left after 40 unattended ticks, expected at most 4000"
echo "  unattended: $WATER mB and $ENERGY FE left after 40 more ticks"

echo "== K: the carried recipe crafts the machine from the tags' vanilla fallbacks =="
# Its own platform, away from the machine and its water: a table beside the running
# machine would sit in the crosshair of its own block column.
ctl command "execute in minecraft:the_end run fill 208 100 6 214 100 12 minecraft:stone" >/dev/null
ctl command "execute in minecraft:the_end run fill 208 101 6 214 104 12 minecraft:air" >/dev/null
ctl command "execute in minecraft:the_end run setblock 211 101 10 minecraft:crafting_table" >/dev/null
ctl command "execute in minecraft:the_end run tp @s 210.5 101 10.5" >/dev/null
ctl wait 12 >/dev/null
[ "$(block_name 211 101 10)" = "minecraft:crafting_table" ] \
    || fail "the crafting table did not place: '$(block_name 211 101 10)'"
python3 "$PORT/tools/craft_recipe.py" "$SOCK" "$CTL" 211 101 10 \
    || fail "the machine's carried recipe did not craft"

echo "== L: a broken machine's item carries its FE and water, and placing it restores them =="
# No sources, so the restored machine cannot run while it is being read.
sources_n 0
say "industrialforegoing progress $SX 101 0 100"
say "industrialforegoing energy $SX 101 0 7000"
say "industrialforegoing water $SX 101 0 5000"
ctl wait 4 >/dev/null
info "$SX" 101 0
expect "the buffer before the break" "$ENERGY" 7000
expect "the tank before the break" "$WATER" 5000
# The drop must be the only copy of the machine: clear the inventory, then break it
# in survival — a creative break destroys a block without dropping it, and the
# iron-block template needs a pickaxe for its drops.
ctl command "gamemode survival @s" >/dev/null
ctl command "clear @s" >/dev/null
ctl command "item replace entity @s hotbar.0 with minecraft:iron_pickaxe 1" >/dev/null
ctl wait 6 >/dev/null
ctl select 1 >/dev/null
ctl command "execute in minecraft:the_end run tp @s $((SX - 2)).5 101 0.5" >/dev/null
ctl wait 8 >/dev/null
# `ctl break` aims at a visible face itself; the floor under the machine is hidden by it.
BROKEN=$(ctl break "$SX" 101 0) || fail "breaking the machine failed"
echo "  break: $(echo "$BROKEN" | jq -c '{block, drops, picked_up, items_left_on_ground}')"
[ "$(echo "$BROKEN" | jq -r '.block')" = "minecraft:air" ] || fail "the machine did not break"
[ "$(echo "$BROKEN" | jq -r '.drops["industrialforegoing:water_condensator"] // 0')" -ge 1 ] \
    || fail "the broken machine dropped no item: $(echo "$BROKEN" | jq -c '.drops')"
# The item lies where the machine stood: stand on the drop so it is picked up
# (vanilla's pickup radius is about one block).
ctl command "execute in minecraft:the_end run tp @s $SX.5 101 0.5" >/dev/null
ctl wait 20 >/dev/null
HELD=$(ctl inventory | jq -r '.counts["industrialforegoing:water_condensator"] // 0')
[ "$HELD" -ge 1 ] || fail "the dropped machine was not picked up (counts $HELD)"
ctl select industrialforegoing:water_condensator >/dev/null 2>&1 || ctl select 1 >/dev/null
ctl wait 3 >/dev/null
place_held "$SX" 101 0
info "$SX" 101 0
expect "the buffer the item carried" "$ENERGY" 7000
expect "the tank the item carried" "$WATER" 5000
if [ "$SELFTEST" = contents ]; then
    echo "(selftest: asserting 5,001 mB came back)"
    expect "the restored tank" "$WATER" 5001
fi
echo "  the broken machine came back with $ENERGY/$FE_CAP FE and $WATER/$WATER_CAP mB"


echo "PASS: a placed machine is empty with no sources and faces its placer; two sources condense 200 mB for exactly 1,000 FE and one condenses nothing; without 1,000 FE two sources give half for free; three and six sources give 300 and 600 mB; a nearly full tank stops at 16,000 mB while still spending the FE; the declared tank drains 400 mB of water through the fluid handler; and left alone the machine keeps condensing on its own 10-tick bar; the carried recipe crafts one machine from its declared ingredients and consumes them, while a wrong grid stays empty; and a machine broken with 7,000 FE and 5,000 mB comes back with exactly those numbers on the item it dropped"
