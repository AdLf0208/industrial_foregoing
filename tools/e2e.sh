#!/usr/bin/env bash
# In-game check for Industrial Foregoing (slices 1-4): the Water Condensator
# condenses water from the FE in its buffer, one 100 mB per water source block
# beside it (half, for free, without power), and nothing below two sources; the
# Core item set (four machine frames, three gears, plastic, 16 lenses) registers
# and reads back, a frame drops itself, and the carried recipes craft — the
# condensator from the tags, the pity frame, the three gears and the 16 lens
# recolours; a machine broken with FE and water in it drops an item that puts
# them back; and its two `titanium:augments` slots take a speed and an
# efficiency addon whose arithmetic the bar reads.
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
# SELFTEST=1 flips leg B's 200 mB assertion (it must FAIL), proving the scenario
# can fail; a run that fails there never reaches the augment legs, so
# SELFTEST=augments leaves leg B alone and flips leg O's four-a-tick fall
# instead — the augment family's own counterfactual. SELFTEST=contents does the
# same for leg L: it asserts 5,001 mB come back from the broken machine's item,
# so the run must fail at that assertion — the contents family's. SELFTEST=core
# is the Core item set's: leg U asserts the red lens arrived twice.
set -Eeuo pipefail
export LC_ALL=C
[ -z "${IF_DEBUG:-}" ] || set -x

CLONE=${CLONE:-/home/adlf/Desktop/all_the_mods_10_rust_rewrite}
PORT=$CLONE/ports/industrialforegoing
CTL=${CTL:-}
GAME=${GAME:-}
RES=${RES:-${HORIZON_RES:-/home/adlf/Desktop/horizon/.local/horizon-client/26.3}}
SOCK=${SOCK:-${E2E_TMP:-/tmp/opencode}/industrialforegoing-e2e.sock}
SELFTEST=${SELFTEST:-0}
KEEP=${KEEP:-0}
WORLD=IndustrialForegoingE2E
# Where the machine stands: the End's void, far from the central island.
X0=200
SX=$((X0 + 2))
# The dissolution chamber (slice 5): its own platform east of the condensator's.
CX=220
CHAMBER_ID=industrialforegoing:dissolution_chamber
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

# `E2E_TMP` is where a run keeps its world: `/tmp` is a 7.5 GB tmpfs, so the launcher
# exports a scratch root on the data disk and this honours it.
SCRATCH=$(mktemp -d "${E2E_TMP:-/tmp/opencode}"/industrialforegoing-home.XXXXXX)
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


# ---------------------------------------------------------------------------
# The dissolution chamber (slice 5)
# ---------------------------------------------------------------------------

# Its own platform east of the condensator's arena: stone at y=100, air above,
# and the barrel under the floor for the hopper leg.
chamber_arena() {
    ctl command "execute in minecraft:the_end run tp @s $((CX + 2)).5 104 0.5" >/dev/null
    local loaded=false
    for _ in $(seq 1 120); do
        ctl wait 5 >/dev/null
        if [ "$(ctl block "$CX" 100 0 | jq -r '.loaded // false')" = true ]; then
            loaded=true
            break
        fi
    done
    [ "$loaded" = true ] || fail "the chamber arena chunk never loaded"
    ctl command "execute in minecraft:the_end run fill $((CX - 6)) 100 -6 $((CX + 6)) 100 6 minecraft:stone" >/dev/null
    ctl command "execute in minecraft:the_end run fill $((CX - 6)) 101 -6 $((CX + 6)) 108 6 minecraft:air" >/dev/null
    ctl wait 8
    ctl command "execute in minecraft:the_end run tp @s $((CX - 2)).5 101 0.5" >/dev/null
    ctl wait 12
    [ "$(block_name "$CX" 100 0)" = "minecraft:stone" ] || fail "the chamber arena floor is not stone"
}

# Place the chamber **already in the hotbar** (the drop a break leaves), no `/give`: the
# only source of that item is the broken machine, which is what proves it carries the contents.
place_held_chamber() {
    ctl command "execute in minecraft:the_end run tp @s $((CX - 2)).5 101 0.5" >/dev/null
    ctl wait 6
    ctl look-at-block "$CX" 100 0 up >/dev/null 2>&1 || fail "cannot aim at the chamber's floor"
    ctl wait 6
    ctl place "$CX" 101 0 --item "$CHAMBER_ID" >/dev/null || fail "the carried chamber did not place"
    ctl wait 6
    [ "$(block_name "$CX" 101 0)" = "$CHAMBER_ID" ] \
        || fail "the carried chamber did not place at $CX (got '$(block_name "$CX" 101 0)')"
}

place_chamber() {
    ctl command "item replace entity @s hotbar.0 with $CHAMBER_ID 1" >/dev/null
    ctl wait 6
    ctl select 1 >/dev/null
    ctl command "execute in minecraft:the_end run tp @s $((CX - 2)).5 101 0.5" >/dev/null
    ctl wait 6
    ctl look-at-block "$CX" 100 0 up >/dev/null 2>&1 || fail "cannot aim at the chamber's floor"
    ctl wait 8
    ctl place "$CX" 101 0 --item "$CHAMBER_ID" >/dev/null || fail "the chamber did not place"
    ctl wait 6
    [ "$(block_name "$CX" 101 0)" = "$CHAMBER_ID" ] \
        || fail "the chamber did not place at $CX (got '$(block_name "$CX" 101 0)')"
}

# `/industrialforegoing info` of the chamber, parsed into the F_* variables.
cbar() {
    say "industrialforegoing info $CX 101 0"
    ctl wait 10
    CBAR_LINE=$(ctl messages 10 | jq -r '.chat[]' | grep -m1 'Dissolution Chamber at' || true)
    [ -n "$CBAR_LINE" ] || fail "no /industrialforegoing info reply for the chamber"
    eval "$(python3 "$PORT/tools/parse_chamber.py" "$CBAR_LINE")"
    [ "${ok:-0}" = 1 ] || fail "the chamber info line did not parse: $CBAR_LINE"
    echo "  $CBAR_LINE" >&2
}

# One `/industrialforegoing` command of the chamber at its position, its arguments in
# one string (`progress 0`, `fluid latex 2000`).
csay() {
    local sub=${1%% *} rest=""
    [ "$sub" != "$1" ] && rest=${1#* }
    say "industrialforegoing $sub $CX 101 0${rest:+ $rest}"
}

# Put one item into chamber input `slot` (0-7) through the container capability, as
# vanilla's `/item replace block` does: server-side, so the machine's own tick sees it.
feed() { # feed SLOT ITEM
    ctl command "item replace block $CX 101 0 container.$1 with $2 1" >/dev/null \
        || fail "could not put $2 into the chamber's input slot $1"
    ctl wait 2
}

# Clear every input slot the harness filled.
empty_inputs() {
    for slot in 0 1 2 3 4 5 6 7; do
        ctl command "item replace block $CX 101 0 container.$slot with minecraft:air" >/dev/null
    done
    ctl wait 4
}

# The three output slots, as `<item> x<count>` joined by commas (empty ones left out).
chamber_outputs() {
    ctl container "$CX" 101 0 | jq -r \
        '.slots[8:11] | map(select(. != null)) | map("\(.item) x\(.count)") | join(", ")'
}

# The items in chamber slot `n`, as `<item> x<count>` or `empty`.
chamber_slot() { # chamber_slot N
    ctl container "$CX" 101 0 | jq -r --argjson n "$1" \
        '.slots[$n] | if . == null then "empty" else "\(.item) x\(.count)" end'
}

# The recipe of the jar for the white lens: four glass panes and a white dye, in
# latex. Slots 0-3 the panes, slot 4 the dye.
feed_lens() {
    for slot in 0 1 2 3; do feed "$slot" minecraft:glass_pane; done
    feed 4 minecraft:white_dye
}

# The simple frame's eight items (its recipe's order, one per slot).
feed_simple_frame() {
    feed 0 industrialforegoing:plastic
    feed 1 industrialforegoing:machine_frame_pity
    feed 2 industrialforegoing:plastic
    feed 3 minecraft:nether_brick
    feed 4 minecraft:nether_brick
    feed 5 minecraft:iron_ingot
    feed 6 industrialforegoing:gold_gear
    feed 7 minecraft:iron_ingot
}

# The advanced frame's eight items: the plastics, the **crafted** simple frame, the
# netherite scraps, the gold ingots and the **crafted** diamond gear.
feed_advanced_frame() {
    feed 0 industrialforegoing:plastic
    feed 1 industrialforegoing:machine_frame_simple
    feed 2 industrialforegoing:plastic
    feed 3 minecraft:netherite_scrap
    feed 4 minecraft:netherite_scrap
    feed 5 minecraft:gold_ingot
    feed 6 industrialforegoing:diamond_gear
    feed 7 minecraft:gold_ingot
}

echo "== launch (The End arena, the one package carries content and code) =="
launch
# Peaceful: the survival legs (phase S's break, U's recolours, L and AE) must not race an
# enderman, and nothing here asserts a mob. A death mid-craft hides the assertion under it.
ctl command "difficulty peaceful" >/dev/null
arena
# The game refuses a carried recipe or advancement file it cannot read, and says
# so in its own log; neither may happen for this package's data.
LOAD_ERRORS=$(ctl logs --lines 400 | grep -c -E "\[load-items/ERROR\]|\[horizon-recipes/ERROR\]|\[horizon-advancements/ERROR\]" || true)
expect "the recipe/advancement load errors in the game's log" "${LOAD_ERRORS:-0}" 0
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

echo "== K: the carried recipes craft (the condensator from the tags, the frame, the gears) =="
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
    || fail "the carried shaped recipes did not craft"

echo "== R: every ModuleCore id of this slice registers and reads back =="
ctl command "gamemode creative @s" >/dev/null
ctl command "clear @s" >/dev/null
ctl wait 4 >/dev/null
# The 24 ids the slice declares: the four frames, the three gears, plastic and
# the 16 coloured lenses. Each is given and read back out of the inventory, so a
# declaration the game refused shows up here rather than as an absent item.
NEW_IDS="
industrialforegoing:machine_frame_pity
industrialforegoing:machine_frame_simple
industrialforegoing:machine_frame_advanced
industrialforegoing:machine_frame_supreme
industrialforegoing:iron_gear
industrialforegoing:gold_gear
industrialforegoing:diamond_gear
industrialforegoing:plastic
industrialforegoing:dryrubber
industrialforegoing:fluid_extractor
industrialforegoing:latex_processing_unit
industrialforegoing:white_laser_lens
industrialforegoing:orange_laser_lens
industrialforegoing:magenta_laser_lens
industrialforegoing:light_blue_laser_lens
industrialforegoing:yellow_laser_lens
industrialforegoing:lime_laser_lens
industrialforegoing:pink_laser_lens
industrialforegoing:gray_laser_lens
industrialforegoing:light_gray_laser_lens
industrialforegoing:cyan_laser_lens
industrialforegoing:purple_laser_lens
industrialforegoing:blue_laser_lens
industrialforegoing:brown_laser_lens
industrialforegoing:green_laser_lens
industrialforegoing:red_laser_lens
industrialforegoing:black_laser_lens
"
for id in $NEW_IDS; do
    ctl give "$id" 1 >/dev/null || fail "the game refused $id"
done
ctl wait 8 >/dev/null
COUNTS=$(ctl inventory | jq -c '.counts')
for id in $NEW_IDS; do
    [ "$(echo "$COUNTS" | jq -r --arg id "$id" '.[$id] // 0')" = 1 ] \
        || fail "$id did not read back out of the inventory: $(echo "$COUNTS" | jq -c --arg id "$id" '{($id): .[$id]}')"
done
echo "  the 24 Core ids read back: $(echo "$NEW_IDS" | wc -w) items"

# The four frames are blocks too: placed, read back by id, then cleared.
ctl command "execute in minecraft:the_end run fill 192 101 -4 196 101 0 minecraft:air" >/dev/null
i=0
for frame in machine_frame_pity machine_frame_simple machine_frame_advanced machine_frame_supreme; do
    ctl command "execute in minecraft:the_end run setblock $((192 + i)) 101 -2 industrialforegoing:$frame" >/dev/null
    i=$((i + 1))
done
ctl wait 6 >/dev/null
i=0
for frame in machine_frame_pity machine_frame_simple machine_frame_advanced machine_frame_supreme; do
    [ "$(block_name $((192 + i)) 101 -2)" = "industrialforegoing:$frame" ] \
        || fail "the placed $frame reads '$(block_name $((192 + i)) 101 -2)'"
    i=$((i + 1))
done
echo "  the four frames place and read back by id"

# An id this package does not register is refused, and the reply says so: a
# `ctl command` exits 0 on a refusal, so the text is what is asserted.
REFUSED=$(ctl give industrialforegoing:not_a_real_item 1 2>&1 || true)
case "$REFUSED" in
    *"unknown item industrialforegoing:not_a_real_item"*) ;;
    *) fail "an unregistered item id was not refused with the game's own message: $REFUSED" ;;
esac
REFUSED=$(ctl setblock 192 101 -6 industrialforegoing:not_a_real_block 2>&1 || true)
case "$REFUSED" in
    *'"ok":true'*) fail "an unregistered block id was accepted: $REFUSED" ;;
esac
[ "$(block_name 192 101 -6)" = "minecraft:air" ] \
    || fail "an unregistered block id changed the world: $(block_name 192 101 -6)"
echo "  an unregistered item and block id are refused"

echo "== S: a machine frame breaks and drops itself =="
# The frame is `Properties.ofFullCopy(IRON_BLOCK)` with `droppingSelf`: in
# survival, with a pickaxe, it must leave its own item — a loot table that does
# not resolve drops nothing (the createaddition lesson).
ctl command "gamemode survival @s" >/dev/null
ctl command "clear @s" >/dev/null
ctl command "item replace entity @s hotbar.0 with minecraft:iron_pickaxe 1" >/dev/null
ctl wait 6 >/dev/null
ctl select 1 >/dev/null
ctl command "execute in minecraft:the_end run tp @s 191.5 101 -2.5" >/dev/null
ctl wait 8 >/dev/null
FRAME_BREAK=$(ctl break 192 101 -2) || fail "breaking the machine frame failed"
echo "  break: $(echo "$FRAME_BREAK" | jq -c '{block, drops, picked_up, items_left_on_ground}')"
[ "$(echo "$FRAME_BREAK" | jq -r '.drops["industrialforegoing:machine_frame_pity"] // 0')" -ge 1 ] \
    || fail "a broken machine frame dropped nothing: $(echo "$FRAME_BREAK" | jq -c '.drops')"

echo "== U: the 16 lens recolours craft from the lens tag and c:dyes, counts before and after =="
# Survival: the inventory key opens the creative screen in creative mode, which
# has no 2x2 crafting grid.
ctl command "gamemode survival @s" >/dev/null
ctl wait 4 >/dev/null
python3 "$PORT/tools/recolor.py" "$SOCK" "$CTL" \
    $([ "$SELFTEST" = core ] && echo 2) \
    || fail "the lens recolours did not craft"

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

echo "== M-Q: the augment slots (titanium:augments) =="

# Stand two blocks west of the machine and aim at its face: a right-click is a
# `use_item_on` there, which is where the addon's seating hook runs.
aim_machine() {
    ctl command "execute in minecraft:the_end run tp @s $((SX - 2)).5 101 0.5" >/dev/null
    ctl wait 6 >/dev/null
    ctl look-at-block "$SX" 101 0 >/dev/null 2>&1 || fail "cannot aim at the machine at $SX"
    # The aim reaches the server a tick after the look: clicking at once would
    # use the previous yaw and miss the block.
    ctl wait 6 >/dev/null
}

# Give one addon, select it, aim at the machine and right-click it. COMPONENTS
# is the item argument's own `[titanium:augments={…}]` or empty for the item's
# declared default.
seat_addon() { # seat_addon ITEM COMPONENTS
    ctl command "give @s $1$2 1" >/dev/null
    ctl wait 8 >/dev/null
    ctl select "$1" >/dev/null 2>&1 || fail "the addon $1 is not in the hotbar"
    aim_machine
    ctl use-item >/dev/null || fail "the right-click with $1 on the machine failed"
    ctl wait 8 >/dev/null
}

# The machine's readout, read back within a couple of ticks: `/industrialforegoing
# info` answers at once, so a short wait is enough and the two reads of a rate
# window stay well inside one 100-tick bar. Sets everything `info` sets.
bar() { # bar X Y Z
    say "industrialforegoing info $1 $2 $3"
    ctl wait 2 >/dev/null
    local line
    line=$(ctl messages 6 | jq -r '.chat[]' | grep -m1 'Water Condensator at' || true)
    [ -n "$line" ] || fail "no /industrialforegoing info reply"
    eval "$(python3 "$PORT/tools/parse_info.py" "$line")"
    [ "${ok:-0}" = 1 ] || fail "the info line did not parse: $line"
    echo "  $line" >&2
}

# The bar's fall over a window the game itself measures: the readout carries the
# game time it was read at (`t=`), so the two reads give the rate exactly — the
# chat round trip cannot skew it. The window stays short so a bar finishing and
# resetting mid-window (25 ticks at speed 4) cannot be mistaken for a rate.
# Sets BAR_P1, BAR_T1, BAR_FELL and BAR_TICKS.
rate() { # rate LABEL
    say "industrialforegoing progress $SX 101 0 100"
    ctl wait 2 >/dev/null
    bar "$SX" 101 0
    BAR_P1=$PROGRESS
    BAR_T1=$T
    ctl wait 4 >/dev/null
    bar "$SX" 101 0
    BAR_FELL=$((BAR_P1 - PROGRESS))
    BAR_TICKS=$((T - BAR_T1))
    echo "  $1: progress $BAR_P1 -> $PROGRESS over $BAR_TICKS ticks" >&2
    [ "$BAR_TICKS" -gt 0 ] || fail "$1: the two reads carry the same game time"
    [ "$BAR_TICKS" -lt 20 ] \
        || fail "$1: a $BAR_TICKS-tick window can cross a finished bar and reset it"
}

held() { ctl inventory | jq -r --arg item "$1" '.counts[$item] // 0'; }

# No sources anywhere while the bar's rate is measured: an idle attempt resets a
# finished bar to 100, and a window shorter than the bar cannot reach zero.
sources_n 0
say "industrialforegoing energy $SX 101 0 0"

echo "== M: with no augment the bar falls exactly one a tick =="
info "$SX" 101 0
expect "the speed with an empty slot" "$SPEED" 1
expect "the efficiency with an empty slot" "$EFFICIENCY" "1.00"
expect "the bar's maximum" "$PROGRESS_MAX" 100
rate "empty slot"
[ "$BAR_FELL" = "$BAR_TICKS" ] \
    || fail "an empty slot fell $BAR_FELL over $BAR_TICKS ticks, expected one a tick"

echo "== N: an efficiency addon shortens the bar to 80 and the cooldown to 8 =="
# The parent's own tier 2 (`1 - tier * 0.1` = 0.8), seated into its declared slot.
seat_addon industrialforegoing:efficiency_addon_tier_2 ""
info "$SX" 101 0
expect "the efficiency the seated augment reads" "$EFFICIENCY" "0.80"
expect "the bar's scaled maximum" "$PROGRESS_MAX" 80
expect "the addon the seat took from the hand" "$(held industrialforegoing:efficiency_addon_tier_2)" 0
attempt 2 5000 200
expect "the water of one scaled attempt" "$WATER" 400
expect "the buffer one scaled attempt cost" "$ENERGY" 4000
[ "$PROGRESS" -le 8 ] && [ "$PROGRESS" -ge 1 ] \
    || fail "the scaled cooldown is $PROGRESS, expected 1..8 of 80"
echo "  efficiency 0.80: +$WATER mB for $((5000 - ENERGY)) FE, bar $PROGRESS/$PROGRESS_MAX"

echo "== O: a seated speed addon makes the bar fall four a tick (100 -> 60 in 10) =="
# Speed 4.0 is the ecosystem's tier 3 (mifa's speed_addon_3, the parent's own
# `1 + tier` at tier 3); the parent itself registers tiers 1 and 2 (2.0, 3.0).
sources_n 0
say "industrialforegoing energy $SX 101 0 0"
seat_addon industrialforegoing:speed_addon_tier_2 "[titanium:augments={Speed:4.0f}]"
info "$SX" 101 0
expect "the speed the seated augment reads" "$SPEED" 4
expect "the efficiency the other slot keeps" "$EFFICIENCY" "0.80"
expect "the addon the seat took from the hand" "$(held industrialforegoing:speed_addon_tier_2)" 0
# The brief's assertion, measured: four a tick is a full bar in 25 ticks and
# 100 -> 60 in ten. The readout's own game time makes the window exact.
rate "speed 4"
EXPECTED=$((4 * BAR_TICKS))
if [ "$SELFTEST" = 1 ] || [ "$SELFTEST" = augments ]; then
    echo "(selftest: asserting a fall of $((EXPECTED + 1)))"
    EXPECTED=$((EXPECTED + 1))
fi
[ "$BAR_FELL" = "$EXPECTED" ] \
    || fail "speed 4 fell $BAR_FELL over $BAR_TICKS ticks, expected $EXPECTED (four a tick)"
if [ "$BAR_P1" = 100 ] && [ "$BAR_TICKS" = 10 ]; then
    [ "$PROGRESS" = 60 ] \
        || fail "ten ticks of a full bar at speed 4 gave $PROGRESS, expected 60"
fi

echo "== P: a second speed addon is refused and stays in the hand =="
ctl command "give @s industrialforegoing:speed_addon_tier_1 1" >/dev/null
ctl wait 8 >/dev/null
ctl select industrialforegoing:speed_addon_tier_1 >/dev/null 2>&1 \
    || fail "the second addon is not in the hotbar"
aim_machine
ctl use-item >/dev/null || true
ctl wait 8 >/dev/null
info "$SX" 101 0
expect "the speed after the refused second addon" "$SPEED" 4
expect "the refused addon is still held" "$(held industrialforegoing:speed_addon_tier_1)" 1

echo "== Q: the augments travel on the machine's item =="
ctl command "gamemode survival @s" >/dev/null
ctl command "clear @s" >/dev/null
ctl command "item replace entity @s hotbar.0 with minecraft:iron_pickaxe 1" >/dev/null
ctl wait 6 >/dev/null
ctl select 1 >/dev/null
ctl command "execute in minecraft:the_end run tp @s $((SX - 2)).5 101 0.5" >/dev/null
ctl wait 8 >/dev/null
BROKEN=$(ctl break "$SX" 101 0) || fail "breaking the machine with its augments failed"
[ "$(echo "$BROKEN" | jq -r '.drops["industrialforegoing:water_condensator"] // 0')" -ge 1 ] \
    || fail "the augmented machine dropped no item: $(echo "$BROKEN" | jq -c '.drops')"
ctl command "execute in minecraft:the_end run tp @s $SX.5 101 0.5" >/dev/null
ctl wait 20 >/dev/null
ctl select industrialforegoing:water_condensator >/dev/null 2>&1 || ctl select 1 >/dev/null
ctl wait 3 >/dev/null
place_held "$SX" 101 0
info "$SX" 101 0
expect "the speed the item carried" "$SPEED" 4
expect "the efficiency the item carried" "$EFFICIENCY" "0.80"
expect "the scaled bar the item carried" "$PROGRESS_MAX" 80
echo "  the broken machine came back with speed $SPEED and efficiency $EFFICIENCY"




# ---------------------------------------------------------------------------
# The latex chain (slice 6): the fluid extractor and the latex processing unit
# ---------------------------------------------------------------------------

LX=240            # the fluid extractor's column (its log is at LX+1)
LXU=244           # the latex processing unit's column (hopper and furnace below)
EXTRACTOR_ID=industrialforegoing:fluid_extractor
UNIT_ID=industrialforegoing:latex_processing_unit

# Its own platform east of the chamber's: stone at y=100, air above, and a cleared
# shaft under LXU for the hopper and the furnace.
latex_arena() {
    ctl command "execute in minecraft:the_end run tp @s $((LX - 2)).5 104 0.5" >/dev/null
    local loaded=false
    for _ in $(seq 1 120); do
        ctl wait 5 >/dev/null
        if [ "$(ctl block "$LX" 100 0 | jq -r '.loaded // false')" = true ]; then
            loaded=true
            break
        fi
    done
    [ "$loaded" = true ] || fail "the latex arena chunk never loaded"
    ctl command "execute in minecraft:the_end run fill $((LX - 8)) 100 -8 $((LX + 12)) 100 8 minecraft:stone" >/dev/null
    ctl command "execute in minecraft:the_end run fill $((LX - 8)) 101 -8 $((LX + 12)) 108 8 minecraft:air" >/dev/null
    ctl wait 8
    ctl command "execute in minecraft:the_end run kill @e[type=!player]" >/dev/null
    ctl command "execute in minecraft:the_end run tp @s $((LX - 2)).5 101 0.5" >/dev/null
    ctl wait 16
    [ "$(block_name "$LX" 100 0)" = "minecraft:stone" ] || fail "the latex arena floor is not stone"
}

# Place any machine of this package from a `/give`n item, aimed at the floor under it so
# the placer's yaw is what the machine's `subfacing` reads.
place_any() { # place_any X Y Z ITEM
    local x=$1 y=$2 z=$3 item=$4
    ctl command "item replace entity @s hotbar.0 with $item 1" >/dev/null
    ctl wait 6
    ctl select 1 >/dev/null
    ctl command "execute in minecraft:the_end run tp @s $((x - 2)).5 $y $z.5" >/dev/null
    ctl wait 6
    ctl look-at-block "$x" $((y - 1)) "$z" up >/dev/null 2>&1 || fail "cannot aim at the floor under $x,$y,$z"
    ctl wait 8
    ctl place "$x" "$y" "$z" --item "$item" >/dev/null || fail "the $item did not place at $x,$y,$z"
    ctl wait 6
    [ "$(block_name "$x" "$y" "$z")" = "$item" ] \
        || fail "the $item did not place at $x,$y,$z (got '$(block_name "$x" "$y" "$z")')"
}

# Place a machine **already in the hotbar** at X Y Z (no `/give`): the drop a break leaves is the
# only source of that item, which is what proves it carries the machine's contents.
place_held_any() { # place_held_any X Y Z ITEM
    local x=$1 y=$2 z=$3 item=$4
    ctl command "execute in minecraft:the_end run tp @s $((x - 2)).5 $y $z.5" >/dev/null
    ctl wait 6
    ctl look-at-block "$x" $((y - 1)) "$z" up >/dev/null 2>&1 || fail "cannot aim at the floor under $x,$y,$z"
    ctl wait 6
    ctl place "$x" "$y" "$z" --item "$item" >/dev/null || fail "the carried $item did not place"
    ctl wait 6
    [ "$(block_name "$x" "$y" "$z")" = "$item" ] \
        || fail "the carried $item did not place at $x,$y,$z (got '$(block_name "$x" "$y" "$z")')"
}

# `/industrialforegoing info` of either latex-chain machine, parsed into the L_* variables.
#
# The answer is read from the **game's own log**, whose `[CHAT]` lines are complete and in order;
# the `ctl messages` window can return an earlier readout of the same machine when the reply lands
# a tick or two late (measured: the fresh line at t=3354, the stale one at t=3343 twice). The
# read waits for a line whose game time differs from the previous read's, bounded at 30 ticks.
linfo() { # linfo X Y Z
    local previous=${L_T:-} line="" t="" attempt
    say "industrialforegoing info $1 $2 $3"
    for attempt in $(seq 1 15); do
        ctl wait 2 >/dev/null
        line=$(ctl logs --lines 400 \
            | grep -E '\[CHAT\] (Fluid Extractor|Latex Processing Unit) at' \
            | tail -1 | sed 's/^.*\[CHAT\] //' || true)
        [ -n "$line" ] || continue
        t=$(echo "$line" | sed -n 's/.* t=\(-*[0-9][0-9]*\)$/\1/p')
        if [ -n "$t" ] && [ "$t" != "$previous" ]; then
            break
        fi
    done
    [ -n "$line" ] || fail "no /industrialforegoing info reply for the latex chain"
    eval "$(python3 "$PORT/tools/parse_latex.py" "$line")"
    [ "${ok:-0}" = 1 ] || fail "the latex machine's info line did not parse: $line"
    echo "  $line" >&2
}

# One `/industrialforegoing` command of a latex-chain machine at its position; every argument
# after the subcommand is passed on (`lsay X Y Z fluid latex 1000`). Taking only `$5` here
# truncated `fluid … latex 1000` to `fluid … latex`, which the dispatcher answered with
# "Unknown or incomplete command" — a harness bug, not the mod's.
lsay() { # lsay X Y Z SUB [ARGS...]
    local x=$1 y=$2 z=$3 sub=$4
    shift 4
    say "industrialforegoing $sub $x $y $z${*:+ $*}"
}

# The chat reply to the last `/industrialforegoing` command, matched by `pattern`.
reply() { # reply PATTERN
    local line="" attempt
    for attempt in $(seq 1 15); do
        ctl wait 2 >/dev/null
        line=$(ctl logs --lines 400 | grep -E '\[CHAT\] ' | sed 's/^.*\[CHAT\] //' \
            | grep -E "$1" | tail -1 || true)
        [ -n "$line" ] && break
    done
    echo "$line"
}

echo "== X: the dissolution chamber places, reads back and shows its own recipe table =="
chamber_arena
place_chamber
cbar
expect "the chamber's facing" "$F_FACING" west
expect "a fresh chamber's buffer" "$F_ENERGY" 0
expect "a fresh chamber's input tank" "$F_FIN" 0
expect "a fresh chamber's output tank" "$F_FO" 0
expect "a fresh chamber's bar" "$F_PROGRESS" 0
expect "the recipe a fresh chamber matches" "$F_RECIPE" none
# The machine's own recipe type, read from this package's data: the 23 files it
# carries (the jar's other 26 name an output this slice does not ship).
expect "the chamber recipes decoded" "$F_RECIPES" 23
expect "the chamber recipe files the game kept" "$F_RECIPES_TOTAL" 23
echo "  block $(block_name "$CX" 101 0), facing $F_FACING, recipes $F_RECIPES/$F_RECIPES_TOTAL"

echo "== Y: a mod fluid lives in the declared tank (the measured gap's route) =="
# `native_fluid_insert` takes only water and lava, so the port declares the fluid
# key and writes the entity's own `fluids` compound; the declared tank
# capability must then report the mod fluid back.
csay "fluid latex 2000"
ctl wait 8
cbar
expect "the tank's fluid" "$F_FLUID" "industrialforegoing:latex"
expect "the tank's amount" "$F_FIN" 2000
expect "the fluid the capability reports" "$F_CAPFLUID" "industrialforegoing:latex"
expect "the capability's amount" "$F_CAPMB" 2000
echo "  input $F_FIN mB of $F_FLUID, capability $F_CAPFLUID $F_CAPMB mB"

echo "== Z: the white lens recipe, a full 100-tick operation =="
# The jar's `white_laser_lens`: four glass panes + a white dye, 250 mB of latex, 100
# ticks. From a full buffer: 100 ticks x 90 FE = 9,000 FE, and the game's own clock
# says how long it took.
empty_inputs
csay "fluid latex 2000"
csay "energy 10000"
csay "progress 0"
feed_lens
cbar
expect "the recipe the lens grid matches" "$F_RECIPE" "industrialforegoing:dissolution_chamber/white_laser_lens"
expect "the bar's length" "$F_LENGTH" 100
Z_T0=$F_T
ctl wait 130
cbar
Z_TICKS=$((F_T - Z_T0))
[ "$Z_TICKS" -ge 100 ] || fail "the operation took $Z_TICKS ticks, expected at least 100"
expect "the buffer after the run (100 x 90 FE)" "$F_ENERGY" 1000
expect "the tank after the run (250 mB)" "$F_FIN" 1750
expect "the input slots after the run" "$F_ITEMS" 0
expect "the bar after the run" "$F_PROGRESS" 0
LENS_COUNT=1
if [ "$SELFTEST" = chamber ]; then
    echo "(selftest: asserting two lenses came out of the operation)"
    LENS_COUNT=2
fi
expect "the product" "$(chamber_slot 8)" "industrialforegoing:white_laser_lens x$LENS_COUNT"
echo "  $Z_TICKS ticks, 9,000 FE and 250 mB latex for $(chamber_slot 8)"

echo "== AA: with the tank empty the same grid does nothing (the fluid gate) =="
csay "fluid latex 0"
csay "energy 10000"
csay "progress 0"
feed_lens
ctl wait 40
cbar
expect "the recipe with an empty tank" "$F_RECIPE" none
expect "the bar with an empty tank" "$F_PROGRESS" 0
expect "the buffer with an empty tank" "$F_ENERGY" 10000
expect "the product with an empty tank" "$(chamber_slot 8)" "industrialforegoing:white_laser_lens x$LENS_COUNT"
# Filling the tank starts the same grid: the gate was the fluid, not the items. The
# bar then runs the recipe's 100 ticks (a 111-tick buffer), so 9,000 FE are spent.
csay "fluid latex 250"
ctl wait 130
cbar
expect "the buffer after the fluid arrives (100 x 90 FE)" "$F_ENERGY" 1000
echo "  no fluid: nothing ran; 250 mB later: $(chamber_slot 8)"

echo "== AB: a hopper under the chamber empties the output =="
ctl command "execute in minecraft:the_end run setblock $CX 100 0 minecraft:hopper[facing=down]" >/dev/null
ctl command "execute in minecraft:the_end run setblock $CX 99 0 minecraft:barrel" >/dev/null
ctl wait 40
BARREL=$(ctl container "$CX" 99 0 | jq -r '[.slots[]? | select(. != null) | .item] | join(",")')
case "$BARREL" in
    *industrialforegoing:white_laser_lens*) ;;
    *) fail "the hopper did not empty the chamber's output into the barrel: '$BARREL'" ;;
esac
expect "the chamber's output after the hopper" "$(chamber_slot 8)" empty
echo "  barrel: $BARREL"
# Put the floor back under the chamber for the frame legs.
ctl command "execute in minecraft:the_end run setblock $CX 100 0 minecraft:stone" >/dev/null
ctl wait 6

echo "== AC: the simple machine frame, the whole 300-tick recipe, at exactly 90 FE a tick =="
# A 300-tick run costs 27,000 FE, three buffers. The rate is measured the way the
# condensator measures its bar (two reads, the game's own clock), then the run is fed
# in windows short enough that the buffer never starves (a 10,000 FE buffer buys 111
# ticks), and the bar must climb from window to window without resetting.
empty_inputs
csay "fluid latex 1000"
csay "progress 0"
feed_simple_frame
cbar
expect "the recipe the frame grid matches" "$F_RECIPE" "industrialforegoing:dissolution_chamber/simple_machine_frame"
expect "the frame bar's length" "$F_LENGTH" 300
csay "energy 10000"
ctl wait 4
cbar
AC_E1=$F_ENERGY AC_T1=$F_T AC_P1=$F_PROGRESS
ctl wait 30
cbar
AC_E2=$F_ENERGY AC_T2=$F_T AC_P2=$F_PROGRESS
AC_TICKS=$((AC_T2 - AC_T1))
[ "$AC_TICKS" -gt 0 ] || fail "the two reads carry the same game time"
expect "the FE one tick takes (90 x $AC_TICKS)" "$((AC_E1 - AC_E2))" "$((90 * AC_TICKS))"
expect "the bar's rise over $AC_TICKS ticks" "$((AC_P2 - AC_P1))" "$AC_TICKS"
echo "  $AC_TICKS ticks: $((AC_E1 - AC_E2)) FE, bar $AC_P1 -> $AC_P2"
AC_LAST=$F_PROGRESS
for window in 1 2 3 4 5 6; do
    csay "energy 10000"
    ctl wait 60
    cbar
    if [ "$F_PROGRESS" -gt 0 ]; then
        [ "$F_PROGRESS" -gt "$AC_LAST" ] \
            || fail "the bar went from $AC_LAST to $F_PROGRESS in window $window: it reset"
        AC_LAST=$F_PROGRESS
    fi
    echo "  window $window: progress $F_PROGRESS/300, $F_ENERGY FE left"
    case "$(chamber_outputs)" in
        *"industrialforegoing:machine_frame_simple x1"*) break ;;
    esac
done
case "$(chamber_outputs)" in
    *"industrialforegoing:machine_frame_simple x1"*) ;;
    *) fail "the simple frame is not in the output: '$(chamber_outputs)'" ;;
esac
expect "the tank after the frame (1,000 - 250 mB)" "$F_FIN" 750
expect "the input slots after the frame" "$F_ITEMS" 0
echo "  the 300-tick frame: $(chamber_outputs), 250 mB latex"

echo "== AD: the advanced frame from #c:plastics, #c:gears/diamond and the simple frame =="
# The three ingredient families `mifa`'s own recipes name: `#c:plastics` (the
# carried tag), `#c:gears/diamond` (the gear phase K crafted) and
# `industrialforegoing:machine_frame/advanced`, made here from the frame AC made.
# The rate is measured as in AC (two reads, the game's own clock), then the run is
# finished from a bar one tick short; the output slots are read as a set, because
# AC's simple frame is still in slot 8.
empty_inputs
csay "fluid pink_slime 1000"
feed_advanced_frame
cbar
expect "the recipe the advanced grid matches" "$F_RECIPE" "industrialforegoing:dissolution_chamber/advanced_machine_frame"
expect "the advanced bar's length" "$F_LENGTH" 300
csay "energy 10000"
ctl wait 4
cbar
AD_E1=$F_ENERGY AD_T1=$F_T
ctl wait 30
cbar
AD_E2=$F_ENERGY AD_T2=$F_T
AD_TICKS=$((AD_T2 - AD_T1))
[ "$AD_TICKS" -gt 0 ] || fail "the two reads carry the same game time"
expect "the FE one tick takes on the advanced recipe" "$((AD_E1 - AD_E2))" "$((90 * AD_TICKS))"
csay "progress 299"
ctl wait 6
cbar
case "$(chamber_outputs)" in
    *"industrialforegoing:machine_frame_advanced x1"*) ;;
    *) fail "the advanced frame is not in the output: '$(chamber_outputs)'" ;;
esac
expect "the tank after the advanced frame (1,000 - 500 mB)" "$F_FIN" 500
expect "the input slots after the advanced frame" "$F_ITEMS" 0
echo "  #c:plastics + #c:gears/diamond + machine_frame/simple -> $(chamber_outputs), 500 mB pink slime"

echo "== AE: a broken chamber's item carries its buffer and fluid back =="
empty_inputs
ctl command "execute in minecraft:the_end run fill $((CX - 2)) 100 -2 $((CX + 2)) 100 2 minecraft:stone" >/dev/null
ctl wait 6
csay "fluid latex 1500"
csay "energy 4000"
ctl wait 6
cbar
expect "the tank before the break" "$F_FIN" 1500
expect "the buffer before the break" "$F_ENERGY" 4000
ctl command "gamemode survival @s" >/dev/null
ctl command "clear @s" >/dev/null
ctl command "item replace entity @s hotbar.0 with minecraft:iron_pickaxe 1" >/dev/null
ctl wait 6
ctl select 1 >/dev/null
ctl command "execute in minecraft:the_end run tp @s $((CX - 2)).5 101 0.5" >/dev/null
ctl wait 8
CHAMBER_BROKEN=$(ctl break "$CX" 101 0) || fail "breaking the chamber failed"
echo "  break: $(echo "$CHAMBER_BROKEN" | jq -c '{block, drops, picked_up}')"
[ "$(echo "$CHAMBER_BROKEN" | jq -r '.drops["industrialforegoing:dissolution_chamber"] // 0')" -ge 1 ] \
    || fail "the broken chamber dropped no item: $(echo "$CHAMBER_BROKEN" | jq -c '.drops')"
ctl command "execute in minecraft:the_end run tp @s $CX.5 101 0.5" >/dev/null
ctl wait 20
ctl select "$CHAMBER_ID" >/dev/null 2>&1 || ctl select 1 >/dev/null
ctl command "gamemode creative @s" >/dev/null
ctl wait 4
place_held_chamber
cbar
expect "the tank the item carried" "$F_FIN" 1500
expect "the buffer the item carried" "$F_ENERGY" 4000
echo "  the chamber came back with $F_ENERGY FE and $F_FIN mB of latex"


echo "== AF: the extractor works on the log behind it, for free without power =="
latex_arena
# The log goes down first: `RangeManager.RangeType.BEHIND` is the block opposite the
# machine's `subfacing`, and the machine faces its placer (west), so the log is east.
ctl command "execute in minecraft:the_end run setblock $((LX + 1)) 101 0 minecraft:oak_log" >/dev/null
ctl wait 4
place_any "$LX" 101 0 "$EXTRACTOR_ID"
linfo "$LX" 101 0
expect "the machine the line describes" "$L_KIND" EXTRACTOR
expect "the facing" "$L_FACING" west
expect "the log it sees" "$L_LOG" minecraft:oak_log
expect "the recipe it answers" "$L_RECIPE" "industrialforegoing:fluid_extractor/oak"
expect "the recipe files its own type carries" "$L_RECIPES" 9
expect "the recipe files the game kept" "$L_RECIPES_TOTAL" 9
expect "a fresh tank" "$L_LATEX" 0
lsay "$LX" 101 0 energy 0
lsay "$LX" 101 0 progress 1
ctl wait 4
linfo "$LX" 101 0
LATEX_FREE=$L_LATEX
if [ "$SELFTEST" = latex ]; then
    echo "(selftest: asserting three mB from an unpowered operation)"
    LATEX_FREE=$((LATEX_FREE + 1))
fi
expect "the latex one unpowered operation gives" "$LATEX_FREE" 2
expect "the buffer it left untouched" "$L_ENERGY" 0
echo "  an oak log: $L_LATEX mB for 0 FE, bar $L_PROGRESS/100"

echo "== AG: with 500 FE an operation gives three times the recipe =="
lsay "$LX" 101 0 energy 5000
lsay "$LX" 101 0 progress 1
ctl wait 4
linfo "$LX" 101 0
expect "the latex after a powered operation (2 + 6 mB)" "$L_LATEX" 8
expect "the buffer after a powered operation (5,000 - 500 FE)" "$L_ENERGY" 4500
[ "$L_PROGRESS" -ge 1 ] && [ "$L_PROGRESS" -le 40 ] \
    || fail "the powered cooldown is $L_PROGRESS, expected 1..40 of 100"
echo "  powered: +6 mB for 500 FE, bar $L_PROGRESS/100"

echo "== AH: a stone block behind it gives nothing (the counterfactual) =="
ctl command "execute in minecraft:the_end run setblock $((LX + 1)) 101 0 minecraft:stone" >/dev/null
ctl wait 4
linfo "$LX" 101 0
expect "the block it sees" "$L_LOG" minecraft:stone
expect "the recipe a stone answers" "$L_RECIPE" none
lsay "$LX" 101 0 energy 5000
lsay "$LX" 101 0 progress 1
ctl wait 6
linfo "$LX" 101 0
expect "the latex stone leaves" "$L_LATEX" 8
expect "the buffer stone leaves" "$L_ENERGY" 5000
# An idle attempt rests the bar at 100 and takes nothing; the read below lands a few
# ticks later, so the bar is somewhere between 80 and 100 rather than exactly 100.
[ "$L_PROGRESS" -ge 80 ] && [ "$L_PROGRESS" -le 100 ] \
    || fail "the bar a machine with no log rests at is $L_PROGRESS, expected 80..100 of 100"
echo "  stone behind it: $L_LATEX mB, $L_ENERGY FE, bar $L_PROGRESS/100"

echo "== AI: the eighth hit consumes the log and bursts 400 mB =="
ctl command "execute in minecraft:the_end run setblock $((LX + 1)) 101 0 minecraft:oak_log" >/dev/null
ctl wait 4
# Park the bar at 100 and the tank at a known amount first: the machine keeps working while
# the harness talks to it, and a finished bar between two commands would spend the roll (and
# a hit) before the assertion. Only the forced `progress 1` may run an operation.
lsay "$LX" 101 0 progress 100
lsay "$LX" 101 0 energy 5000
ctl wait 4
linfo "$LX" 101 0
BURST_BEFORE=$L_LATEX
HITS_REPLY=$(lsay "$LX" 101 0 extraction 7 || true; reply "has taken")
case "$HITS_REPLY" in
    *"has taken 7/8 hits"*) ;;
    *) fail "the hit counter did not take 7: $HITS_REPLY" ;;
esac
lsay "$LX" 101 0 roll 0
lsay "$LX" 101 0 progress 1
ctl wait 6
linfo "$LX" 101 0
expect "the hits after the consuming operation" "$L_HITS" 0
expect "the latex the operation gave (6 + 400 mB)" "$L_LATEX" "$((BURST_BEFORE + 406))"
expect "the buffer after the operation" "$L_ENERGY" 4500
expect "the block the consumed log left" "$(block_name "$((LX + 1))" 101 0)" minecraft:stripped_oak_log
DRAIN_LINE=$(lsay "$LX" 101 0 drain 400 || true; reply "drained")
case "$DRAIN_LINE" in
    *"drained 400 mB of industrialforegoing:latex"*) ;;
    *) fail "the tank refused the drain of the mod fluid it made: $DRAIN_LINE" ;;
esac
linfo "$LX" 101 0
expect "the tank after a 400 mB drain" "$L_LATEX" "$((BURST_BEFORE + 6))"
echo "  the eighth hit: $(block_name "$((LX + 1))" 101 0), $L_LATEX mB left after draining 400"

echo "== AJ: a roll over the break chance moves the counter not at all =="
ctl command "execute in minecraft:the_end run setblock $((LX + 1)) 101 0 minecraft:oak_log" >/dev/null
ctl wait 4
lsay "$LX" 101 0 progress 100
lsay "$LX" 101 0 energy 5000
ctl wait 4
linfo "$LX" 101 0
ROLL_BEFORE=$L_LATEX
lsay "$LX" 101 0 extraction 7
lsay "$LX" 101 0 roll 1
lsay "$LX" 101 0 progress 1
ctl wait 6
linfo "$LX" 101 0
expect "the hits a roll over the chance leaves" "$L_HITS" 7
expect "the block a roll over the chance leaves" "$(block_name "$((LX + 1))" 101 0)" minecraft:oak_log
expect "the latex the operation still gave" "$L_LATEX" "$((ROLL_BEFORE + 6))"
echo "  roll 1.0: no hit, the log stays, $L_LATEX mB"

echo "== AK: the unit takes latex and water, and makes dry rubber =="
place_any "$LXU" 101 0 "$UNIT_ID"
linfo "$LXU" 101 0
expect "the machine the line describes" "$L_KIND" UNIT
expect "a fresh unit's latex tank" "$L_LATEX" 0
expect "a fresh unit's water tank" "$L_WATER" 0
lsay "$LXU" 101 0 fluid latex 1000
ctl wait 4
# Water through the declared tank capability: the pin's insert takes water and lava.
FILL_REPLY=$(lsay "$LXU" 101 0 fill water 1000 || true; reply "accepted")
case "$FILL_REPLY" in
    *"accepted 1000 mB of minecraft:water"*) ;;
    *) fail "the water tank refused the pin's own fill: $FILL_REPLY" ;;
esac
linfo "$LXU" 101 0
expect "the unit's latex tank" "$L_LATEX" 1000
expect "the unit's water tank" "$L_WATER" 1000
expect "the latex the capability reports" "$L_CAP_FLUID" industrialforegoing:latex
# The same fill path with a mod fluid: zero moved, which is why the latex chain's own
# tanks are filled by `/industrialforegoing fluid` (the report's fluid-move answer).
LATEX_FILL=$(lsay "$LXU" 101 0 fill latex 100 || true; reply "accepted")
case "$LATEX_FILL" in
    *"accepted 0 mB of industrialforegoing:latex"*) ;;
    *) fail "the pin's fill accepted a mod fluid: $LATEX_FILL" ;;
esac
lsay "$LXU" 101 0 energy 10000
ctl wait 130
linfo "$LXU" 101 0
expect "the latex after one operation (1,000 - 750 mB)" "$L_LATEX" 250
expect "the water after one operation (1,000 - 500 mB)" "$L_WATER" 500
expect "the buffer after 100 ticks at 20 FE" "$L_ENERGY" 8000
expect "the bar after the operation" "$L_PROGRESS" 0
expect "the product" "$L_OUTPUT" "industrialforegoing:dryrubber x1"
echo "  $L_OUTPUT from 750 mB latex and 500 mB water, 2,000 FE; the mod fluid refused a fill"

echo "== AL: a hopper feeds a furnace, and the dry rubber smelts into plastic =="
ctl command "execute in minecraft:the_end run setblock $LXU 100 0 minecraft:hopper[facing=down]" >/dev/null
ctl command "execute in minecraft:the_end run setblock $LXU 99 0 minecraft:furnace" >/dev/null
ctl command "item replace block $LXU 99 0 container.1 with minecraft:coal 4" >/dev/null
ctl wait 300
FURNACE=$(ctl container "$LXU" 99 0)
FURNACE_OUT=$(echo "$FURNACE" | jq -r '.slots[2] | if . == null then "empty" else "\(.item) x\(.count)" end')
FURNACE_IN=$(echo "$FURNACE" | jq -r '.slots[0] | if . == null then "empty" else "\(.item) x\(.count)" end')
expect "the furnace's product" "$FURNACE_OUT" "industrialforegoing:plastic x1"
expect "the dry rubber the furnace consumed" "$FURNACE_IN" empty
linfo "$LXU" 101 0
expect "the unit's output after the hopper" "$L_OUTPUT" empty
echo "  the hopper moved the rubber into the furnace, which made $FURNACE_OUT"

echo "== AM: a broken unit carries its latex, water and buffer on its item =="
# Take the hopper and the furnace of the previous leg out first: a hopper under the machine
# would swallow the drop before the player can stand on it.
ctl command "execute in minecraft:the_end run setblock $LXU 100 0 minecraft:stone" >/dev/null
ctl command "execute in minecraft:the_end run setblock $LXU 99 0 minecraft:air" >/dev/null
ctl wait 6
# 400 mB of water is under the machine's 500 mB gate, so it stands idle and spends nothing:
# the numbers read before the break are the numbers the item must carry, exactly.
lsay "$LXU" 101 0 fluid latex 1500
lsay "$LXU" 101 0 fluid water 400
lsay "$LXU" 101 0 energy 4000
ctl wait 6
linfo "$LXU" 101 0
expect "the latex before the break" "$L_LATEX" 1500
expect "the water before the break" "$L_WATER" 400
expect "the buffer before the break" "$L_ENERGY" 4000
ctl command "gamemode survival @s" >/dev/null
ctl command "clear @s" >/dev/null
ctl command "item replace entity @s hotbar.0 with minecraft:iron_pickaxe 1" >/dev/null
ctl wait 6
ctl select 1 >/dev/null
ctl command "execute in minecraft:the_end run tp @s $((LXU - 2)).5 101 0.5" >/dev/null
ctl wait 8
UNIT_BROKEN=$(ctl break "$LXU" 101 0) || fail "breaking the unit failed"
echo "  break: $(echo "$UNIT_BROKEN" | jq -c '{block, drops, picked_up}')"
[ "$(echo "$UNIT_BROKEN" | jq -r ".drops[\"$UNIT_ID\"] // 0")" -ge 1 ] \
    || fail "the broken unit dropped no item: $(echo "$UNIT_BROKEN" | jq -c '.drops')"
ctl command "execute in minecraft:the_end run tp @s $LXU.5 101 0.5" >/dev/null
ctl wait 20
HELD_UNIT=$(ctl inventory | jq -r '.counts["industrialforegoing:latex_processing_unit"] // 0')
[ "$HELD_UNIT" -ge 1 ] || fail "the broken unit's item was not picked up (counts $HELD_UNIT)"
ctl select "$UNIT_ID" >/dev/null 2>&1 || ctl select 1 >/dev/null
ctl command "gamemode creative @s" >/dev/null
ctl wait 4
place_held_any "$LXU" 101 0 "$UNIT_ID"
linfo "$LXU" 101 0
expect "the latex the item carried" "$L_LATEX" 1500
expect "the water the item carried" "$L_WATER" 400
expect "the buffer the item carried" "$L_ENERGY" 4000
echo "  the unit came back with $L_ENERGY FE, $L_LATEX mB of latex and $L_WATER mB of water"

echo "PASS: a placed machine is empty with no sources and faces its placer; two sources condense 200 mB for exactly 1,000 FE and one condenses nothing; without 1,000 FE two sources give half for free; three and six sources give 300 and 600 mB; a nearly full tank stops at 16,000 mB while still spending the FE; the declared tank drains 400 mB of water through the fluid handler; and left alone the machine keeps condensing on its own 10-tick bar; the Core ids of these slices register and read back, the four frames place and read back, an unregistered item and block id are refused, and a frame broken in survival drops itself; the condensator crafts from the tags (#c:plastics, #c:gears/iron, machine_frame/pity) with the grid empty afterwards while slice 2's stand-ins stay empty, the pity frame and the three gears craft, and all 16 lens recolours craft from the lens tag and c:dyes with the player's counts before and after; a machine broken with 7,000 FE and 5,000 mB comes back with exactly those numbers on the item it dropped; a seated speed augment of 4 makes the bar fall four a tick where an empty slot falls one, a second speed augment is refused and stays in the hand, an efficiency augment of 0.8 shortens the bar to 80 with an 8-tick cooldown at the same 1,000 FE, and both augments travel on the machine's item; the dissolution chamber places and faces its placer, reads its own recipe type from this package's data (23 files), holds its own fluids in a declared tank the capability reports back, runs the white lens recipe in exactly 100 ticks for 9,000 FE and 250 mB of latex, does nothing with an empty tank and starts when 250 mB arrive, empties its output into a hopper and a barrel, runs the simple machine frame's 300 ticks at exactly 90 FE a tick, makes the advanced frame from #c:plastics, the crafted #c:gears/diamond gear and the simple frame, and a broken chamber carries its buffer and latex back on its item; and the latex chain: a fluid extractor works on the oak log behind it, giving 2 mB for free and 6 mB for 500 FE, nothing at all when the block behind it is stone, and — on its eighth hit — consuming the log into a stripped one and bursting 400 mB that drain back out of its declared tank, where a roll over its break chance moves nothing; a latex processing unit takes 1,000 mB of latex through the command path and 1,000 mB of water through the declared capability (which refuses the mod fluid, so no fluid move can fill it), turns 750 mB of latex and 500 mB of water into one dry rubber for 2,000 FE, a hopper feeds that rubber into a furnace which smelts it into one plastic, and a broken unit carries its latex, water and buffer back on its item"
