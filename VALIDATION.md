# Validation — `industrialforegoing` 0.2.4

The ladder actually run for the delivered package, cheapest first. Every compile went through
`ports/tools/build-slot.sh` (`BUILD_LOCAL=1`, `CARGO_INCREMENTAL=0`, no `--target-dir`); `check` and
both `fits` runs are the tool's own non-compiling checks, run in the port. The in-game legs are
launcher builds of this port at the campaign pin
**`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`** + mixins `e59da909…`.
Evidence: `ports/_shared/evidence/industrialforegoing/` and
`ports/_shared/evidence/industrialforegoing-e2e/`.

| step | command | result | evidence |
|---|---|---|---|
| red | `cargo test` with the seven `content` tests and no slice-4 files | **39 passed, 7 failed** (missing ids, tags, recipes, loot) | `industrialforegoing/sdk-test-red-slice4.log` |
| tests | `cargo test` | **46 passed**, 0 failed (39 carried + 7 slice-4 content tests) | `industrialforegoing/sdk-test.log` |
| format | `cargo fmt --package industrialforegoing --check` | clean (`FMT-CLEAN`) | `industrialforegoing/sdk-clippy.log` |
| lints | `cargo clippy --all-targets -- -D warnings` | exit 0, 0 warnings | `industrialforegoing/sdk-clippy.log` |
| content | `python3 tools/verify_content.py` (tree), `--package industrialforegoing.hmod` (delivered bytes), `--selftest` | `CONTENT OK` twice; the selftest catches three mutations | `industrialforegoing/sdk-content-slice4.log` |
| package | `rm -f Cargo.lock && cargo horizon-mod build` ×2 | `cmp`-identical (sha256 `dd43090d…`), **230** entries, no packed `Cargo.lock` | `industrialforegoing/sdk-build-slice4.log` |
| check | `cargo horizon-mod check industrialforegoing.hmod` | `industrialforegoing 0.2.4 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, **153,549 B**, sha256 `dd43090d…08f`, exit 0 | `industrialforegoing/sdk-check.log` (prints the sha it ran against) |
| fits (pin) | `cargo horizon-mod fits industrialforegoing.hmod --build 0a875da` | `0a875da  made for`, exit 0 | `industrialforegoing/sdk-fits.log` |
| fits (other) | `… --build 0b14f8a` | `0b14f8a  fits`, exit 0 | `industrialforegoing/sdk-fits.log` |
| package gate | `python3 ports/tools/verify-sdk-package.py industrialforegoing` | **1/1** (runtime byte-identical `83ae3b11…`, 46 tests, 0 clippy warnings, check+fits, skills) | — |
| in-game, green | `launcher-game.sh build` (log says `Compiling industrialforegoing (1 of 1)`) + `tools/e2e.sh` (phases A–U) | **`PASS` + `EXIT:0`** | `industrialforegoing-e2e/e2e-launcher.log` |
| control | the same script on `GAME=$CONTROL_GAME` (same Nightly, no package) | `FAIL: the machine item did not register` → `EXIT:1` | `industrialforegoing-e2e/e2e-launcher-control.log` |
| selftest | `SELFTEST=1` (phase B 200 → 201) | `FAIL: the water is 200, expected 201` → `EXIT:1` | `industrialforegoing-e2e/e2e-launcher-selftest.log` |
| selftest, core | `SELFTEST=core` (phase U's red-lens count 1 → 2) | `FAIL: the red lens count after the craft is 1, expected 2` → `EXIT:1` | `industrialforegoing-e2e/e2e-launcher-selftest-core.log` |
| log gate | `python3 ports/tools/check-e2e-logs.py industrialforegoing-e2e --strict` | `logs=4 ok=4 missing-exit=0 bad-exit=0` | — |
| skills gate | `python3 ports/tools/check-skills.py --notes ports/industrialforegoing` | PASS | `verify-sdk-package.py` row |

Artifact: **153,549 B**, SHA-256
`dd43090d0f7a96b01f82d842cf42cd2f59f45e9d2763c524d65187c320b9508f`, 230 entries, no packed
`Cargo.lock`, `src/runtime.rs` byte-identical to the tree (`83ae3b111494…`).

The 0.2.3 ladder (slice 3, kept as history): tests 39; `check` 59,334 B
`819f00fc…59e7c`; `0a875da  made for` / `0b14f8a  fits`; e2e phases A–Q `PASS` + `EXIT:0`, control and
both selftests `EXIT:1`. Its logs are in `industrialforegoing/retired-0.2.3/` and
`industrialforegoing-e2e/retired-0.2.3/`.

The 0.2.2 ladder (slice 2): tests 26; `check` 34,096 B
`2672a800…4826`; `0b14f8a  fits`; e2e phases A–L `PASS` + `EXIT:0`, control and both selftests
`EXIT:1`.
