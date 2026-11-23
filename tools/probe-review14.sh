#!/usr/bin/env bash
# Review round two (0.2.13): the four in-game assertions, as a **before/after differential**.
#
# Run against the delivered 0.2.12 package (the export under
# `~/.cache/horizon-e2e/0a875da/industrialforegoing/`, built before the repair) it fails at the
# first leg; each leg can also be selected alone with `LEG=dh|di|dj|dk` so every red is its own
# run. Run against 0.2.13 it is green. The same four legs live in `tools/e2e.sh` (phases DH–DK);
# this script exists because the full scenario stops at its first failing assertion.
#
#   eval "$(ports/tools/launcher-game.sh env industrialforegoing)"
#   LEG=dk ports/industrialforegoing/tools/probe-review14.sh
#
# `E2E_TMP` is honoured: `/tmp` is a 7.5 GB tmpfs.
set -Eeuo pipefail
export LC_ALL=C

CLONE=${CLONE:-/home/adlf/Desktop/all_the_mods_10_rust_rewrite}
PORT=$CLONE/ports/industrialforegoing
CTL=${CTL:-}
GAME=${GAME:-}
RES=${RES:-${HORIZON_RES:-/home/adlf/Desktop/horizon/.local/horizon-client/26.3}}
SOCK=${SOCK:-${E2E_TMP:-/tmp/opencode}/industrialforegoing-review14.sock}
LEG=${LEG:-all}
WORLD=IndustrialForegoingReview14
SELFTEST=${SELFTEST:-0}
X0=380

[ -n "$CTL" ] && [ -x "$CTL" ] || { echo "CTL is unset or missing" >&2; exit 1; }
[ -n "$GAME" ] && [ -x "$GAME" ] || { echo "GAME is unset or missing" >&2; exit 1; }

SCRATCH=$(mktemp -d "${E2E_TMP:-/tmp/opencode}"/industrialforegoing-review14.XXXXXX)
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

cleanup() { ctl quit >/dev/null 2>&1 || true; sleep 1; rm -rf "$SCRATCH" 2>/dev/null || true; }
trap 'status=$?; cleanup; echo "EXIT:$status"' EXIT

launch() {
    rm -f "$SOCK"
    cd "$SCRATCH"
    "$CTL" --socket "$SOCK" launch --game "$GAME" --world "$WORLD" --seed 20261105 --creative \
        --offscreen --wait --resources "$RES" >/dev/null
    ctl wait-ready 120
    ctl resume >/dev/null 2>&1 || true
    for _ in $(seq 1 30); do
        ctl chat "/time query gametime" >/dev/null 2>&1 || true
        ctl wait-for chat "The game time is" --timeout 10 >/dev/null 2>&1 && return 0
    done
    return 1
}

block_name() { ctl block "$1" "$2" "$3" | jq -r '.name // .block // empty'; }

arena() {
    ctl command "difficulty peaceful" >/dev/null
    ctl command "execute in minecraft:the_end run tp @s $X0.5 104 0.5" >/dev/null
    for _ in $(seq 1 120); do
        ctl wait 5 >/dev/null
        [ "$(ctl block "$X0" 100 0 | jq -r '.loaded // false')" = true ] && break
    done
    ctl command "execute in minecraft:the_end run fill $((X0 - 20)) 100 -10 $((X0 + 40)) 100 10 minecraft:stone" >/dev/null
    ctl command "execute in minecraft:the_end run fill $((X0 - 20)) 101 -10 $((X0 + 40)) 108 10 minecraft:air" >/dev/null
    ctl command "execute in minecraft:the_end run kill @e[type=!player]" >/dev/null
    ctl wait 8 >/dev/null
    ctl command "execute in minecraft:the_end run tp @s $X0.5 101 0.5" >/dev/null
    ctl wait 20 >/dev/null
}

# Place `item` at X Y Z from a `/give`n stack, aimed at the floor under it.
place_any() {
    local x=$1 y=$2 z=$3 item=$4
    ctl command "gamemode creative @s" >/dev/null
    ctl command "item replace entity @s hotbar.0 with $item 1" >/dev/null
    ctl wait 6
    ctl select 1 >/dev/null
    ctl command "execute in minecraft:the_end run tp @s $((x - 2)).5 $y $z.5" >/dev/null
    ctl wait 6
    ctl look-at-block "$x" $((y - 1)) "$z" up >/dev/null 2>&1 || fail "cannot aim at the floor under $x,$y,$z"
    ctl wait 8
    ctl place "$x" "$y" "$z" --item "$item" >/dev/null || fail "$item did not place at $x,$y,$z"
    ctl wait 6
    [ "$(block_name "$x" "$y" "$z")" = "$item" ] || fail "$item did not place at $x,$y,$z"
}

want() { # want LEG
    [ "$LEG" = all ] || [ "$LEG" = "$1" ]
}

echo "== the four review-round-two legs, LEG=$LEG =="
launch || fail "the game never answered a chat round trip"
arena
echo "  game: $(basename "$(dirname "$GAME")")"

if want dh; then
    echo "== DH: a broken latex processing unit's item carries no Items =="
    UX=$((X0 + 2))
    UNIT=industrialforegoing:latex_processing_unit
    place_any "$UX" 101 0 "$UNIT"
    # Give the machine something to carry: `has_contents` is the machine's own keys, and an item
    # that carries nothing is not the case under test.
    say "industrialforegoing energy $UX 101 0 4000"
    say "industrialforegoing fluid $UX 101 0 latex 500"
    ctl wait 4
    ctl command "item replace block $UX 101 0 container.0 with industrialforegoing:dryrubber 2" >/dev/null
    ctl wait 4
    ctl command "item replace block $UX 101 0 container.1 with industrialforegoing:dryrubber 3" >/dev/null
    ctl wait 4
    ctl command "execute in minecraft:the_end run tp @s $((UX - 2)).5 101 0.5" >/dev/null
    ctl wait 6
    ctl command "gamemode survival @s" >/dev/null
    ctl wait 4
    ctl command "clear @s" >/dev/null
    ctl command "item replace entity @s hotbar.0 with minecraft:iron_pickaxe 1" >/dev/null
    ctl wait 4
    ctl select 1 >/dev/null
    ctl wait 3
    BROKEN=$(ctl break "$UX" 101 0) || fail "breaking the unit failed"
    echo "  break: $(echo "$BROKEN" | jq -c '{drops, picked_up}')"
    [ "$(echo "$BROKEN" | jq -r ".drops[\"$UNIT\"] // 0")" -ge 1 ] || fail "the unit dropped no item: $BROKEN"
    case "$(echo "$BROKEN" | jq -c '.drops')" in
        *industrialforegoing:dryrubber*) ;;
        *) fail "the engine did not spill the unit's slots: $(echo "$BROKEN" | jq -c '.drops')" ;;
    esac
    ctl command "execute in minecraft:the_end run tp @s $UX.5 101 0.5" >/dev/null
    ctl wait 20
    ctl command "gamemode creative @s" >/dev/null
    ctl wait 4
    ctl select "$UNIT" >/dev/null 2>&1 || ctl select 1 >/dev/null
    ctl wait 4
    ITEM=$(ctl command "data get entity @s SelectedItem" | jq -r '.chat[0] // empty')
    echo "  carried item: $ITEM"
    case "$ITEM" in
        *'"minecraft:block_entity_data"'*) ;;
        *) fail "the item carried no block entity data: $ITEM" ;;
    esac
    case "$ITEM" in
        *Items*) fail "the carried payload still holds Items: $ITEM" ;;
    esac
    echo "DH PASS: the broken unit's item carries its machine data and no Items"
fi

if want di; then
    echo "== DI: a named, damaged stack survives the crusher's output merge =="
    CX=$((X0 + 10))
    CRUSHER=industrialforegoing:mob_crusher
    place_any "$CX" 101 0 "$CRUSHER"
    for slot in $(seq 0 17); do
        ctl command "item replace block $CX 101 0 container.$slot with minecraft:air" >/dev/null
    done
    ctl wait 4
    ctl command "item replace block $CX 101 0 container.0 with minecraft:beef[minecraft:custom_name='\"Named Beef\"',minecraft:damage=0] 1" >/dev/null
    ctl command "item replace block $CX 101 0 container.5 with minecraft:iron_sword[minecraft:custom_name='\"Named Sword\"',minecraft:damage=17] 1" >/dev/null
    ctl wait 6
    say "industrialforegoing energy $CX 101 0 5000"
    say "industrialforegoing progress $CX 101 0 100"
    ctl wait 2 >/dev/null
    ctl command "execute in minecraft:the_end run summon minecraft:cow $((CX + 1)).5 101 0.5 {NoAI:1b,Silent:1b,PersistenceRequired:1b}" >/dev/null
    ctl wait 8 >/dev/null
    say "industrialforegoing progress $CX 101 0 1"
    ctl wait 8 >/dev/null
    ITEMS=$(ctl command "data get block $CX 101 0 Items" | jq -r '.chat[0] // empty')
    echo "  the container: $ITEMS"
    [ -n "$ITEMS" ] || fail "the container read back empty"
    COUNT=$(grep -o 'Slot: [0-9]*b' <<<"$ITEMS" | wc -l)
    echo "  the container holds $COUNT stack(s)"
    NAMED=0
    NAMED_COUNT=0
    SWORD=0
    SWORD_DAMAGE=0
    PLAIN=0
    for i in $(seq 0 $((COUNT - 1))); do
        ENTRY=$(ctl command "data get block $CX 101 0 Items[$i]" | jq -r '.chat[0] // empty')
        echo "  Items[$i]: $ENTRY"
        case "$ENTRY" in
            *'"Named Beef"'*)
                NAMED=1
                NAMED_COUNT=$(sed -n 's/.*count: \([0-9][0-9]*\).*/\1/p' <<<"$ENTRY")
                ;;
            *'"Named Sword"'*)
                SWORD=1
                SWORD_DAMAGE=$(sed -n 's/.*"minecraft:damage": \([0-9][0-9]*\).*/\1/p' <<<"$ENTRY")
                ;;
            *'id: "minecraft:beef"'*)
                PLAIN=$((PLAIN + 1))
                ;;
        esac
    done
    [ "$NAMED" = 1 ] || fail "the named stack left the container (its name was lost in the merge)"
    [ "${NAMED_COUNT:-0}" = 1 ] || fail "the cow's beef merged into the named stack (its count is ${NAMED_COUNT:-absent})"
    [ "$PLAIN" -ge 1 ] || fail "the cow's beef landed nowhere beside the named stack"
    [ "$SWORD" = 1 ] || fail "the damaged named sword left the container"
    [ "${SWORD_DAMAGE:-0}" = 17 ] || fail "the damaged named sword lost its damage (${SWORD_DAMAGE:-absent})"
    echo "DI PASS: the plain beef went to its own slot, and the named and damaged stacks are intact"
fi

if want dj; then
    echo "== DJ: a full output container gives the reward back to the world =="
    DX=$((X0 + 20))
    CRUSHER=industrialforegoing:mob_crusher
    ctl command "execute in minecraft:the_end run kill @e[type=minecraft:item]" >/dev/null
    place_any "$DX" 101 0 "$CRUSHER"
    for slot in $(seq 0 17); do
        ctl command "item replace block $DX 101 0 container.$slot with minecraft:netherrack 64" >/dev/null
    done
    ctl wait 6
    say "industrialforegoing energy $DX 101 0 5000"
    say "industrialforegoing progress $DX 101 0 100"
    ctl wait 2 >/dev/null
    ctl command "execute in minecraft:the_end run summon minecraft:cow $((DX + 1)).5 101 0.5 {NoAI:1b,Silent:1b,PersistenceRequired:1b}" >/dev/null
    ctl wait 8 >/dev/null
    say "industrialforegoing progress $DX 101 0 1"
    ctl wait 10 >/dev/null
    echo "  the container after: $(ctl command "data get block $DX 101 0 Items" | jq -r '.chat[0] // empty' | cut -c1-300)"
    echo "  the items on the ground: $(ctl entities 24 item | jq -c '[.entities[] | {item: .item.item, count: (.item.count // 1), pos}]')"
    GROUND=$(ctl entities 24 item | jq -r --argjson x0 $((DX - 2)) --argjson x1 $((DX + 3)) '
      [.entities[] | select(.kind == "item")
       | select(.pos != null)
       | select((.item.item // "") | test("minecraft:(beef|leather)$"))
       | select((.pos[0] | floor) >= $x0 and (.pos[0] | floor) <= $x1)
      ] | length')
    echo "  the cow's loot on the floor: $GROUND"
    [ "$GROUND" -ge 1 ] || fail "a full output container lost the cow's loot (nothing on the floor)"
    echo "DJ PASS: the reward the container could not take is on the floor, not lost"
fi

if want dk; then
    echo "== DK: a fluid laser base at an ineligible depth applies no gate damage =="
    FX=$((X0 + 30))
    BASE=industrialforegoing:fluid_laser_base
    DRILL=industrialforegoing:laser_drill
    place_any "$FX" 101 0 "$BASE"
    ctl command "gamemode creative @s" >/dev/null
    ctl command "item replace entity @s hotbar.0 with $DRILL 1" >/dev/null
    ctl wait 6
    ctl select 1 >/dev/null
    ctl command "execute in minecraft:the_end run tp @s $((FX + 1)).5 106 0.5" >/dev/null
    ctl wait 6
    ctl look-at-block "$FX" 101 0 up >/dev/null 2>&1 || fail "cannot aim at the base's top face"
    ctl wait 4
    ctl place "$FX" 102 0 --item "$DRILL" >/dev/null || fail "the drill did not place above the base"
    ctl wait 6
    [ "$(block_name "$FX" 102 0)" = "$DRILL" ] || fail "the drill is not above the base"
    ctl command "item replace block $FX 101 0 container.0 with industrialforegoing:purple_laser_lens 1" >/dev/null
    say "industrialforegoing depth $FX 101 0 300"
    ctl command "difficulty easy" >/dev/null
    ctl command "execute in minecraft:the_end run kill @e[type=minecraft:wither]" >/dev/null
    ctl command "execute in minecraft:the_end run summon minecraft:wither $((FX + 1)) 102 0 {NoAI:1b,Silent:1b,PersistenceRequired:1b,Health:100.0f}" >/dev/null
    ctl wait 8 >/dev/null
    health() {
        ctl command "data get entity @e[type=minecraft:wither,limit=1,sort=nearest] Health" \
            | jq -r '.chat[0] // empty' | sed -n 's/.*data: \([0-9.]*\)f*.*/\1/p'
    }
    BEFORE=$(health)
    echo "  the wither's health before: $BEFORE"
    [ -n "$BEFORE" ] || fail "the wither has no readable health"
    say "industrialforegoing progress $FX 101 0 19"
    say "industrialforegoing progress $FX 102 0 1"
    say "industrialforegoing energy $FX 102 0 10000"
    ctl wait 8 >/dev/null
    AFTER=$(health)
    echo "  the wither's health after one bar at depth 300: $AFTER"
    [ -n "$AFTER" ] || fail "the wither's health did not read back"
    [ "$AFTER" = "$BEFORE" ] || fail "an ineligible base still damaged the gate entity ($BEFORE -> $AFTER)"
    echo "DK PASS: depth 300 is outside the ether band and the wither's health is unchanged"
    ctl command "execute in minecraft:the_end run kill @e[type=minecraft:wither]" >/dev/null
    ctl command "difficulty peaceful" >/dev/null
fi

echo "PROBE PASS"
