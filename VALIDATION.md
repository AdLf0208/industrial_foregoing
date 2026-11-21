# Validation — `industrialforegoing` 0.2.5

The ladder actually run for the delivered package, cheapest first. Every compile went through
`ports/tools/build-slot.sh` (`BUILD_LOCAL=1`, `CARGO_INCREMENTAL=0`, no `--target-dir`); `check` and
both `fits` runs are the tool's own non-compiling checks, run in the port. The in-game legs are
launcher builds of this port at the campaign pin
**`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`** + mixins `e59da909…`.
Evidence: `ports/_shared/evidence/industrialforegoing/` and
`ports/_shared/evidence/industrialforegoing-e2e/`.

| step | command | result | evidence |
|---|---|---|---|
| red | `cargo test` with slice 5's rules as tests and their bodies at the pre-implementation state | **60 passed, 15 failed** (`the_exact_item_set_matches`, `a_hundred_tick_recipe_takes_a_hundred_ticks_and_nine_thousand_fe`, …) | `industrialforegoing/sdk-test-red-slice5.log` |
| tests | `cargo test` | **75 passed**, 0 failed (46 carried + 20 chamber rules + 9 content/glue) | `industrialforegoing/sdk-test.log` |
| format | `cargo fmt --package industrialforegoing --check` | clean (`FMT-CLEAN`) | `industrialforegoing/sdk-clippy.log` |
| lints | `cargo clippy --all-targets -- -D warnings` | exit 0, 0 warnings | `industrialforegoing/sdk-clippy.log` |
| content | `python3 tools/verify_content.py` (tree), `--selftest` | `CONTENT OK`; the selftest catches four mutations, incl. the chamber's glass-pane tag and its declaration | `industrialforegoing/sdk-content-slice5.log` |
| package | `rm -f Cargo.lock && cargo horizon-mod build` ×2 | `cmp`-identical (sha256 `0876ea23…`), **268** entries, no packed `Cargo.lock` | `industrialforegoing/sdk-build-slice5.log` |
| check | `cargo horizon-mod check industrialforegoing.hmod` | `industrialforegoing 0.2.5 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, **189,268 B**, sha256 `0876ea23…ad`, exit 0 | `industrialforegoing/sdk-check.log` (prints the sha it ran against) |
| fits (pin) | `cargo horizon-mod fits industrialforegoing.hmod --build 0a875da` | `0a875da  made for`, exit 0 | `industrialforegoing/sdk-fits.log` |
| fits (other) | `… --build 0b14f8a` | `0b14f8a  fits`, exit 0 | `industrialforegoing/sdk-fits.log` |
| package gate | `python3 ports/tools/verify-sdk-package.py industrialforegoing` | **1/1** (runtime byte-identical `83ae3b11…`, check+fits, skills) | — |
| in-game, green | `launcher-game.sh build` (log says `Compiling industrialforegoing (1 of 1)`) + `tools/e2e.sh` (phases A–AE) | **`PASS` + `EXIT:0`** | `industrialforegoing-e2e/e2e-launcher.log` |
| control | the same script on `GAME=$CONTROL_GAME` (same Nightly, no package) | `FAIL: the machine item did not register` → `EXIT:1` | `industrialforegoing-e2e/e2e-launcher-control.log` |
| selftest | `SELFTEST=1` (phase B 200 → 201) | `FAIL: the water is 200, expected 201` → `EXIT:1` | `industrialforegoing-e2e/e2e-launcher-selftest.log` |
| selftest, core | `SELFTEST=core` (phase U's red-lens count 1 → 2) | `FAIL: the red lens count after the craft is 1, expected 2` → `EXIT:1` | `industrialforegoing-e2e/e2e-launcher-selftest-core.log` |
| selftest, chamber | `SELFTEST=chamber` (phase Z's lens count 1 → 2) | `FAIL: the product is industrialforegoing:white_laser_lens x1, expected … x2` → `EXIT:1` | `industrialforegoing-e2e/e2e-launcher-selftest-chamber.log` |
| log gate | `python3 ports/tools/check-e2e-logs.py industrialforegoing-e2e --strict` | `logs=5 ok=5 missing-exit=0 bad-exit=0` | — |
| skills gate | `python3 ports/tools/check-skills.py --notes industrialforegoing` | PASS | `verify-sdk-package.py` row |

Artifact: **189,268 B**, SHA-256
`0876ea235c4fd196113992cd743c116d513add5e408e723dc2ec7bf34c7b2bad`, 268 entries, no packed
`Cargo.lock`, `src/runtime.rs` byte-identical to the tree (`83ae3b111494…`).

The 0.2.4 ladder (slice 4, kept as history): tests 46; `check` 153,549 B
`dd43090d…08f`; `0a875da  made for` / `0b14f8a  fits`; e2e phases A–U `PASS` + `EXIT:0`, control and
all selftests `EXIT:1`. The 0.2.3 ladder (slice 3): tests 39; `check` 59,334 B
`819f00fc…59e7c`. The 0.2.2 ladder (slice 2): tests 26; `check` 34,096 B `2672a800…4826`.
