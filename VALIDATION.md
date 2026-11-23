# Validation — `industrialforegoing` 0.2.13

The ladder actually run for the delivered package, cheapest first. Every compile went through
`ports/tools/build-slot.sh` (`BUILD_LOCAL=1`, `CARGO_INCREMENTAL=0`, no `--target-dir`); `check` and
both `fits` runs are the tool's own non-compiling checks, run in the port. The in-game legs are
launcher builds of this port at the campaign pin
**`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`** + mixins `e59da909…`; the launcher's own output reads
`Compiling industrialforegoing (1 of 1)` and the exported manifest's `version = "0.2.13"`. Evidence:
`ports/_shared/evidence/industrialforegoing/` and `ports/_shared/evidence/industrialforegoing-e2e/`.

The slice is **the review round two repair** — the four accepted blockers: the carried payloads as
allowlists (no `Items`), the crusher's whole-stack merge with instance-bound, world-returned rewards,
the powered-attempt-only mob queries / released fake actor / laser eligibility order, and the upstream
copyright line. No new content; `src/runtime.rs` untouched.

| step | command | result | evidence |
|---|---|---|---|
| tests, red | `cargo test` with the ten new tests on the 0.2.12 sources | **233 passed, 8 failed** (scan counts `5 != 0` ×2, the marked stack grew to 2, three payloads still carried `Items`, the binding seam `not yet implemented` ×2) | `industrialforegoing/sdk-test-red-slice14.log` |
| tests | `cargo test` | **242 passed**, 0 failed (`Compiling industrialforegoing v0.2.13`) | `industrialforegoing/sdk-test.log` |
| format | `cargo fmt --check` | clean | `sdk-clippy.log` (same lease) |
| lints | `cargo clippy --all-targets -- -D warnings` | exit 0, **0 warnings** | `industrialforegoing/sdk-clippy.log` |
| content | `python3 tools/verify_content.py`, `--package industrialforegoing.hmod`, `--selftest` | `CONTENT OK: … 433 content files` for the tree **and** the delivered `.hmod`; `selftest: OK` | `industrialforegoing/sdk-content-slice14.log` |
| package | `rm -f Cargo.lock && cargo horizon-mod build` ×2 | `cmp`-identical, sha256 `a0c1d8cd…` both times, **457** entries, no packed `Cargo.lock` | `industrialforegoing/sdk-check.log` |
| check | `cargo horizon-mod check industrialforegoing.hmod` | `industrialforegoing 0.2.13 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, **398,113 B**, sha256 `a0c1d8cd75db5f24149f0c2362a73d79f5e8b79fa5136c34fa99ca23b35648b7`, exit 0 | `industrialforegoing/sdk-check.log` |
| fits (pin) | `cargo horizon-mod fits industrialforegoing.hmod --build 0a875da` | `0a875da  made for`, exit 0 | `industrialforegoing/sdk-fits.log` |
| fits (other) | `… --build 0b14f8a` | `0b14f8a  fits`, exit 0 | `industrialforegoing/sdk-fits.log` |
| recipe types | `python3 ports/tools/check-recipe-types.py industrialforegoing` | `industrialforegoing: ok` | `industrialforegoing/sdk-recipe-types.log` |
| runtime | packed `src/runtime.rs` = tree | identical, `83ae3b111494…` | gate row below |
| package gate | `python3 ports/tools/verify-sdk-package.py industrialforegoing` | **1/1** (runtime byte-identical, **package matches the tree**, R9 clean, no packed lock, 242 tests, 0 warnings, check+`fits`, skills) | `industrialforegoing/sdk-gate-0.2.13.log` |
| in-game, green | `launcher-game.sh build industrialforegoing --port …` + `tools/e2e.sh` (phases A–DK) | **`PASS` + `EXIT:0`**; the new DH–DK block is quoted in `notes.md` | `industrialforegoing-e2e/e2e-launcher.log` |
| in-game, the four reds | `tools/probe-review14.sh` (one leg each) against the **delivered 0.2.12** export | DH `FAIL: the carried payload still holds Items`; DI `FAIL: the named stack left the container (its name was lost in the merge)`; DJ `FAIL: a full output container lost the cow's loot`; DK `FAIL: an ineligible base still damaged the gate entity (100.0 -> 95.0)` — all `EXIT:1` | `industrialforegoing-e2e/review14-before-{dh,di,dj,dk}.log` |
| in-game, the same probe after | `tools/probe-review14.sh` on 0.2.13 | `DH PASS`, `DI PASS`, `DJ PASS`, `DK PASS`, `PROBE PASS` + `EXIT:0` | `industrialforegoing-e2e/review14-after.log` |
| control | the same script on `GAME=$CONTROL_GAME` (same Nightly, no package) | `FAIL: the machine item did not register` → `EXIT:1` | `…-control.log` |
| selftest, base | `SELFTEST=1` (phase B 200 → 201) | `FAIL: the water is 200, expected 201` → `EXIT:1` | `…-selftest.log` |
| log gate | `python3 ports/tools/check-e2e-logs.py industrialforegoing-e2e --strict` | `logs=24 ok=24 missing-exit=0 bad-exit=0` | run in this report |
| skills gate | `python3 ports/tools/check-skills.py --notes industrialforegoing` | PASS | `verify-sdk-package.py` row |

Artifact: **398,113 B**, SHA-256
`a0c1d8cd75db5f24149f0c2362a73d79f5e8b79fa5136c34fa99ca23b35648b7`, **457** entries, no packed
`Cargo.lock`, `src/runtime.rs` byte-identical to the tree (`83ae3b111494…`), `LICENSE` line 3
`Copyright (c) 2017 Buuz135`. The in-game reads that matter: the broken unit's
`minecraft:block_entity_data` with `energy: 4000` and its latex and no `Items` while five dry rubber
drop; the crusher's container holding `"Named Beef"` at `count: 1` beside a plain `minecraft:beef`
stack and a `"Named Sword"` at `"minecraft:damage": 17`; a full 18-slot container leaving the cow's
beef and leather on the floor; and a base at depth 300 whose wither reads `100.0` before and after a
completed bar with `recipe none`.

## Notes on this run

- **The launcher reuses a build whose `horizon-mod.toml` is byte-identical**, so every `native-*` tag
  carrying this mod is removed before each rebuild (the exported
  `horizon-native-mods/industrialforegoing/` was deleted first). The delivered tag is
  `native-0a875da-76ffa82b6863`.
- **The latex and laser arenas overlap** (x 232–248): the laser arena is laid after the latex legs and
  clears the unit at LXU, which is why the new DH leg sets its own floor and places its own unit, as
  the chamber's `outputFluid` leg does.
- **Phase C flaked twice before the green run** (`the sources is 2, expected 1`) — the condensator
  counting a second water source from the previous leg, the flow interference slice 2's notes record.
  It is not in any path this slice changed; the green run and the runs after it were clean.
- **The crusher's floor helper was reading `.position` where `ctl entities` emits `.pos`** — that made
  phase CO's "nothing on the floor" true only because no item entity existed, and made DJ impossible
  (jq errored on the null). The helper now reads `.pos`; CO and DJ both pass and both can fail.
