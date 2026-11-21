# Validation — `industrialforegoing` 0.2.3

The ladder actually run for the delivered package, cheapest first. Every command went through
`ports/tools/build-slot.sh` (`BUILD_LOCAL=1`, `CARGO_INCREMENTAL=0`, no `--target-dir`); the
in-game legs are launcher builds of this port at the campaign pin
**`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`** + mixins `e59da909…`.
Evidence: `ports/_shared/evidence/industrialforegoing/` and
`ports/_shared/evidence/industrialforegoing-e2e/`.

| step | command | result | evidence |
|---|---|---|---|
| red | `cargo test` with the augment helpers as `todo!()` | **16 passed, 23 failed** (`not yet implemented`) | `industrialforegoing/sdk-test-red-slice3.log` |
| tests | `cargo test` | **39 passed**, 0 failed (26 carried + 13 new) | `industrialforegoing/sdk-test-slice3.log` = `sdk-test.log` |
| format | `cargo fmt --check` | clean (`FMT-CLEAN`) | `industrialforegoing/sdk-clippy-slice3.log` |
| lints | `cargo clippy --all-targets -- -D warnings` | exit 0, 0 warnings | `industrialforegoing/sdk-clippy-slice3.log` |
| package | `rm -f Cargo.lock && cargo horizon-mod build` ×2 | `cmp`-identical (`819f00fc…`), **41** entries, no packed `Cargo.lock` | `industrialforegoing/sdk-build-slice3.log` |
| check | `cargo horizon-mod check industrialforegoing.hmod` | `industrialforegoing 0.2.3 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, **59,334 B**, sha256 `819f00fc…59e7c`, exit 0 | `industrialforegoing/sdk-check-slice3.log` |
| fits (pin) | `cargo horizon-mod fits industrialforegoing.hmod --build 0a875da` | `0a875da  made for`, exit 0 | `industrialforegoing/sdk-fits-slice3.log` |
| fits (other) | `… --build 0b14f8a` | `0b14f8a  fits`, exit 0 | `industrialforegoing/sdk-fits-slice3.log` |
| package gate | `python3 ports/tools/verify-sdk-package.py industrialforegoing` | **1/1** (runtime.rs byte-identical, 39 tests, 0 clippy warnings, check+fits, skills) | — |
| in-game, green | launcher build + `tools/e2e.sh` (phases A–Q) | **`PASS` + `EXIT:0`** | `industrialforegoing-e2e/e2e-launcher-slice3.log` |
| control | the same script on `GAME=$CONTROL_GAME` (same Nightly, no package) | `FAIL: the machine item did not register` → `EXIT:1` | `industrialforegoing-e2e/e2e-launcher-control-slice3.log` |
| selftest | `SELFTEST=1` (phase B 200 → 201) | `FAIL: the water is 200, expected 201` → `EXIT:1` | `industrialforegoing-e2e/e2e-launcher-selftest-slice3.log` |
| selftest, augments | `SELFTEST=augments` (phase O's rate +1) | `FAIL: speed 4 fell 56 over 14 ticks, expected 57` → `EXIT:1` | `industrialforegoing-e2e/e2e-launcher-selftest-augments.log` |
| log gate | `python3 ports/tools/check-e2e-logs.py industrialforegoing-e2e --strict` | `logs=10 ok=10 missing-exit=0 bad-exit=0` | — |
| skills gate | `python3 ports/tools/check-skills.py --notes ports/industrialforegoing` | PASS | `verify-sdk-package.py` row |

Artifact: **59,334 B**, SHA-256
`819f00fc54dd6a2623406681494a3842f57331207d60e620b2f597ef66559e7c`, 41 entries, no packed
`Cargo.lock`, `src/runtime.rs` byte-identical to the tree (`83ae3b111494…`).

The 0.2.2 ladder (slice 2, kept as history): tests 26; `check` 34,096 B
`2672a800…4826`; `0b14f8a  fits`; e2e phases A–L `PASS` + `EXIT:0`, control and both selftests
`EXIT:1`. Its logs keep their old names (`sdk-test.log` was overwritten by this ladder's run;
`sdk-test-red.log`, `sdk-fits.log`, `sdk-check.log` still carry 0.2.2's).
