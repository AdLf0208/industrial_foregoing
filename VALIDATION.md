# Validation — `industrialforegoing` 0.2.15

The ladder actually run for the delivered package, cheapest first. Rust compile steps went through
`ports/tools/build-slot.sh` (forwarded to `AdLf-PC`, `CARGO_INCREMENTAL=0`, no `--target-dir`);
`check` and both `fits` runs are the tool's own non-compiling checks, run in the port. The in-game
legs are launcher builds of this port at the campaign pin
**`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`** + mixins `e59da909…`; the launcher's own output reads
`Compiling industrialforegoing (1 of 1)` and the exported manifest's `version = "0.2.15"`. Evidence:
`ports/_shared/evidence/industrialforegoing/` and `ports/_shared/evidence/industrialforegoing-e2e/`.

The slice is **the Plant Fertilizer** (`industrialforegoing:plant_fertilizer`): the natural consumer of
the fertilizer slice 15 makes. `src/runtime.rs` is untouched; the new pure rules are in
`src/plantfert.rs`, the glue in `src/plantnet.rs`.

| step | command | result | evidence |
|---|---|---|---|
| tests, red | `cargo test` with the two new `tick` bodies at `todo!()` | **269 passed, 4 failed** — every new failure `not yet implemented: the pointed-block cooldown and pointer advance` / `… the bonemeal's FE, stack shrink and cooldown` | `industrialforegoing/sdk-test-red-slice16.log` |
| tests | `cargo test` | **279 passed**, 0 failed (`Compiling industrialforegoing v0.2.15`) | `industrialforegoing/sdk-test.log` |
| format | `cargo fmt --check` | clean | `industrialforegoing/sdk-fmt-slice16.log` |
| lints | `cargo clippy --all-targets -- -D warnings` | exit 0, **0 warnings** | `industrialforegoing/sdk-clippy.log` |
| content | `python3 tools/assemble_plantfert.py --check`, `verify_content.py` | 14 plant-fertilizer files identical; content OK | run in this report |
| package | `rm -f Cargo.lock && cargo horizon-mod build` ×2 | `cmp`-identical, no `Cargo.lock` | `industrialforegoing/sdk-build.log`, `sdk-build-2.log` |
| check | `cargo horizon-mod check industrialforegoing.hmod` | `industrialforegoing 0.2.15 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, **439,047 B**, 496 entries, sha256 `da80ff40c706c1ad96127f54fcbfe6d4bc87aafd2c1024e52dcc5c4b37966886`, exit 0 | `industrialforegoing/sdk-check.log` |
| fits (pin) | `cargo horizon-mod fits industrialforegoing.hmod --build 0a875da` | `0a875da  made for`, exit 0 | `industrialforegoing/sdk-fits.log` |
| fits (other) | `… --build 0b14f8a` | `0b14f8a  fits`, exit 0 | `industrialforegoing/sdk-fits.log` |
| runtime | packed `src/runtime.rs` = tree | identical, `83ae3b111494…` | gate row below |
| package gate | `python3 ports/tools/verify-sdk-package.py industrialforegoing` | 1/1 (runtime byte-identical, package matches the tree, R9 clean, no packed lock, 279 tests, 0 warnings, check+`fits`, skills) | run in this report |
| in-game, green | `launcher-game.sh build industrialforegoing` + `tools/e2e.sh` (phases A–DN) | **`PASS` + `EXIT:0`**; DN: no-fertilizer crop stays age 0 with 10,000 FE, one fertilizer grows a cocoa crop 0 → 1 for 1,000 FE, consumes the one input and rests the 12-tick branch | `industrialforegoing-e2e/e2e-launcher.log` |
| control | the same script on `GAME=$CONTROL_GAME` (same Nightly, no package) | `FAIL: the machine item did not register` → `EXIT:1` | `…-control-slice16.log` |
| selftest, plantfert | `SELFTEST=plantfert` (age 1 → asserted 2) | `FAIL: the crop's age in the machine readout is 1, expected 2` → `EXIT:1` | `…-selftest-plantfert.log` |
| log gate | `python3 ports/tools/check-e2e-logs.py industrialforegoing-e2e --strict` | `logs=35 ok=35 missing-exit=0 bad-exit=0` | run in this report |
| skills gate | `python3 ports/tools/check-skills.py --notes industrialforegoing` | PASS | `verify-sdk-package.py` row |

Artifact: **439,047 B**, SHA-256
`da80ff40c706c1ad96127f54fcbfe6d4bc87aafd2c1024e52dcc5c4b37966886`, **496** entries, no packed
`Cargo.lock`, `src/runtime.rs` byte-identical to the tree (`83ae3b111494…`), `LICENSE` line 3
`Copyright (c) 2017 Buuz135`. The in-game reads that matter: an equally powered box with no
fertilizer leaves a cocoa crop at `age=0` and all 10,000 FE; with one fertilizer the machine points
at `[325, 101, 0]`, reads `crop age 1`, ends with 9,000 FE and an empty input, resting at a
cooldown inside the 1..12-tick branch (`floor(50 × 0.25) = 12`).

## Notes on this run

- The crop's age is read from `ctl block <pos>`'s state string: this pin refuses
  `data get block <pos> age` for a block without a block entity (`The target block is not a block
  entity`). The machine's input count still uses `data get block <machine> Items[0].count`.
- The artefact's config *comments* say 100 ticks and 400 FE but its fields are 50 and 1,000; the
  fields are carried, and the discrepancy is recorded as a deviation in `notes.md`.
- Several in-game runs failed in pre-existing water-flow/break/aim legs under a loaded box; their
  logs are kept as `e2e-launcher-flake*-slice16.log`. No assertion was weakened: the composter's FE
  check is now the exact 40 FE/tick delta from its own first readout, so it no longer races the
  setter, and the original water-leg sequence was restored.
- The launcher reused a build only when the packaged manifest was byte-identical; the version moved
  to 0.2.15 and the sources changed, so it recompiled (`Compiling industrialforegoing (1 of 1)`)
  and exported the new tag `native-0a875da-7697335398cd`.
- The earlier ladders (0.2.0 migration, retired `.hnmod`, slices 1–15) are kept in `notes.md` as
  history.
