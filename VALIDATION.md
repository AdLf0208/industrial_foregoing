# Validation — `industrialforegoing` 0.2.6

The ladder actually run for the delivered package, cheapest first. Every compile went through
`ports/tools/build-slot.sh` (`BUILD_LOCAL=1`, `CARGO_INCREMENTAL=0`, no `--target-dir`); `check` and
both `fits` runs are the tool's own non-compiling checks, run in the port. The in-game legs are
launcher builds of this port at the campaign pin
**`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`** + mixins `e59da909…`, with the log's own
`Compiling industrialforegoing (1 of 1)`.
Evidence: `ports/_shared/evidence/industrialforegoing/` and
`ports/_shared/evidence/industrialforegoing-e2e/`.

| step | command | result | evidence |
|---|---|---|---|
| red | `cargo test` with slice 6's rules as tests and their three bodies at `todo!()` | **78 passed, 17 failed** (`recipe_for`, the extractor's `tick`, the unit's `tick`; every failure `not yet implemented`) | `industrialforegoing/sdk-test-red-slice6.log` |
| tests | `cargo test` | **105 passed**, 0 failed (78 carried + 17 latex rules + 10 latex glue) | `industrialforegoing/sdk-test.log` |
| format | `cargo fmt --check` | clean (`FMT-CLEAN`) | `industrialforegoing/sdk-clippy.log` |
| lints | `cargo clippy --all-targets -- -D warnings` | exit 0, 0 warnings | `industrialforegoing/sdk-clippy.log` |
| content | `python3 tools/verify_content.py` (tree), `--selftest`, `--package` | `CONTENT OK`; the selftest catches six mutations (incl. the latex chain's cobblestone tag and an extractor recipe) | `industrialforegoing/sdk-content-slice6.log` |
| package | `rm -f Cargo.lock && cargo horizon-mod build` ×2 | `cmp`-identical (sha256 `d9dbb231…`), **305** entries, no packed `Cargo.lock` | `industrialforegoing/sdk-check.log` |
| check | `cargo horizon-mod check industrialforegoing.hmod` | `industrialforegoing 0.2.6 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, **225,305 B**, sha256 `d9dbb2316b701dab2bf7877bf2361772d3f634113dd75291ac9e6e172b4869a4`, exit 0 | `industrialforegoing/sdk-check.log` |
| fits (pin) | `cargo horizon-mod fits industrialforegoing.hmod --build 0a875da` | `0a875da  made for`, exit 0 | `industrialforegoing/sdk-check.log` |
| fits (other) | `… --build 0b14f8a` | `0b14f8a  fits`, exit 0 | `industrialforegoing/sdk-check.log` |
| package gate | `python3 ports/tools/verify-sdk-package.py industrialforegoing` | **1/1** (runtime byte-identical `83ae3b11…`, check+`fits`, 105 tests, 0 warnings, skills) | — |
| in-game, green | `launcher-game.sh build` + `tools/e2e.sh` (phases A–AM) | **`PASS` + `EXIT:0`** | `industrialforegoing-e2e/e2e-launcher.log` |
| control | the same script on `GAME=$CONTROL_GAME` (same Nightly, no package) | `FAIL: the machine item did not register` → `EXIT:1` | `…-control.log` |
| selftest | `SELFTEST=1` (phase B 200 → 201) | `FAIL: the water is 200, expected 201` → `EXIT:1` | `…-selftest.log` |
| selftest, contents | `SELFTEST=contents` (phase L 5,000 → 5,001) | `FAIL: the restored tank is 5000, expected 5001` → `EXIT:1` | `…-selftest-contents.log` |
| selftest, core | `SELFTEST=core` (phase U's red-lens count 1 → 2) | `FAIL: the red lens count after the craft is 1, expected 2` → `EXIT:1` | `…-selftest-core.log` |
| selftest, chamber | `SELFTEST=chamber` (phase Z's lens count 1 → 2) | `FAIL: the product is industrialforegoing:white_laser_lens x1, expected … x2` → `EXIT:1` | `…-selftest-chamber.log` |
| selftest, latex | `SELFTEST=latex` (phase AF's 2 mB → 3) | `FAIL: the latex one unpowered operation gives is 3, expected 2` → `EXIT:1` | `…-selftest-latex.log` |
| log gate | `python3 ports/tools/check-e2e-logs.py industrialforegoing-e2e --strict` | `logs=7 ok=7 missing-exit=0 bad-exit=0` | — |
| skills gate | `python3 ports/tools/check-skills.py --notes industrialforegoing` | PASS | `verify-sdk-package.py` row |

Artifact: **225,305 B**, SHA-256
`d9dbb2316b701dab2bf7877bf2361772d3f634113dd75291ac9e6e172b4869a4`, 305 entries, no packed
`Cargo.lock`, `src/runtime.rs` byte-identical to the tree (`83ae3b111494…`).

The 0.2.5 ladder (slice 5, kept as history): tests 75; `check` 189,268 B
`0876ea23…bad`; `0a875da  made for` / `0b14f8a  fits`; e2e phases A–AE `PASS` + `EXIT:0`, control and
all four selftests `EXIT:1`. The 0.2.4 ladder (slice 4): tests 46; `check` 153,549 B
`dd43090d…08f`. The 0.2.3 ladder (slice 3): tests 39; `check` 59,334 B
`819f00fc…59e7c`. The 0.2.2 ladder (slice 2): tests 26; `check` 34,096 B `2672a800…4826`.
