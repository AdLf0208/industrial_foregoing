# Validation — `industrialforegoing` 0.2.14

The ladder actually run for the delivered package, cheapest first. Rust compile steps went through
`ports/tools/build-slot.sh` (forwarded to `AdLf-PC`, `CARGO_INCREMENTAL=0`, no `--target-dir`,
no `BUILD_LOCAL=1`); `check` and both `fits` runs are the tool's own non-compiling checks, run in
the port. The in-game legs are launcher builds of this port at the campaign pin
**`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`** + mixins `e59da909…`; the launcher's own output reads
`Compiling industrialforegoing (1 of 1)` and the exported manifest's `version = "0.2.14"`. Evidence:
`ports/_shared/evidence/industrialforegoing/` and `ports/_shared/evidence/industrialforegoing-e2e/`.

The slice is **the sewage chain** — the Sewer and the Sewage Composter, their two recipes, the
fertilizer item and its tag, plus the `industrialforegoing:sewage` fluid declaration. `src/runtime.rs`
is untouched; the new pure rules are in `src/sewage.rs`.

| step | command | result | evidence |
|---|---|---|---|
| tests, red | `cargo test` with the two new `tick` bodies at `todo!()` | **244 passed, 9 failed** — every new failure `not yet implemented: the sewer's bar and per-animal collection` / `… the composter's rising bar and one fertilizer per 1000 mB` | `industrialforegoing/sdk-test-red-slice15.log` |
| tests | `cargo test` | **262 passed**, 0 failed (`Compiling industrialforegoing v0.2.14`) | `industrialforegoing/sdk-test.log` |
| format | `cargo fmt --check` | clean | `FMT-CLEAN` in the lease |
| lints | `cargo clippy --all-targets -- -D warnings` | exit 0, **0 warnings** | `industrialforegoing/sdk-clippy.log` |
| content | `python3 tools/verify_content.py`, assembler `--check` | `CONTENT OK: … 457 content files`; 27 sewage-chain files identical | run in this report |
| package | `rm -f Cargo.lock && cargo horizon-mod build` ×2 | `cmp`-identical, no `Cargo.lock` | `industrialforegoing/sdk-build.log` |
| check | `cargo horizon-mod check industrialforegoing.hmod` | `industrialforegoing 0.2.14 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, **422,257 B**, 483 entries, sha256 `10e7f0ee53b09d9e3fdb4fb17f50009034d39d38ea18e06592e099107c16faa9`, exit 0 | `industrialforegoing/sdk-check.log` |
| fits (pin) | `cargo horizon-mod fits industrialforegoing.hmod --build 0a875da` | `0a875da  made for`, exit 0 | `industrialforegoing/sdk-fits.log` |
| fits (other) | `… --build 0b14f8a` | `0b14f8a  fits`, exit 0 | `industrialforegoing/sdk-fits.log` |
| runtime | packed `src/runtime.rs` = tree | identical, `83ae3b111494…` | gate row below |
| package gate | `python3 ports/tools/verify-sdk-package.py industrialforegoing` | **1/1** (runtime byte-identical, **package matches the tree**, R9 clean, no packed lock, 262 tests, 0 warnings, check+`fits`, skills) | run in this report |
| in-game, green | `launcher-game.sh build industrialforegoing` + `tools/e2e.sh` (phases A–DM) | **`PASS` + `EXIT:0`**; DL: 50 mB + capability 50 mB for 20 FE, empty pen 0/0; DM: 1,000 mB → 1 fertilizer in 138 ticks for 4,000 FE | `industrialforegoing-e2e/e2e-launcher.log` |
| control | the same script on `GAME=$CONTROL_GAME` (same Nightly, no package) | `FAIL: the machine item did not register` → `EXIT:1` | `…-control-slice15.log` |
| selftest, sewage | `SELFTEST=sewage` (DL's 50 mB → 51) | `FAIL: the sewage one cow gives is 50, expected 51` → `EXIT:1` | `…-selftest-sewage.log` |
| log gate | `python3 ports/tools/check-e2e-logs.py industrialforegoing-e2e --strict` | `logs=26 ok=26 missing-exit=0 bad-exit=0` (the 24 earlier legs plus two new) | run in this report |
| skills gate | `python3 ports/tools/check-skills.py --notes industrialforegoing` | PASS | `verify-sdk-package.py` row |

Artifact: **422,257 B**, SHA-256
`10e7f0ee53b09d9e3fdb4fb17f50009034d39d38ea18e06592e099107c16faa9`, **483** entries, no packed
`Cargo.lock`, `src/runtime.rs` byte-identical to the tree (`83ae3b111494…`), `LICENSE` line 3
`Copyright (c) 2017 Buuz135`. The in-game reads that matter: `tank industrialforegoing:sewage
50/8000 mB` and `capability industrialforegoing:sewage 50 mB` after one `NoAI` cow and one forced
bar (10,000 → 9,980 FE), 0 mB and 0 FE with an empty pen, and `output 1x
industrialforegoing:fertilizer` with the tank at 0 and the buffer at 6,000 FE after 138 ticks.

## Notes on this run

- The launcher reuses a build only when the packaged manifest is byte-identical; the version moved
  to 0.2.14 and the sources changed, so it recompiled (`Compiling industrialforegoing (1 of 1)`)
  and exported the new tag `native-0a875da-82fd3429c12d`.
- Two defects were found by the first in-game run and fixed before the green one: a double `fill`
  in the sewer glue (100 mB instead of 50) and the missing `industrialforegoing:sewage` entry in
  `dissolution::DECLARED_FLUIDS` (the capability read `empty`). Both are unit-pinned now.
- The new phases DL/DM use their own arena at x=320/322 in the End's void, clear of the rancher's
  at x=300 and every earlier arena; `E2E_TMP` (not `/tmp`) holds the scratch world.
- The earlier ladders (0.2.0 migration, retired `.hnmod`) are kept in `notes.md` as history.
