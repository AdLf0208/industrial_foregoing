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
# SELFTEST=laser flips BD's charge; SELFTEST=fluid flips BJ's 10 mB of ether;
# SELFTEST=slaughter flips CC's 200 mB of pink slime; SELFTEST=nether flips CL's
# 100 mB of lava (the Nether leg, slice 10); SELFTEST=crusher flips CO's
# `20 x experience` essence by one mB (the mob crusher, slice 11).
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
# The add-on's own jar: its `laser_drill_ore` file and biome tag are the surface slice 7
# unblocks. It is installed in the scratch world **before the run's opening**, because its
# biome tag table is built at world load.
SOULS_JAR=$CLONE/ports/_shared/licence-cache/industrial-foregoing-souls-1.21.1-1.10.7.jar
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
    # A reopened world can come back with the pause menu up (`pause_on_lost_focus`); with the
    # server paused the chat commands queue and no readout arrives (measured 2026-11-22: the
    # add-on leg's first post-reopen `info` worked, the next never answered).
    ctl resume >/dev/null 2>&1 || true
    wait_chat_roundtrip || fail "the player's chat path never answered a launch"
    ctl wait 40
}

# Wait until the player's chat path answers: a vanilla chat command whose reply is a new
# `[CHAT]` line. Required after a launch and after every **dimension change** — a `tp`
# across dimensions drops chat lines until the client has finished the transfer, which is
# what made slice 10's first runs fail phase A with "no fresh info reply" under load.
wait_chat_roundtrip() {
    local i
    for i in $(seq 1 30); do
        ctl chat "/time query gametime" >/dev/null 2>&1 || true
        # The ctl's own wait, not a log grep: it subscribes to the chat line and answers
        # only when a *new* one arrives, so a dropped send costs one retry, not a parse race.
        if ctl wait-for chat "The game time is" --timeout 10 >/dev/null 2>&1; then
            return 0
        fi
    done
    return 1
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
    wait_chat_roundtrip || fail "the chat path never answered after the End transfer"
    [ "$(block_name "$X0" 100 0)" = "minecraft:stone" ] || fail "the arena floor is not stone"
    [ "$(block_name "$X0" 101 0)" = "minecraft:air" ] || fail "the arena air above the floor is not air"
}

# `/industrialforegoing info X Y Z`: the line, then the parsed fields in
# ENERGY, WATER, SOURCES, PROGRESS, T.
info() { # info X Y Z
    local pattern='Water Condensator at .*, sources ' line="" best="" best_t=-999999 t cand source attempt
    local before_t=${INFO_T:-0}
    # A **new** readout from the newest line of **both** sources: the game's own `[CHAT]`
    # log (complete and ordered) and the live `ctl messages` window. Slice 10 measured the
    # log stopping at the launch while the window still answered, so either alone is a
    # flaky oracle; the line's own `t=` is what says it is newer than the previous read.
    for attempt in $(seq 1 40); do
        # Re-sent every attempt: a chat line is dropped while the client is finishing a
        # dimension transfer, and a readout is idempotent. The real sleep is a floor too —
        # a `wait 2` before the server ticks passes immediately.
        say "industrialforegoing info $1 $2 $3"
        ctl wait 2 >/dev/null
        sleep 0.2
        for source in logs messages; do
            if [ "$source" = logs ]; then
                cand=$(ctl logs --lines 400 | grep "\[CHAT\] $pattern" | tail -1 | sed 's/^.*\[CHAT\] //' || true)
            else
                cand=$(ctl messages 40 | jq -r '.chat[]' | grep -E "$pattern" | tail -1 || true)
            fi
            [ -n "$cand" ] || continue
            t=$(sed -n 's/.* t=\(-\?[0-9][0-9]*\)$/\1/p' <<<"$cand")
            [ -n "$t" ] || continue
            if [ "$t" -gt "$best_t" ]; then
                best=$cand
                best_t=$t
            fi
        done
        if [ -n "$best" ] && [ "$best_t" -gt "$before_t" ]; then
            line=$best
            break
        fi
    done
    INFO_T=$best_t
    INFO_LINE=$line
    [ -n "$INFO_LINE" ] || fail "no fresh /industrialforegoing info reply"
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

# Put the player in survival and **wait until the state says so**: a `gamemode` command
# runs a tick later, and a break that lands before it is a creative break (no drops). A
# loaded box needed the wait on 2026-11-22: phase L read `drops: {}` because the break
# raced the mode switch.
ensure_survival() {
    local mode attempt
    for attempt in $(seq 1 20); do
        ctl command "gamemode survival @s" >/dev/null
        ctl wait 2 >/dev/null
        mode=$(ctl state | jq -r '.gamemode // .game_mode // empty')
        case "$mode" in
            survival|0) return 0 ;;
        esac
    done
    fail "the player is still in mode '$mode', not survival, before the break"
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
    local before now attempt
    before=$(ctl logs --lines 400 | grep -c 'Dissolution Chamber at ' || true)
    say "industrialforegoing info $CX 101 0"
    for attempt in $(seq 1 15); do
        ctl wait 2 >/dev/null
        now=$(ctl logs --lines 400 | grep -c 'Dissolution Chamber at ' || true)
        if [ "$now" -gt "$before" ]; then
            CBAR_LINE=$(ctl logs --lines 400 | grep 'Dissolution Chamber at ' \
                | tail -1 | sed 's/^.*\[CHAT\] //' || true)
            break
        fi
    done
    [ -n "$CBAR_LINE" ] || fail "no fresh /industrialforegoing info reply for the chamber"
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
# One open creates the scratch world; the add-on's own data pack is then installed and the
# world reopened, because the level's biome tag table is built at load. This happens once,
# before the scenario, so no leg has to run across a mid-scenario relaunch (a reopened
# world on this box stopped answering chat commands, measured 2026-11-22).
launch
ctl quit >/dev/null 2>&1 || true
sleep 1
WORLD_DIR=$(dirname "$(find "$SCRATCH" -name level.dat -print -quit)")
[ -n "$WORLD_DIR" ] || fail "the scratch world has no level.dat"
PACK=$WORLD_DIR/datapacks/if-souls-surface
mkdir -p "$PACK/data/industrialforegoingsouls/recipe/laser_drill_ore" \
         "$PACK/data/industrialforegoingsouls/tags/worldgen/biome"
cat > "$PACK/pack.mcmeta" <<'META'
{
  "pack": {
    "pack_format": 15,
    "description": "industrialforegoingsouls' own laser_drill_ore surface, as its package ships it",
    "supported_formats": {"min_inclusive": 1, "max_inclusive": 9999}
  }
}
META
[ -f "$SOULS_JAR" ] || fail "the add-on's jar is not in the licence cache"
unzip -p "$SOULS_JAR" data/industrialforegoingsouls/recipe/laser_drill_ore/echo_shard.json \
    > "$PACK/data/industrialforegoingsouls/recipe/laser_drill_ore/echo_shard.json"
printf '{"values":["minecraft:deep_dark"]}\n' \
    > "$PACK/data/industrialforegoingsouls/tags/worldgen/biome/deep_dark.json"
launch
# The second open's player is a fresh identity: grant the op the harness setters need.
ctl command "op @a" >/dev/null 2>&1 || true
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
ensure_survival
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
ensure_survival
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
ensure_survival
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
    local before now line="" attempt
    before=$(ctl logs --lines 400 | grep -c 'Water Condensator at .*, sources ' || true)
    say "industrialforegoing info $1 $2 $3"
    for attempt in $(seq 1 10); do
        ctl wait 1 >/dev/null
        now=$(ctl logs --lines 400 | grep -c 'Water Condensator at .*, sources ' || true)
        if [ "$now" -gt "$before" ]; then
            line=$(ctl logs --lines 400 | grep 'Water Condensator at .*, sources ' \
                | tail -1 | sed 's/^.*\[CHAT\] //' || true)
            break
        fi
    done
    [ -n "$line" ] || fail "no fresh /industrialforegoing info reply"
    eval "$(python3 "$PORT/tools/parse_info.py" "$line")"
    [ "${ok:-0}" = 1 ] || fail "the info line did not parse: $line"
    echo "  $line" >&2
}

# The bar's fall over a window the game itself measures: the readout carries the
# game time it was read at (`t=`), so the two reads give the rate exactly — the
# chat round trip cannot skew it. The window stays short so a bar finishing and
# resetting mid-window (25 ticks at speed 4) cannot be mistaken for a rate; the
# caller names the longest window its own bar can survive (a 100-tick bar at one a
# tick survives 50, the 80-tick bar at four a tick only 15).
# Sets BAR_P1, BAR_T1, BAR_FELL and BAR_TICKS.
rate() { # rate LABEL [MAX-WINDOW]
    say "industrialforegoing progress $SX 101 0 100"
    ctl wait 2 >/dev/null
    bar "$SX" 101 0
    BAR_P1=$PROGRESS
    BAR_T1=$T
    # One tick, not four: the second read waits for its own fresh line, so the window is
    # 1 + the read latency. At speed 4 a 19-tick window (measured under load on 2026-11-22)
    # reached the 80-tick bar's reset and the rate came back negative; 15 ticks cannot.
    ctl wait 1 >/dev/null
    bar "$SX" 101 0
    BAR_FELL=$((BAR_P1 - PROGRESS))
    BAR_TICKS=$((T - BAR_T1))
    echo "  $1: progress $BAR_P1 -> $PROGRESS over $BAR_TICKS ticks" >&2
    [ "$BAR_TICKS" -gt 0 ] || fail "$1: the two reads carry the same game time"
    [ "$BAR_TICKS" -lt "${2:-15}" ] \
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
rate "empty slot" 50
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
ensure_survival
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
# The machine's own recipe type, read from this package's data: the 25 files it
# carries (the jar's other 24 name an output this slice does not ship).
expect "the chamber recipes decoded" "$F_RECIPES" 25
expect "the chamber recipe files the game kept" "$F_RECIPES_TOTAL" 25
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
# The loop breaks on the **live** container query, which can see the product of an operation
# that completed after the last readout (2026-11-22: window 3 read 289/300 and 1,000 mB, then
# the product was already in the output). One fresh read after the product is the numbers the
# finished operation left.
cbar
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
ensure_survival
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
ensure_survival
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


# ---------------------------------------------------------------------------
# The laser drill and its ore base (slice 7)
# ---------------------------------------------------------------------------

DRX=232            # the ore base's column; the drill sits one block above it
FLX=240            # the fluid laser base's column, east of the ore base's
BASE_ID=industrialforegoing:ore_laser_base
FLUID_BASE_ID=industrialforegoing:fluid_laser_base
DRILL_ID=industrialforegoing:laser_drill
PURPLE_LENS=industrialforegoing:purple_laser_lens
ORANGE_LENS=industrialforegoing:orange_laser_lens
# The Nether column for the fluid type's lava recipe (slice 10): its own dimension, and the
# 5..20 depth band. Far from the End's arenas by dimension, not by coordinate.
NFX=200
# Its own platform east of the latex chain's: stone at y=100 and air above, in the End's void.
# Peaceful: the End's mobs would otherwise reach the player during the leg.
laser_arena() {
    ctl command "difficulty peaceful" >/dev/null
    ctl command "execute in minecraft:the_end run tp @s $((DRX - 2)).5 104 0.5" >/dev/null
    local loaded=false
    for _ in $(seq 1 120); do
        ctl wait 5 >/dev/null
        if [ "$(ctl block "$DRX" 100 0 | jq -r '.loaded // false')" = true ]; then
            loaded=true
            break
        fi
    done
    [ "$loaded" = true ] || fail "the laser arena chunk never loaded"
    ctl command "execute in minecraft:the_end run fill $((DRX - 8)) 100 -8 $((DRX + 12)) 100 8 minecraft:stone" >/dev/null
    ctl command "execute in minecraft:the_end run fill $((DRX - 8)) 101 -8 $((DRX + 12)) 108 8 minecraft:air" >/dev/null
    ctl wait 8
    ctl command "execute in minecraft:the_end run kill @e[type=!player]" >/dev/null
    ctl command "execute in minecraft:the_end run tp @s $((DRX - 2)).5 101 0.5" >/dev/null
    ctl wait 16
    wait_chat_roundtrip || fail "the chat path never answered after the End transfer"
    [ "$(block_name "$DRX" 100 0)" = "minecraft:stone" ] || fail "the laser arena floor is not stone"
}

# The drill above the base, placed **from its item while looking down**: the six-way state's
# `facing` must be `up`, and the base must fall into the working area below it.
place_drill_above() {
    ctl command "item replace entity @s hotbar.0 with $DRILL_ID 1" >/dev/null
    ctl wait 6
    ctl select 1 >/dev/null
    # One east and five up: 4.1 blocks from the face (inside vanilla's reach, or `look-at-block`
    # refuses), about 76 degrees down (far above the 40-degree threshold) whatever the fall has
    # done, and the player does not land on the block being placed (straight above, a run's
    # place landed after the player had come down onto the base and the placement was refused).
    ctl command "execute in minecraft:the_end run tp @s $((DRX + 1)).5 106 0.5" >/dev/null
    ctl wait 4
    ctl look-at-block "$DRX" 101 0 up >/dev/null 2>&1 || fail "cannot aim at the base's top face"
    ctl wait 2
    ctl place "$DRX" 102 0 --item "$DRILL_ID" >/dev/null || fail "the drill did not place above the base"
    ctl wait 4
    [ "$(block_name "$DRX" 102 0)" = "$DRILL_ID" ] \
        || fail "the drill did not place at $DRX 102 0 (got '$(block_name "$DRX" 102 0)')"
}

# The drill above an arbitrary laser base, placed the same way (looking down, six-way
# `facing: up`), for the fluid base's column.
place_drill_over() { # place_drill_over BASE_X
    local x=$1
    ctl command "item replace entity @s hotbar.0 with $DRILL_ID 1" >/dev/null
    ctl wait 6
    ctl select 1 >/dev/null
    ctl command "execute in minecraft:the_end run tp @s $((x + 1)).5 106 0.5" >/dev/null
    ctl wait 4
    ctl look-at-block "$x" 101 0 up >/dev/null 2>&1 || fail "cannot aim at the fluid base's top face"
    ctl wait 2
    ctl place "$x" 102 0 --item "$DRILL_ID" >/dev/null || fail "the drill did not place above the fluid base"
    ctl wait 4
    [ "$(block_name "$x" 102 0)" = "$DRILL_ID" ] \
        || fail "the drill did not place at $x 102 0 (got '$(block_name "$x" 102 0)')"
}

# One `/industrialforegoing` command of a laser machine at its position.
lasay() { # lasay X Y Z SUB [ARGS...]
    local x=$1 y=$2 z=$3 sub=$4
    shift 4
    say "industrialforegoing $sub $x $y $z${*:+ $*}"
}

# The Nether arena (slice 10): stone at y=10, air above, so the base stands at y=11 —
# inside the lava recipe's 5..20 band — in its own dimension. A fresh dimension is its own
# oracle: the same base, lens and depth in the End answer no recipe (phase CJ).
nether_arena() {
    ctl command "difficulty peaceful" >/dev/null
    ctl command "execute in minecraft:the_nether run tp @s $((NFX - 2)).5 40 0.5" >/dev/null
    local loaded=false
    for _ in $(seq 1 120); do
        ctl wait 5 >/dev/null
        if [ "$(ctl block "$NFX" 30 0 | jq -r '.loaded // false')" = true ]; then
            loaded=true
            break
        fi
    done
    [ "$loaded" = true ] || fail "the Nether arena chunk never loaded"
    ctl command "execute in minecraft:the_nether run fill $((NFX - 8)) 10 -6 $((NFX + 8)) 10 6 minecraft:stone" >/dev/null
    ctl command "execute in minecraft:the_nether run fill $((NFX - 8)) 11 -6 $((NFX + 8)) 20 6 minecraft:air" >/dev/null
    ctl wait 8
    ctl command "execute in minecraft:the_nether run kill @e[type=!player]" >/dev/null
    ctl command "execute in minecraft:the_nether run tp @s $((NFX - 2)).5 11 0.5" >/dev/null
    ctl wait 16
    wait_chat_roundtrip || fail "the chat path never answered after the Nether transfer"
    [ "$(block_name "$NFX" 10 0)" = "minecraft:stone" ] || fail "the Nether arena floor is not stone"
}

# `place_any` in the Nether: the same item-in-hand placement, aiming at the floor under it.
place_nether() { # place_nether X Y Z ITEM
    local x=$1 y=$2 z=$3 item=$4
    ctl command "item replace entity @s hotbar.0 with $item 1" >/dev/null
    ctl wait 6
    ctl select 1 >/dev/null
    ctl command "execute in minecraft:the_nether run tp @s $((x - 2)).5 $y $z.5" >/dev/null
    ctl wait 6
    ctl look-at-block "$x" $((y - 1)) "$z" up >/dev/null 2>&1 || fail "cannot aim at the Nether floor under $x,$y,$z"
    ctl wait 8
    ctl place "$x" "$y" "$z" --item "$item" >/dev/null || fail "the $item did not place at $x,$y,$z in the Nether"
    ctl wait 6
    [ "$(block_name "$x" "$y" "$z")" = "$item" ] \
        || fail "the $item did not place at $x,$y,$z in the Nether (got '$(block_name "$x" "$y" "$z")')"
}

# The drill above the Nether base, from its item looking down (`facing: up`). Two blocks
# above the base's top face, not five: a fresh arena's player falls, and a taller drop put
# the aim out of reach (measured in this slice's probe).
place_drill_over_nether() { # place_drill_over_nether BASE_X
    local x=$1
    ctl command "item replace entity @s hotbar.0 with $DRILL_ID 1" >/dev/null
    ctl wait 6
    ctl select 1 >/dev/null
    ctl command "execute in minecraft:the_nether run tp @s $((x + 1)).5 13 0.5" >/dev/null
    ctl wait 3
    ctl look-at-block "$x" 11 0 up >/dev/null 2>&1 || fail "cannot aim at the Nether base's top face"
    ctl wait 1
    ctl place "$x" 12 0 --item "$DRILL_ID" >/dev/null || fail "the drill did not place above the Nether base"
    ctl wait 4
    [ "$(block_name "$x" 12 0)" = "$DRILL_ID" ] \
        || fail "the drill did not place at $x 12 0 in the Nether (got '$(block_name "$x" 12 0)')"
}

# `/industrialforegoing info` of a laser machine, read from the game's own `[CHAT]` log and
# waiting for a line whose game time differs from the previous read's (the `ctl messages`
# window can return an earlier readout, as slice 6 measured).
laser_info() { # laser_info X Y Z base|drill|fluid
    local x=$1 y=$2 z=$3 kind=$4 pattern line="" best="" best_t=-999999 t cand source attempt
    local before_t=${LASER_T:-0}
    if [ "$kind" = base ]; then
        pattern='Ore Laser Base at .*, biome '
    elif [ "$kind" = fluid ]; then
        pattern='Fluid Laser Base at .*, biome '
    else
        pattern='Laser Drill at .*, target '
    fi
    # A reopened world stops growing `ctl logs`' chat (slice 7's measurement) while the live
    # `ctl messages` window may still hold an older readout. Take the newest line from
    # **both** sources and require its game time to be past the previous read's, so a stale
    # line can never be asserted against. Re-sent every attempt: a chat line is dropped
    # while the client is finishing a dimension transfer (slice 10).
    for attempt in $(seq 1 20); do
        say "industrialforegoing info $x $y $z"
        ctl wait 2 >/dev/null
        for source in logs messages; do
            if [ "$source" = logs ]; then
                cand=$(ctl logs --lines 500 | grep "\[CHAT\] $pattern" | tail -1 | sed 's/^.*\[CHAT\] //' || true)
            else
                cand=$(ctl messages 80 | jq -r '.chat[]' | grep -E "$pattern" | tail -1 || true)
            fi
            [ -n "$cand" ] || continue
            t=$(sed -n 's/.* t=\(-\?[0-9][0-9]*\)$/\1/p' <<<"$cand")
            [ -n "$t" ] || continue
            if [ "$t" -gt "$best_t" ]; then
                best=$cand
                best_t=$t
            fi
        done
        if [ -n "$best" ] && [ "$best_t" -gt "$before_t" ]; then
            line=$best
            break
        fi
    done
    [ -n "$line" ] || fail "no fresh /industrialforegoing info reply for the laser machine"
    LASER_T=$best_t
    eval "$(python3 "$PORT/tools/parse_laser.py" "$line")"
    [ "${ok:-0}" = 1 ] || fail "the laser machine's info line did not parse: $line"
    echo "  $line" >&2
}

# One slot of the base's container: 0–5 are the lens slots, 6–20 the output slots.
base_slot() { # base_slot N
    ctl container "$DRX" 101 0 | jq -r --argjson n "$1" \
        '.slots[$n] | if . == null then "empty" else "\(.item) x\(.count)" end'
}

echo "== BA: the ore base resolves no ore in the End's own biome, and the drill sees it =="
laser_arena
place_any "$DRX" 101 0 "$BASE_ID"
place_drill_above
laser_info "$DRX" 102 0 drill
expect "the drill's six-way facing (placed while looking down)" "$LD_FACING" up
expect "the base it found in its working area" "$LD_TARGET" "[$DRX, 101, 0]"
expect "a fresh drill's buffer" "$LD_ENERGY" 0
laser_info "$DRX" 101 0 base
expect "the base's depth (its own Y until the GUI changes it)" "$LA_DEPTH" 101
expect "the dimension the filters read" "$LA_DIM" minecraft:the_end
expect "the biome the filters read" "$LA_BIOME" minecraft:the_end
expect "the recipes the base's type carries (ten ours and the add-on's)" "$LA_RULES" 11
expect "the recipe files the game kept" "$LA_FILES" 11
expect "the ore in the End's own biome" "$LA_RECIPE" none
echo "  $LA_RULES/$LA_FILES recipes, no ore in $LA_BIOME; the drill targets $LD_TARGET facing $LD_FACING"

echo "== BB: a mountain biome makes the emerald recipe the only match =="
# A whole box, not one cell: `biome_at` is `LevelReader.getBiome`, which zooms across the
# four cells around the position, so a single changed cell averages back to the End's own.
ctl command "execute in minecraft:the_end run fillbiome $((DRX - 8)) 96 -8 $((DRX + 8)) 112 8 minecraft:jagged_peaks" >/dev/null
ctl wait 6
laser_info "$DRX" 101 0 base
expect "the biome after the fillbiome" "$LA_BIOME" minecraft:jagged_peaks
expect "the recipe the biome unlocks" "$LA_RECIPE" "industrialforegoing:laser_drill_ore/emerald_ore"
expect "its weight with no lens" "$LA_WEIGHT" 1
echo "  $LA_BIOME: $LA_RECIPE, weight $LA_WEIGHT"

echo "== BC: a lime lens in a lens slot adds the catalyst modifier =="
ctl command "item replace block $DRX 101 0 container.0 with industrialforegoing:lime_laser_lens 1" >/dev/null
ctl wait 6
laser_info "$DRX" 101 0 base
expect "the lens slot's item" "$(base_slot 0)" "industrialforegoing:lime_laser_lens x1"
expect "the weight with a matching lens" "$LA_WEIGHT" 9
ctl command "item replace block $DRX 101 0 container.0 with minecraft:air" >/dev/null
ctl wait 6
laser_info "$DRX" 101 0 base
expect "the weight after taking the lens out" "$LA_WEIGHT" 1
echo "  weight 1 -> 9 with a lime lens, back to $LA_WEIGHT after"

echo "== BD: one completed base bar makes the recipe's ore for the drill's 1,000 FE =="
lasay "$DRX" 101 0 progress 99
lasay "$DRX" 102 0 energy 10000
lasay "$DRX" 102 0 progress 1
ctl wait 6
laser_info "$DRX" 102 0 drill
laser_info "$DRX" 101 0 base
CHARGE=$LD_ENERGY
if [ "$SELFTEST" = laser ]; then
    echo "(selftest: asserting one FE more than one charge costs)"
    CHARGE=$((CHARGE + 1))
fi
expect "the ore the base yielded" "$(base_slot 6)" "minecraft:emerald_ore x1"
expect "the drill's buffer after one charge (10,000 - 1,000 FE)" "$CHARGE" 9000
expect "the base's bar after the ore" "$LA_PROGRESS" 0
echo "  $LD_ENERGY FE left, $(base_slot 6), bar $LA_PROGRESS/$LA_MAX"

echo "== BE: the counterfactual — the End's own biome, the same charge, nothing mined =="
ctl command "execute in minecraft:the_end run fillbiome $((DRX - 8)) 96 -8 $((DRX + 8)) 112 8 minecraft:the_end" >/dev/null
ctl wait 6
lasay "$DRX" 101 0 progress 99
lasay "$DRX" 102 0 energy 9000
lasay "$DRX" 102 0 progress 1
ctl wait 6
laser_info "$DRX" 102 0 drill
laser_info "$DRX" 101 0 base
expect "the recipe in the End's own biome" "$LA_RECIPE" none
expect "the output the counterfactual leaves" "$(base_slot 6)" "minecraft:emerald_ore x1"
expect "the buffer the charge still took" "$LD_ENERGY" 8000
echo "  recipe $LA_RECIPE, still $(base_slot 6), $LD_ENERGY FE"

echo "== BF: a drill placed from its item while looking east faces west =="
place_any "$((DRX + 4))" 101 0 "$DRILL_ID"
laser_info "$((DRX + 4))" 101 0 drill
expect "the horizontal six-way facing" "$LD_FACING" west
echo "  placed from the west: facing $LD_FACING"

echo "== BG: the add-on's own laser_drill_ore recipe loads under this package's type =="
# `industrialforegoingsouls` ships no recipe type: its one file is ours to load, and it is
# already in the table (11/11 from the first phase). Its biome filter names its own tag,
# loaded with the pack at this world's open; the base answers it in the deep dark at depth 0.
lasay "$DRX" 102 0 progress 50
lasay "$DRX" 102 0 energy 0
lasay "$DRX" 101 0 depth 0
ctl command "execute in minecraft:the_end run fillbiome $((DRX - 8)) 96 -8 $((DRX + 8)) 112 8 minecraft:deep_dark" >/dev/null
ctl wait 8
echo "  $(ctl command "datapack list" | jq -r '.chat | join(" / ")')"
laser_info "$DRX" 101 0 base
expect "the add-on's recipe in the base's table" "$LA_RULES" 11
expect "the files the game kept" "$LA_FILES" 11
expect "the biome the add-on's filter names" "$LA_BIOME" minecraft:deep_dark
expect "the depth it answers" "$LA_DEPTH" 0
expect "the add-on's recipe, resolved by this package" "$LA_RECIPE" "industrialforegoingsouls:laser_drill_ore/echo_shard"
lasay "$DRX" 101 0 progress 99
lasay "$DRX" 102 0 energy 10000
lasay "$DRX" 102 0 progress 1
ctl wait 6
laser_info "$DRX" 101 0 base
expect "the add-on's ore the base yielded" "$(base_slot 7)" "minecraft:echo_shard x1"
echo "  $LA_RULES/$LA_FILES recipes; the add-on's $LA_RECIPE ran: $(base_slot 7)"

echo "== BH: a broken base drops itself and its slots, and its item keeps the bar =="
lasay "$DRX" 102 0 progress 50
lasay "$DRX" 102 0 energy 0
lasay "$DRX" 101 0 progress 40
lasay "$DRX" 101 0 depth 7
ctl wait 4
ensure_survival
ctl command "clear @s" >/dev/null
ctl command "item replace entity @s hotbar.0 with minecraft:iron_pickaxe 1" >/dev/null
ctl wait 6
ctl select 1 >/dev/null
ctl command "execute in minecraft:the_end run tp @s $((DRX - 2)).5 101 0.5" >/dev/null
ctl wait 8
BASE_BROKEN=$(ctl break "$DRX" 101 0) || fail "breaking the base failed"
echo "  break: $(echo "$BASE_BROKEN" | jq -c '{block, drops, picked_up}')"
for id in "$BASE_ID" minecraft:emerald_ore minecraft:echo_shard; do
    [ "$(echo "$BASE_BROKEN" | jq -r ".drops[\"$id\"] // 0")" -ge 1 ] \
        || fail "the broken base did not drop $id: $(echo "$BASE_BROKEN" | jq -c '.drops')"
done
ctl command "execute in minecraft:the_end run tp @s $DRX.5 101 0.5" >/dev/null
ctl wait 20
HELD_BASE=$(ctl inventory | jq -r ".counts[\"$BASE_ID\"] // 0")
[ "$HELD_BASE" -ge 1 ] || fail "the broken base's item was not picked up (counts $HELD_BASE)"
ctl select "$BASE_ID" >/dev/null 2>&1 || ctl select 1 >/dev/null
ctl command "gamemode creative @s" >/dev/null
ctl wait 4
place_held_any "$DRX" 101 0 "$BASE_ID"
laser_info "$DRX" 101 0 base
expect "the bar the item carried" "$LA_PROGRESS" 40
expect "the depth the item carried" "$LA_DEPTH" 7
expect "the output the item carried (none: the slots dropped themselves)" "$LA_OUTPUT" empty
echo "  the base came back with its bar $LA_PROGRESS/100 and depth $LA_DEPTH; its slots dropped as items"

echo "== BI: the fluid base with no wither answers no recipe, and the charge still costs FE =="
# The jar's ether recipe is the one that names this mod's fluid; its gate is a live wither
# in the base's box, not a biome (its biome filter is empty, the depth band is the whole
# world). The counterfactual is therefore the same box with no wither: no recipe, an empty
# tank, and the drill still pays its 1,000 FE.
ctl command "execute in minecraft:the_end run setblock $FLX 101 0 minecraft:air" >/dev/null
ctl command "execute in minecraft:the_end run setblock $FLX 102 0 minecraft:air" >/dev/null
ctl command "execute in minecraft:the_end run setblock $FLX 103 0 minecraft:air" >/dev/null
ctl wait 6
place_any "$FLX" 101 0 "$FLUID_BASE_ID"
place_drill_over "$FLX"
ctl command "item replace block $FLX 101 0 container.0 with $PURPLE_LENS 1" >/dev/null
lasay "$FLX" 101 0 depth 101
ctl command "execute in minecraft:the_end run kill @e[type=!player]" >/dev/null
ctl wait 6
lasay "$FLX" 101 0 progress 19
lasay "$FLX" 102 0 energy 10000
lasay "$FLX" 102 0 progress 1
ctl wait 6
laser_info "$FLX" 102 0 drill
laser_info "$FLX" 101 0 fluid
expect "the drill's target (the fluid base under it)" "$LD_TARGET" "[$FLX, 101, 0]"
expect "the recipes the fluid type carries in a standalone package" "$LF_RULES" 2
expect "the recipe files the game kept" "$LF_FILES" 3
expect "the recipe with no wither in the box" "$LF_RECIPE" none
expect "the tank the counterfactual leaves" "$LF_TANK_MB" 0
expect "the capability the counterfactual leaves" "$LF_CAP_MB" 0
expect "the buffer the charge still took (10,000 - 1,000 FE)" "$LD_ENERGY" 9000
echo "  $LF_RULES/$LF_FILES recipes, $LF_RECIPE without a wither, tank $LF_TANK_MB mB, $LD_ENERGY FE"

echo "== BJ: a wither in the box makes the ether recipe, and one bar fills the tank =="
# The jar's gate: a living entity of the named kind with more than 5 health in the box.
# `NoAI` freezes it (no gravity, no AI); `PersistenceRequired` keeps it past the difficulty
# change, and the player is creative, so the End's mobs do not reach her.
ctl command "difficulty easy" >/dev/null
ctl command "execute in minecraft:the_end run summon minecraft:wither $((FLX + 1)) 102 0 {NoAI:1b,Silent:1b,PersistenceRequired:1b}" >/dev/null
ctl wait 8
# The depth band is the other half of the recipe's rarity: the same box and the same wither,
# only `miningDepth` changes, so the recipe readout is the differential. Ether's band is
# -64..256, so a depth of 300 is outside it and 101 is inside.
lasay "$FLX" 101 0 depth 300
laser_info "$FLX" 101 0 fluid
expect "the recipe above the ether band's ceiling (depth 300)" "$LF_RECIPE" none
expect "the depth the readout reports" "$LF_DEPTH" 300
lasay "$FLX" 101 0 depth 101
laser_info "$FLX" 101 0 fluid
expect "the recipe back inside the band (depth 101)" "$LF_RECIPE" "industrialforegoing:laser_drill_fluid/ether"
echo "  depth 300: no recipe; depth 101: the ether recipe"
lasay "$FLX" 101 0 progress 19
lasay "$FLX" 102 0 energy 9000
lasay "$FLX" 102 0 progress 1
ctl wait 6
laser_info "$FLX" 102 0 drill
laser_info "$FLX" 101 0 fluid
ETHER_MB=$LF_TANK_MB
if [ "$SELFTEST" = fluid ]; then
    echo "(selftest: asserting one mB more than one ether operation gives)"
    ETHER_MB=$((ETHER_MB + 1))
fi
expect "the recipe a wither unlocks" "$LF_RECIPE" "industrialforegoing:laser_drill_fluid/ether"
expect "the ether one bar yields" "$ETHER_MB" 10
expect "the tank's fluid" "$LF_TANK_FLUID" "industrialforegoing:ether_gas"
expect "the fluid the declared capability reports" "$LF_CAP_FLUID" "industrialforegoing:ether_gas"
expect "the capability's amount" "$LF_CAP_MB" 10
expect "the buffer after the charge (9,000 - 1,000 FE)" "$LD_ENERGY" 8000
echo "  $LF_RECIPE: $LF_TANK_MB mB, capability $LF_CAP_MB mB, $LD_ENERGY FE"

echo "== BK: a second bar adds ten more mB to the same tank =="
lasay "$FLX" 101 0 progress 19
lasay "$FLX" 102 0 energy 8000
lasay "$FLX" 102 0 progress 1
ctl wait 6
laser_info "$FLX" 101 0 fluid
laser_info "$FLX" 102 0 drill
expect "the tank after two operations" "$LF_TANK_MB" 20
expect "the buffer after two charges" "$LD_ENERGY" 7000
echo "  tank $LF_TANK_MB/32000 mB of $LF_TANK_FLUID"

echo "== BL: the tank drains through the declared capability =="
lasay "$FLX" 101 0 drain 10
ctl wait 6
laser_info "$FLX" 101 0 fluid
expect "the tank after the drain (20 - 10 mB)" "$LF_TANK_MB" 10
expect "the capability after the drain" "$LF_CAP_MB" 10
echo "  drained 10 mB; tank and capability both read $LF_TANK_MB mB"

echo "== BM: the chamber runs the one carried recipe that names ether gas =="
# The pin's tank capability takes water and lava only, so a mod fluid does not move between
# machines; the chamber's own command writes its input tank, as slice 6's unit does. The
# recipe is the jar's `supreme_machine_frame` (135 mB of ether gas, 300 ticks at 90 FE).
ctl command "execute in minecraft:the_end run kill @e[type=minecraft:wither]" >/dev/null
ctl command "difficulty peaceful" >/dev/null
ctl command "execute in minecraft:the_end run tp @s $((CX - 2)).5 101 0.5" >/dev/null
ctl wait 8
empty_inputs
csay "fluid ether_gas 135"
csay "progress 0"
feed 0 industrialforegoing:plastic
feed 1 industrialforegoing:machine_frame_advanced
feed 2 industrialforegoing:plastic
feed 3 minecraft:netherite_ingot
feed 4 minecraft:netherite_ingot
feed 5 minecraft:diamond
feed 6 industrialforegoing:diamond_gear
feed 7 minecraft:diamond
cbar
expect "the fluid the chamber holds" "$F_FLUID" industrialforegoing:ether_gas
expect "the ether the chamber holds" "$F_FIN" 135
expect "the recipe the grid matches" "$F_RECIPE" "industrialforegoing:dissolution_chamber/supreme_machine_frame"
csay "energy 10000"
csay "progress 299"
ctl wait 8
cbar
expect "the ether the operation consumed" "$F_FIN" 0
expect "the input slots after the operation" "$F_ITEMS" 0
case "$(chamber_outputs)" in
    *"industrialforegoing:machine_frame_supreme x1"*) ;;
    *) fail "the supreme frame is not in the output: '$(chamber_outputs)'" ;;
esac
echo "  135 mB of ether gas and eight items -> $(chamber_outputs)"

echo "== BN: the chamber's only outputFluid recipe — 300 mB of pink slime and a pane give 150 mB of water =="
# A **fresh** chamber at the same position: the one phase AE placed and BM ran is not advanced by
# its own tick any more in this world (measured twice: 219 ticks, bar 0, 10,000 FE unchanged after
# the ether-gas run), while a chamber placed now runs the recipe — the probe under
# `evidence/industrialforegoing/` shows the same sequence from a clean world working. The phase is
# about the recipe, not about that block entity, so it places its own machine; the open question is
# recorded in `notes.md`.
ctl command "execute in minecraft:the_end run setblock $CX 101 0 minecraft:air" >/dev/null
ctl wait 6 >/dev/null
place_any "$CX" 101 0 "$CHAMBER_ID"
ctl command "execute in minecraft:the_end run tp @s $((CX - 2)).5 101 0.5" >/dev/null
ctl wait 8 >/dev/null
csay "fluid pink_slime 300"
feed 0 minecraft:glass_pane
cbar
expect "the fluid the chamber holds" "$F_FLUID" industrialforegoing:pink_slime
expect "the pink slime the chamber holds" "$F_FIN" 300
expect "the recipe the grid matches" "$F_RECIPE" "industrialforegoing:dissolution_chamber/pink_slime_ball"
csay "energy 10000"
csay "progress 0"
ctl wait 100 >/dev/null
csay "energy 10000"
ctl wait 110 >/dev/null
cbar
expect "the pink slime the operation consumed" "$F_FIN" 0
expect "the input slot after the operation" "$F_ITEMS" 0
expect "the output tank's fluid" "$F_OF" minecraft:water
expect "the water the output tank holds (150 mB)" "$F_FO" 150
case "$(chamber_outputs)" in
    *"industrialforegoing:pink_slime x1"*) ;;
    *) fail "the pink slime ball is not in the output: '$(chamber_outputs)'" ;;
esac
echo "  300 mB of pink slime and a pane -> $(chamber_outputs), output tank $F_OF $F_FO mB"


# ---------------------------------------------------------------------------
# The mob slaughter factory and the pink slime set (slice 9)
# ---------------------------------------------------------------------------

SFX=250            # the factory's column, east of the fluid base's
FACTORY_ID=industrialforegoing:mob_slaughter_factory
SLAUGHTER_T=0      # the game time of the last factory readout this run asserted against

# Its own platform east of the laser arena's: stone at y=100, air above, in the End's void.
factory_arena() {
    ctl command "difficulty peaceful" >/dev/null
    ctl command "execute in minecraft:the_end run tp @s $((SFX - 2)).5 104 0.5" >/dev/null
    local loaded=false
    for _ in $(seq 1 120); do
        ctl wait 5 >/dev/null
        if [ "$(ctl block "$SFX" 100 0 | jq -r '.loaded // false')" = true ]; then
            loaded=true
            break
        fi
    done
    [ "$loaded" = true ] || fail "the factory arena chunk never loaded"
    ctl command "execute in minecraft:the_end run fill $((SFX - 6)) 100 -8 $((SFX + 6)) 100 8 minecraft:stone" >/dev/null
    ctl command "execute in minecraft:the_end run fill $((SFX - 6)) 101 -8 $((SFX + 6)) 108 8 minecraft:air" >/dev/null
    ctl wait 8
    ctl command "execute in minecraft:the_end run kill @e[type=!player]" >/dev/null
    ctl command "execute in minecraft:the_end run tp @s $((SFX - 2)).5 101 0.5" >/dev/null
    ctl wait 16
    wait_chat_roundtrip || fail "the chat path never answered after the End transfer"
    [ "$(block_name "$SFX" 100 0)" = "minecraft:stone" ] || fail "the factory arena floor is not stone"
}

# One `/industrialforegoing` command of the factory at its position.
slay() { # slay SUB [ARGS...]
    local sub=$1
    shift
    say "industrialforegoing $sub $SFX 101 0${*:+ $*}"
}

# `/industrialforegoing info` of the factory, from the newest line of the game's own chat (the
# `ctl messages` window can return an earlier readout, measured on 2026-11-22), required to be
# newer than the previous read by its own `t=`.
slaughter_info() {
    local pattern='Mob Slaughter Factory at ' line="" best="" best_t=-999999 t cand source attempt
    for attempt in $(seq 1 20); do
        say "industrialforegoing info $SFX 101 0"
        ctl wait 2 >/dev/null
        for source in logs messages; do
            if [ "$source" = logs ]; then
                cand=$(ctl logs --lines 500 | grep "\[CHAT\] $pattern" | tail -1 | sed 's/^.*\[CHAT\] //' || true)
            else
                cand=$(ctl messages 80 | jq -r '.chat[]' | grep -E "$pattern" | tail -1 || true)
            fi
            [ -n "$cand" ] || continue
            t=$(sed -n 's/.* t=\(-\?[0-9][0-9]*\)$/\1/p' <<<"$cand")
            [ -n "$t" ] || continue
            if [ "$t" -gt "$best_t" ]; then
                best=$cand
                best_t=$t
            fi
        done
        if [ -n "$best" ] && [ "$best_t" -gt "$SLAUGHTER_T" ]; then
            line=$best
            break
        fi
    done
    SLAUGHTER_T=$best_t
    SLAY_LINE=$line
    [ -n "$SLAY_LINE" ] || fail "no fresh /industrialforegoing info reply for the factory"
    eval "$(python3 "$PORT/tools/parse_slaughter.py" "$SLAY_LINE")"
    [ "${ok:-0}" = 1 ] || fail "the factory info line did not parse: $SLAY_LINE"
    echo "  $SLAY_LINE" >&2
}

# A mob in the factory's working area: the block behind it (west-facing, so east), on the floor.
# `NoAI` freezes it (no AI step, no gravity in the air), `Silent` keeps the log quiet, and
# `PersistenceRequired` keeps it past a difficulty change.
summon_mob() { # summon_mob KIND
    ctl command "execute in minecraft:the_end run summon minecraft:$1 $((SFX + 1)).5 101 0.5 {NoAI:1b,Silent:1b,PersistenceRequired:1b}" >/dev/null
    ctl wait 8 >/dev/null
}

# The two comparisons a bar that is still falling needs (`expect` is exact).
expect_ge() { [ "$2" -ge "$3" ] || fail "$1 is $2, expected at least $3"; }
expect_le() { [ "$2" -le "$3" ] || fail "$1 is $2, expected at most $3"; }

echo "== CA: the mob slaughter factory places, faces its placer, and is empty =="
factory_arena
place_any "$SFX" 101 0 "$FACTORY_ID"
slay progress 100
ctl wait 2 >/dev/null
slaughter_info
expect "the block" "$(block_name "$SFX" 101 0)" "$FACTORY_ID"
expect "the facing" "$S_FACING" west
expect "the buffer" "$S_ENERGY" 0
expect "the meat tank" "$S_MEAT_MB" 0
expect "the pink slime tank" "$S_SLIME_MB" 0
expect "the mobs in its working area" "$S_VICTIMS" 0
expect "the bar's length" "$S_BAR" 100
# The bar counts down every tick, so the readout is never exactly 100: what a fresh machine
# asserts is a bar near full (an attempt rests at 20 only after a kill).
expect_ge "the bar" "$S_PROGRESS" 90
echo "  block $FACTORY_ID, facing $S_FACING, both tanks empty, $S_ENERGY FE, bar $S_PROGRESS/$S_BAR"

echo "== CB: the counterfactual — no mob in the box, nothing killed and nothing spent =="
# The artefact's rule (`SlaughterFactoryTile.work()`): without a mob it answers
# `WorkAction(1f, 0)`, so the attempt costs **no** FE and the bar rests full. (The brief's
# prose said "FE still spent"; the jar says otherwise, and the port follows the jar.)
slay progress 100
slay energy 5000
ctl wait 2 >/dev/null
slay progress 1
ctl wait 6 >/dev/null
slaughter_info
expect "the buffer (an idle attempt takes nothing)" "$S_ENERGY" 5000
expect "the meat tank" "$S_MEAT_MB" 0
expect "the pink slime tank" "$S_SLIME_MB" 0
expect "the mobs in the box" "$S_VICTIMS" 0
expect_ge "the bar after an idle attempt (rests full)" "$S_PROGRESS" 90

echo "== CC: a cow in the box is killed for 10 meat and 200 mB of pink slime =="
slay progress 100
slay energy 5000
summon_mob cow
slaughter_info
expect "the mobs in the box before the attempt" "$S_VICTIMS" 1
slay progress 1
ctl wait 6 >/dev/null
slaughter_info
SLIME_WANT=$S_SLIME_MB
if [ "$SELFTEST" = slaughter ]; then
    echo "(selftest: asserting one mB more of pink slime than a cow gives)"
    SLIME_WANT=$((SLIME_WANT + 1))
fi
expect "the meat a cow gives ((int) health)" "$S_MEAT_MB" 10
expect "the pink slime a cow gives ((int)(health * 20))" "$SLIME_WANT" 200
expect "the meat the declared capability reports" "$S_MEAT_CAP_MB" 10
expect "the pink slime the declared capability reports" "$S_SLIME_CAP_MB" 200
expect "the fluid the pink slime tank holds" "$S_SLIME" industrialforegoing:pink_slime
expect "the fluid the meat tank holds" "$S_MEAT" industrialforegoing:meat
expect "the buffer after the kill (5,000 - 400 FE)" "$S_ENERGY" 4600
expect_le "the bar after a kill (floor(100 * 0.2))" "$S_PROGRESS" 20
expect "the mobs left in the box (the cow is gone)" "$S_VICTIMS" 0
echo "  a cow: $S_MEAT_MB mB of $S_MEAT, $S_SLIME_MB mB of $S_SLIME, $S_ENERGY FE"

echo "== CD: the counterfactual — a mob but no FE: nothing is killed =="
slay progress 100
slay energy 0
summon_mob villager
slaughter_info
expect "the mobs in the box (a villager, not an animal)" "$S_VICTIMS" 1
slay progress 1
ctl wait 6 >/dev/null
slaughter_info
expect "the mobs still in the box (hasEnergy(400) refused)" "$S_VICTIMS" 1
expect "the meat tank (unchanged)" "$S_MEAT_MB" 10
expect "the pink slime tank (unchanged)" "$S_SLIME_MB" 200
expect "the buffer" "$S_ENERGY" 0
expect_ge "the bar after the refused attempt" "$S_PROGRESS" 90

echo "== CE: a villager is killed for 400 meat and 20 mB of pink slime, and both tanks drain =="
slay energy 5000
ctl wait 2 >/dev/null
slay progress 1
ctl wait 6 >/dev/null
slaughter_info
expect "the meat the villager adds ((int) health * 20)" "$S_MEAT_MB" 410
expect "the pink slime the villager adds ((int) health)" "$S_SLIME_MB" 220
expect "the buffer after the second kill" "$S_ENERGY" 4600
expect "the mobs left in the box" "$S_VICTIMS" 0
say "industrialforegoing drain $SFX 101 0 100"
ctl wait 8 >/dev/null
DRAINED=$(ctl messages 14 | jq -r '.chat[]' | grep -m1 'Mob Slaughter Factory at .*drained' || true)
echo "  $DRAINED" >&2
case "$DRAINED" in
    *"drained 100 mB of industrialforegoing:meat and 100 mB of industrialforegoing:pink_slime."*) ;;
    *) fail "both declared tanks refused a real drain: $DRAINED" ;;
esac
slaughter_info
expect "the meat after the drain" "$S_MEAT_MB" 310
expect "the pink slime after the drain" "$S_SLIME_MB" 120
echo "  a villager: $S_MEAT_MB mB of $S_MEAT, $S_SLIME_MB mB of $S_SLIME; both drained through the capability"

echo "== CG: the pink slime block places and drops itself =="
place_any 246 101 0 industrialforegoing:pink_slime_block
expect "the pink slime block" "$(block_name 246 101 0)" industrialforegoing:pink_slime_block
ensure_survival
ctl command "clear @s" >/dev/null
ctl wait 4 >/dev/null
SLIME_BROKEN=$(ctl break 246 101 0) || fail "breaking the pink slime block failed"
echo "  break: $(echo "$SLIME_BROKEN" | jq -c '{block, drops}')"
[ "$(echo "$SLIME_BROKEN" | jq -r '.drops["industrialforegoing:pink_slime_block"] // 0')" -ge 1 ] \
    || fail "the pink slime block did not drop itself: $(echo "$SLIME_BROKEN" | jq -c '.drops')"
ctl command "gamemode creative @s" >/dev/null
ctl wait 4 >/dev/null


echo "== CI: a broken factory's item carries both tanks and its buffer =="
ctl command "execute in minecraft:the_end run tp @s $((SFX - 2)).5 101 0.5" >/dev/null
ctl wait 8 >/dev/null
ensure_survival
ctl command "clear @s" >/dev/null
ctl command "item replace entity @s hotbar.0 with minecraft:iron_pickaxe 1" >/dev/null
ctl wait 6 >/dev/null
ctl select 1 >/dev/null
ctl wait 4 >/dev/null
FACTORY_BROKEN=$(ctl break "$SFX" 101 0) || fail "breaking the factory failed"
echo "  break: $(echo "$FACTORY_BROKEN" | jq -c '{block, drops, picked_up}')"
[ "$(echo "$FACTORY_BROKEN" | jq -r '.drops["industrialforegoing:mob_slaughter_factory"] // 0')" -ge 1 ] \
    || fail "the factory did not drop itself: $(echo "$FACTORY_BROKEN" | jq -c '.drops')"
ctl command "execute in minecraft:the_end run tp @s $SFX.5 101 0.5" >/dev/null
ctl wait 20 >/dev/null
HELD_FACTORY=$(ctl inventory | jq -r '.counts["industrialforegoing:mob_slaughter_factory"] // 0')
[ "$HELD_FACTORY" -ge 1 ] || fail "the broken factory's item was not picked up (counts $HELD_FACTORY)"
ctl command "gamemode creative @s" >/dev/null
ctl select "$FACTORY_ID" >/dev/null 2>&1 || ctl select 1 >/dev/null
ctl wait 4 >/dev/null
place_held_any "$SFX" 101 0 "$FACTORY_ID"
slaughter_info
expect "the meat the item carried" "$S_MEAT_MB" 310
expect "the pink slime the item carried" "$S_SLIME_MB" 120
expect "the buffer the item carried" "$S_ENERGY" 4600
echo "  the factory came back with $S_MEAT_MB mB of $S_MEAT, $S_SLIME_MB mB of $S_SLIME and $S_ENERGY FE"

echo "== CJ: the lava recipe's depth in the End answers no recipe (the dimension arm) =="
# The lava file's rarity is `dimension_filter.whitelist: ["minecraft:the_nether"]` and
# `depth 5..20`; the ether file names every dimension but is gated on a wither in the box
# (BM killed the last one and the difficulty is peaceful). So the same base, the same
# orange lens and the same depth **11**, differing only in the dimension, must answer no
# recipe in the End. This is the shared `laser::Filter`'s dimension branch in game.
ctl command "difficulty peaceful" >/dev/null
ctl command "execute in minecraft:the_end run kill @e[type=minecraft:wither]" >/dev/null
ctl command "execute in minecraft:the_end run tp @s $((FLX - 2)).5 101 0.5" >/dev/null
ctl command "item replace block $FLX 101 0 container.0 with $ORANGE_LENS 1" >/dev/null
lasay "$FLX" 101 0 depth 11
ctl wait 8
laser_info "$FLX" 101 0 fluid
expect "the dimension filter the base reads" "$LF_DIM" minecraft:the_end
expect "the depth inside the lava band" "$LF_DEPTH" 11
expect "the recipe the dimension arm refuses in the End" "$LF_RECIPE" none
expect "the tank the refused arm leaves (BL's 10 mB of ether)" "$LF_TANK_MB" 10
echo "  depth 11 in $LF_DIM: $LF_RECIPE (the lava file needs the Nether)"

echo "== CK: in the Nether at depth 0 the same base answers no recipe and leaves the tank empty =="
# The depth arm, inside the right dimension: the base stands at y=11 (its own depth until a
# GUI/command changes it) but the command sets 0, below the band's floor of 5.
nether_arena
place_nether "$NFX" 11 0 "$FLUID_BASE_ID"
place_drill_over_nether "$NFX"
ctl command "item replace block $NFX 11 0 container.0 with $ORANGE_LENS 1" >/dev/null
lasay "$NFX" 11 0 depth 0
lasay "$NFX" 12 0 energy 0
ctl wait 8
laser_info "$NFX" 11 0 fluid
expect "the dimension the base now stands in" "$LF_DIM" minecraft:the_nether
expect "the depth the counterfactual sets" "$LF_DEPTH" 0
expect "the recipe below the lava band" "$LF_RECIPE" none
expect "the tank the depth counterfactual leaves" "$LF_TANK_MB" 0
expect "the capability it leaves" "$LF_CAP_MB" 0
expect "the recipe files the game keeps in the Nether" "$LF_RULES/$LF_FILES" 2/3
echo "  $LF_DIM depth $LF_DEPTH: $LF_RECIPE, tank $LF_TANK_MB mB"

echo "== CL: at depth 11 in the Nether one bar fills the tank with 100 mB of lava =="
# The positive arm: the same box, same orange lens, depth inside the band, the drill
# charged. `progress 19` + one drill charge is exactly one completed 20-tick bar.
lasay "$NFX" 11 0 depth 11
ctl wait 2
laser_info "$NFX" 11 0 fluid
expect "the depth inside the lava band" "$LF_DEPTH" 11
expect "the recipe the dimension arm admits" "$LF_RECIPE" "industrialforegoing:laser_drill_fluid/lava"
expect "the tank before the operation" "$LF_TANK_MB" 0
lasay "$NFX" 11 0 progress 19
lasay "$NFX" 12 0 energy 10000
lasay "$NFX" 12 0 progress 1
ctl wait 8
laser_info "$NFX" 12 0 drill
laser_info "$NFX" 11 0 fluid
LAVA_MB=$LF_TANK_MB
if [ "$SELFTEST" = nether ]; then
    echo "(selftest: asserting one mB more than one lava operation gives)"
    LAVA_MB=$((LAVA_MB + 1))
fi
expect "the lava one bar yields" "$LAVA_MB" 100
expect "the tank's fluid" "$LF_TANK_FLUID" minecraft:lava
expect "the fluid the declared capability reports" "$LF_CAP_FLUID" minecraft:lava
expect "the capability's amount" "$LF_CAP_MB" 100
expect "the drill's target (the Nether base)" "$LD_TARGET" "[$NFX, 11, 0]"
expect "the buffer after the charge (10,000 - 1,000 FE)" "$LD_ENERGY" 9000
echo "  $LF_RECIPE: tank $LF_TANK_FLUID $LF_TANK_MB/32000 mB, capability $LF_CAP_MB mB, $LD_ENERGY FE"

# ---------------------------------------------------------------------------
# The mob crusher and its essence (slice 11)
# ---------------------------------------------------------------------------

CFX=260            # the crusher's column, east of the slaughter factory's
CRUSHER_ID=industrialforegoing:mob_crusher
CRUSHER_T=0        # the game time of the last crusher readout this run asserted against

# Its own platform east of the slaughter factory's: stone at y=100, air above, in the End's void.
crusher_arena() {
    ctl command "difficulty peaceful" >/dev/null
    ctl command "execute in minecraft:the_end run tp @s $((CFX - 2)).5 104 0.5" >/dev/null
    local loaded=false
    for _ in $(seq 1 120); do
        ctl wait 5 >/dev/null
        if [ "$(ctl block "$CFX" 100 0 | jq -r '.loaded // false')" = true ]; then
            loaded=true
            break
        fi
    done
    [ "$loaded" = true ] || fail "the crusher arena chunk never loaded"
    ctl command "execute in minecraft:the_end run fill $((CFX - 6)) 100 -8 $((CFX + 6)) 100 8 minecraft:stone" >/dev/null
    ctl command "execute in minecraft:the_end run fill $((CFX - 6)) 101 -8 $((CFX + 6)) 108 8 minecraft:air" >/dev/null
    ctl wait 8
    ctl command "execute in minecraft:the_end run kill @e[type=!player]" >/dev/null
    ctl command "execute in minecraft:the_end run kill @e[type=minecraft:experience_orb]" >/dev/null
    ctl command "execute in minecraft:the_end run tp @s $((CFX - 2)).5 101 0.5" >/dev/null
    ctl wait 16
    wait_chat_roundtrip || fail "the chat path never answered after the End transfer"
    [ "$(block_name "$CFX" 100 0)" = "minecraft:stone" ] || fail "the crusher arena floor is not stone"
}

# One `/industrialforegoing` command of the crusher at its position.
crush() { # crush SUB [ARGS...]
    local sub=$1
    shift
    say "industrialforegoing $sub $CFX 101 0${*:+ $*}"
}

# `/industrialforegoing info` of the crusher, from the newest line of both chat sources, required
# to be newer than the previous read by its own `t=`.
crusher_info() {
    local pattern='Mob Crusher at ' line="" best="" best_t=-999999 t cand source attempt
    for attempt in $(seq 1 20); do
        say "industrialforegoing info $CFX 101 0"
        ctl wait 2 >/dev/null
        sleep 0.2
        for source in logs messages; do
            if [ "$source" = logs ]; then
                cand=$(ctl logs --lines 600 | grep "\[CHAT\] $pattern" | tail -1 | sed 's/^.*\[CHAT\] //' || true)
            else
                cand=$(ctl messages 80 | jq -r '.chat[]' | grep -E "$pattern" | tail -1 || true)
            fi
            [ -n "$cand" ] || continue
            t=$(sed -n 's/.* t=\(-\?[0-9][0-9]*\)$/\1/p' <<<"$cand")
            [ -n "$t" ] || continue
            if [ "$t" -gt "$best_t" ]; then
                best=$cand
                best_t=$t
            fi
        done
        if [ -n "$best" ] && [ "$best_t" -gt "$CRUSHER_T" ]; then
            line=$best
            break
        fi
    done
    CRUSHER_T=$best_t
    CRUSHER_LINE=$line
    [ -n "$CRUSHER_LINE" ] || fail "no fresh /industrialforegoing info reply for the crusher"
    eval "$(python3 "$PORT/tools/parse_crusher.py" "$CRUSHER_LINE")"
    [ "${ok:-0}" = 1 ] || fail "the crusher info line did not parse: $CRUSHER_LINE"
    echo "  $CRUSHER_LINE" >&2
}

# A mob in the crusher's working area: the block behind it (west-facing, so east), on the floor.
summon_victim() { # summon_victim KIND
    ctl command "execute in minecraft:the_end run summon minecraft:$1 $((CFX + 1)).5 101 0.5 {NoAI:1b,Silent:1b,PersistenceRequired:1b}" >/dev/null
    ctl wait 8 >/dev/null
}

# The beef and leather item entities on the arena floor: the differential for the output — a
# `living_drops` that let the world have them would leave them here, and the output would be empty.
crusher_ground_loot() {
    ctl entities 48 item 2>/dev/null | jq -r --argjson x0 $((CFX - 2)) --argjson x1 $((CFX + 4)) '
      [.entities[] | select(.kind == "item")
       | select((.item.item // "") | test("minecraft:(beef|leather)$"))
       | select((.position[0] | floor) >= $x0 and (.position[0] | floor) <= $x1)
       | select((.position[2] | floor) >= -4 and (.position[2] | floor) <= 4)
      ] | length' 2>/dev/null || echo 0
}

echo "== CM: the mob crusher places, faces its placer, and is empty =="
crusher_arena
place_any "$CFX" 101 0 "$CRUSHER_ID"
crush progress 100
ctl wait 2 >/dev/null
crusher_info
expect "the block" "$(block_name "$CFX" 101 0)" "$CRUSHER_ID"
expect "the facing" "$C_FACING" west
expect "the buffer" "$C_ENERGY" 0
expect "the essence tank" "$C_TANK_MB" 0
expect "the tank's fluid (empty)" "$C_TANK" empty
expect "the declared capability's amount" "$C_CAP_MB" 0
expect "the mobs in its working area" "$C_VICTIMS" 0
expect "the output container" "$C_OUTPUT" empty
expect "the last kill's experience" "$C_LAST_XP" 0
expect "the bar's length" "$C_BAR" 100
expect_ge "the bar" "$C_PROGRESS" 90
echo "  block $CRUSHER_ID facing $C_FACING, tank $C_TANK $C_TANK_MB/$C_TANK_CAP mB, output $C_OUTPUT"

echo "== CN: the counterfactual — no mob in the box, nothing killed and nothing spent =="
# `MobCrusherTile.work()` without a victim answers `WorkAction(1f, 0)`: no FE out, the bar full.
crush progress 100
crush energy 5000
ctl wait 2 >/dev/null
crush progress 1
ctl wait 6 >/dev/null
crusher_info
expect "the buffer (an idle attempt takes nothing)" "$C_ENERGY" 5000
expect "the essence tank" "$C_TANK_MB" 0
expect "the mobs in the box" "$C_VICTIMS" 0
expect "the last kill's experience" "$C_LAST_XP" 0
expect_ge "the bar after an idle attempt (rests full)" "$C_PROGRESS" 90

echo "== CO: a cow in the box is killed for 20 mB a point of its experience =="
# The jar's rule is `experience * 20` (`MobCrusherTile.instantKill`), and a cow's experience
# reward is 1-3 points, so one cow adds 20, 40 or 60 mB — the readout's `last xp` is the
# engine's own count, and the tank must be exactly twenty times it. A health-based rule (the
# brief's premise) would read 200 here and fail.
crush progress 100
crush energy 5000
summon_victim cow
crusher_info
expect "the mobs in the box before the attempt" "$C_VICTIMS" 1
crush progress 1
ctl wait 3 >/dev/null
crusher_info
XP=$C_LAST_XP
ESSENCE_WANT=$((XP * 20))
if [ "$SELFTEST" = crusher ]; then
    echo "(selftest: asserting one mB more than the experience gives)"
    ESSENCE_WANT=$((ESSENCE_WANT + 1))
fi
case "$XP" in
    1|2|3) ;;
    *) fail "a cow's experience reward is $XP, expected 1-3 (the vanilla 1 + random(3))" ;;
esac
expect "the essence a cow gives (20 x $XP points)" "$C_TANK_MB" "$ESSENCE_WANT"
expect "the tank's fluid" "$C_TANK" industrialforegoing:essence
expect "the fluid the declared capability reports" "$C_CAP_FLUID" industrialforegoing:essence
expect "the capability's amount" "$C_CAP_MB" "$ESSENCE_WANT"
expect "the buffer after the kill (5,000 - 50 FE)" "$C_ENERGY" 4950
expect_le "the bar after a kill (floor(100 * 0.1))" "$C_PROGRESS" 10
expect "the mobs left in the box (the cow is gone)" "$C_VICTIMS" 0
case "$C_OUTPUT" in
    *"minecraft:beef"*) ;;
    *) fail "the cow's loot did not land in the crusher's output: $C_OUTPUT" ;;
esac
GROUND=$(crusher_ground_loot)
expect "the cow's loot left on the floor (the machine took it)" "$GROUND" 0
echo "  a cow: $XP experience -> $C_TANK_MB mB of $C_TANK, output $C_OUTPUT, $C_ENERGY FE"

echo "== CP: the counterfactual — a mob but no FE: nothing is killed and nothing is added =="
TANK_BEFORE=$C_TANK_MB
crush progress 100
crush energy 0
summon_victim cow
crusher_info
expect "the mobs in the box (a cow)" "$C_VICTIMS" 1
crush progress 1
ctl wait 8 >/dev/null
crusher_info
expect "the mobs still in the box (hasEnergy(50) refused)" "$C_VICTIMS" 1
expect "the essence tank (unchanged)" "$C_TANK_MB" "$TANK_BEFORE"
expect "the buffer" "$C_ENERGY" 0
expect_ge "the bar after the refused attempt" "$C_PROGRESS" 90
echo "  no FE: the cow is still there, tank $C_TANK_MB mB"

echo "== CQ: a hopper under the crusher pulls its 18-slot output =="
# The output is upstream's `SidedInventoryComponent`, output-only but reachable: a hopper below
# the machine must take the cow's loot out of it, which is what makes the container a real
# capability instead of a private list.
ctl command "execute in minecraft:the_end run setblock $CFX 100 0 minecraft:hopper[facing=down]" >/dev/null
ctl command "execute in minecraft:the_end run setblock $CFX 99 0 minecraft:barrel" >/dev/null
ctl wait 60 >/dev/null
BARREL=$(ctl container "$CFX" 99 0 | jq -r '[.slots[]? | select(. != null) | .item] | join(",")')
case "$BARREL" in
    *minecraft:beef*|*minecraft:leather*) ;;
    *) fail "the hopper did not empty the crusher's output into the barrel: '$BARREL'" ;;
esac
crusher_info
expect "the output after the hopper" "$C_OUTPUT" empty
echo "  barrel through the hopper: $BARREL"
# Put the floor back under the crusher for the break leg.
ctl command "execute in minecraft:the_end run setblock $CFX 100 0 minecraft:stone" >/dev/null
ctl wait 6 >/dev/null

echo "== CR: a broken crusher's item carries its tank and buffer =="
ctl command "execute in minecraft:the_end run kill @e[type=minecraft:cow]" >/dev/null
crusher_arena
place_any "$CFX" 101 0 "$CRUSHER_ID"
crush energy 4600
summon_victim cow
crush progress 1
ctl wait 6 >/dev/null
crusher_info
BROKEN_TANK=$C_TANK_MB
BROKEN_XP=$C_LAST_XP
BROKEN_ENERGY=$C_ENERGY
[ "$BROKEN_TANK" -gt 0 ] || fail "the crusher's tank never filled before the break"
expect "the buffer after the kill (4,600 - 50 FE)" "$BROKEN_ENERGY" 4550
ctl command "execute in minecraft:the_end run tp @s $((CFX - 2)).5 101 0.5" >/dev/null
ctl wait 8 >/dev/null
ensure_survival
ctl command "clear @s" >/dev/null
ctl command "item replace entity @s hotbar.0 with minecraft:iron_pickaxe 1" >/dev/null
ctl wait 6 >/dev/null
ctl select 1 >/dev/null
ctl wait 4 >/dev/null
CRUSHER_BROKEN=$(ctl break "$CFX" 101 0) || fail "breaking the crusher failed"
echo "  break: $(echo "$CRUSHER_BROKEN" | jq -c '{block, drops, picked_up}')"
[ "$(echo "$CRUSHER_BROKEN" | jq -r '.drops["industrialforegoing:mob_crusher"] // 0')" -ge 1 ] \
    || fail "the crusher did not drop itself: $(echo "$CRUSHER_BROKEN" | jq -c '.drops')"
ctl command "execute in minecraft:the_end run tp @s $CFX.5 101 0.5" >/dev/null
ctl wait 20 >/dev/null
HELD_CRUSHER=$(ctl inventory | jq -r '.counts["industrialforegoing:mob_crusher"] // 0')
[ "$HELD_CRUSHER" -ge 1 ] || fail "the broken crusher's item was not picked up (counts $HELD_CRUSHER)"
ctl command "gamemode creative @s" >/dev/null
ctl select "$CRUSHER_ID" >/dev/null 2>&1 || ctl select 1 >/dev/null
ctl wait 4 >/dev/null
place_held_any "$CFX" 101 0 "$CRUSHER_ID"
crusher_info
expect "the essence the item carried" "$C_TANK_MB" "$BROKEN_TANK"
expect "the buffer the item carried" "$C_ENERGY" "$BROKEN_ENERGY"
expect "the last kill's experience the item carried" "$C_LAST_XP" "$BROKEN_XP"
echo "  the crusher came back with $C_TANK_MB mB of $C_TANK, $C_ENERGY FE and last xp $C_LAST_XP"

echo "PASS: a placed machine is empty with no sources and faces its placer; two sources condense 200 mB for exactly 1,000 FE and one condenses nothing; without 1,000 FE two sources give half for free; three and six sources give 300 and 600 mB; a nearly full tank stops at 16,000 mB while still spending the FE; the declared tank drains 400 mB of water through the fluid handler; and left alone the machine keeps condensing on its own 10-tick bar; the Core ids of these slices register and read back, the four frames place and read back, an unregistered item and block id are refused, and a frame broken in survival drops itself; the condensator crafts from the tags (#c:plastics, #c:gears/iron, machine_frame/pity) with the grid empty afterwards while slice 2's stand-ins stay empty, the pity frame and the three gears craft, and all 16 lens recolours craft from the lens tag and c:dyes with the player's counts before and after; a machine broken with 7,000 FE and 5,000 mB comes back with exactly those numbers on the item it dropped; a seated speed augment of 4 makes the bar fall four a tick where an empty slot falls one, a second speed augment is refused and stays in the hand, an efficiency augment of 0.8 shortens the bar to 80 with an 8-tick cooldown at the same 1,000 FE, and both augments travel on the machine's item; the dissolution chamber places and faces its placer, reads its own recipe type from this package's data (25 files), holds its own fluids in a declared tank the capability reports back, runs the white lens recipe in exactly 100 ticks for 9,000 FE and 250 mB of latex, does nothing with an empty tank and starts when 250 mB arrive, empties its output into a hopper and a barrel, runs the simple machine frame's 300 ticks at exactly 90 FE a tick, makes the advanced frame from #c:plastics, the crafted #c:gears/diamond gear and the simple frame, and a broken chamber carries its buffer and latex back on its item; and the latex chain: a fluid extractor works on the oak log behind it, giving 2 mB for free and 6 mB for 500 FE, nothing at all when the block behind it is stone, and — on its eighth hit — consuming the log into a stripped one and bursting 400 mB that drain back out of its declared tank, where a roll over its break chance moves nothing; a latex processing unit takes 1,000 mB of latex through the command path and 1,000 mB of water through the declared capability (which refuses the mod fluid, so no fluid move can fill it), turns 750 mB of latex and 500 mB of water into one dry rubber for 2,000 FE, a hopper feeds that rubber into a furnace which smelts it into one plastic, and a broken unit carries its latex, water and buffer back on its item; and the laser drill: placed above its ore base it faces up and finds it, reads the base's own ten-recipe table, resolves no ore in the End's biome and the emerald recipe at weight 1 in a mountain one, a lime lens raises that weight to 9, one completed base bar yields the recipe's emerald ore for exactly 1,000 FE while the same charge in the End's own biome yields nothing, and — with the add-on's own laser_drill_ore file and biome tag installed as its package ships them — the base's table reads 11/11 and that recipe runs, yielding one echo shard; a broken base carries its output slots back on its item; and the fluid laser base, under its own drill, answers no recipe without the wither its ether file names and leaves the tank empty while the drill still pays 1,000 FE, then with a wither in its box one completed bar yields 10 mB of industrialforegoing:ether_gas into the tank the declared capability reports, a second bar adds ten more, the tank drains through that capability, and the dissolution chamber runs its one carried recipe naming ether gas (supreme_machine_frame, 135 mB) to the product; and the mob slaughter factory: placed facing its placer it is empty, a bar with no mob in the working area kills nothing and spends no FE, a cow in the box is killed for 10 mB of meat and 200 mB of pink slime for exactly 400 FE (both tanks reported back by the declared capability), the same box with no FE leaves a villager alive and adds nothing, 5,000 FE then kills the villager for 400 mB of meat and 20 mB of pink slime, both tanks drain through the capability, the factory's own shaped recipe crafts from #c:plastics, #c:gears/gold and machine_frame/pity, the pink slime block crafts from nine pink slime and gives them back shapeless and drops itself, a broken factory carries both tanks and its buffer back on its item, and the dissolution chamber runs the jar's only outputFluid recipe — 300 mB of pink slime and a glass pane for one pink slime ball and 150 mB of minecraft:water in its output tank; and the fluid laser base's dimension-and-depth pair: the lava recipe's band at depth 11 answers no recipe in the End (the dimension filter), in the Nether at depth 0 it answers none and leaves an empty tank, and at depth 11 in the Nether one completed bar of the drill fills the tank with 100 mB of minecraft:lava, reported by the declared capability, for 1,000 FE; and the mob crusher: placed facing its placer it is empty, a bar with no mob spends nothing, a cow in its box is killed for its own experience at 20 mB a point (1-3 points, so 20, 40 or 60 mB) for exactly 50 FE, its loot lands in the machine's 18-slot output instead of the floor and a hopper under it pulls that loot into a barrel, the same box with no FE leaves the cow alive and adds nothing, the crusher's own shaped recipe crafts from #c:plastics, #c:gears/gold and machine_frame/advanced, and a broken crusher carries its essence, its buffer and its last kill's experience back on its item"
