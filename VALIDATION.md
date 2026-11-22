# Validation — `industrialforegoing` 0.2.7

The ladder actually run for the delivered package, cheapest first. Every compile went through
`ports/tools/build-slot.sh` (`BUILD_LOCAL=1`, `CARGO_INCREMENTAL=0`, no `--target-dir`); `check` and
both `fits` runs are the tool's own non-compiling checks, run in the port. The in-game legs are
launcher builds of this port at the campaign pin
**`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`** + mixins `e59da909…`, with the log's own
`Compiling industrialforegoing (1 of 1)` (`evidence/industrialforegoing-e2e/launcher-build-slice7.log`).
Evidence: `ports/_shared/evidence/industrialforegoing/` and
`ports/_shared/evidence/industrialforegoing-e2e/`.

| step | command | result | evidence |
|---|---|---|---|
| red (rules) | `cargo test` with slice 7's rules as tests and their bodies at `todo!()` | **106 passed, 14 failed** (`not yet implemented`, e.g. `the_drill_bar_falls_one_a_tick_and_charges_the_base_at_zero`) | `industrialforegoing/sdk-test-red-slice7.log` |
| red (glue) | `cargo test` with the declarations/shapes at `todo!()` | **122 passed, 9 failed** (the eight glue tests and the engine registration test) | `industrialforegoing/sdk-test-red-slice7-glue.log` |
| tests | `cargo test` | **134 passed**, 0 failed (120 carried + 14 laser rules + 9 glue − 9; the count after the final glue and content tests) | `industrialforegoing/sdk-test.log` |
| format | `cargo fmt --check` | clean (`FMT-CLEAN`) | `industrialforegoing/sdk-clippy.log` |
| lints | `cargo clippy --all-targets -- -D warnings` | exit 0, 0 warnings | `industrialforegoing/sdk-clippy.log` |
| content | `python3 tools/verify_content.py`, `--selftest`, and the generator's `--check` | `CONTENT OK` (341 files); the selftest catches 10 mutations, incl. the laser's raw-iron tag, an ore recipe, and an ore advancement's condition | `industrialforegoing/sdk-content-slice7.log` |
| package | `rm -f Cargo.lock && cargo horizon-mod build` ×2 | `cmp`-identical (sha256 `68b63343…`), **355** entries, no packed `Cargo.lock` | `industrialforegoing/sdk-build-slice7.log` |
| check | `cargo horizon-mod check industrialforegoing.hmod` | `industrialforegoing 0.2.7 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, **265,740 B**, sha256 `68b63343357f9f9847bb5bf9be3cd44348aa0f90c98e449afbfc0af8ce5ec614`, exit 0 | `industrialforegoing/sdk-check.log` |
| fits (pin) | `cargo horizon-mod fits industrialforegoing.hmod --build 0a875da` | `0a875da  made for`, exit 0 | `industrialforegoing/sdk-fits.log` |
| fits (other) | `… --build 0b14f8a` | `0b14f8a  fits`, exit 0 | `industrialforegoing/sdk-fits.log` |
| package gate | `python3 ports/tools/verify-sdk-package.py industrialforegoing` | **1/1** (runtime byte-identical `83ae3b11…`, check+`fits`, 134 tests, 0 warnings, skills) | — |
| in-game, green | `launcher-game.sh build` + `tools/e2e.sh` (phases A–BH) | **`PASS` + `EXIT:0`** — the laser phases BA–BH are quoted in `notes.md` | `industrialforegoing-e2e/e2e-launcher.log` |
| control | the same script on `GAME=$CONTROL_GAME` (same Nightly, no package) | `FAIL: the machine item did not register` → `EXIT:1` | `…-control.log` |
| selftest | `SELFTEST=1` (phase B 200 → 201) | `FAIL: the water is 200, expected 201` → `EXIT:1` | `…-selftest.log` |
| selftest, laser | `SELFTEST=laser` (phase BD's 1,000 FE charge 9,000 → 9,001) | `FAIL: the drill's buffer after one charge (10,000 - 1,000 FE) is 9001, expected 9000` → `EXIT:1` | `…-selftest-laser.log` |
| log gate | `python3 ports/tools/check-e2e-logs.py industrialforegoing-e2e --strict` | `logs=8 ok=8 missing-exit=0 bad-exit=0` | — |
| skills gate | `python3 ports/tools/check-skills.py --notes industrialforegoing` | PASS | `verify-sdk-package.py` row |

Artifact: **265,740 B**, SHA-256
`68b63343357f9f9847bb5bf9be3cd44348aa0f90c98e449afbfc0af8ce5ec614`, 355 entries, no packed
`Cargo.lock`, `src/runtime.rs` byte-identical to the tree (`83ae3b111494…`). The add-on count: the
`industrialforegoingsouls` jar's **1/1** `laser_drill_ore` recipe loads under this type (the base's
table reads 11/11 with it) and runs (`echo_shard x1`).

The 0.2.6 ladder (slice 6, kept as history): tests 105; `check` 225,305 B `d9dbb231…`; e2e phases
A–AM `PASS` + `EXIT:0`, control and five selftests `EXIT:1`. The 0.2.5 ladder (slice 5): tests 75;
`check` 189,268 B `0876ea23…bad`. The 0.2.4 ladder (slice 4): tests 46; `check` 153,549 B
`dd43090d…08f`. The 0.2.3 ladder (slice 3): tests 39; `check` 59,334 B `819f00fc…59e7c`. The 0.2.2
ladder (slice 2): tests 26; `check` 34,096 B `2672a800…4826`.
