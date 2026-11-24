# Validation — `industrialforegoing` 0.2.16

The ladder actually run for the delivered package, cheapest first. Rust compile steps went through
`ports/tools/build-slot.sh` (forwarded to `AdLf-PC`, `CARGO_INCREMENTAL=0`, no `--target-dir`,
no `BUILD_LOCAL=1`); `check` and both `fits` runs are the tool's own non-compiling checks, run in
the port. The in-game legs are launcher builds of this port at the campaign pin
**`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`** + mixins `e59da909…`; the launcher's own output
reads `Compiling industrialforegoing (1 of 1)` and the exported manifest's `version = "0.2.16"`.
Evidence: `ports/_shared/evidence/industrialforegoing/` and `ports/_shared/evidence/industrialforegoing-e2e/`.

The slice is **the Plant Gatherer** (`industrialforegoing:plant_gatherer`): the harvest half of the
agriculture family, from gap 9. `src/runtime.rs` is untouched; the new pure rules are in
`src/plantgather.rs`, the glue in `src/plantgathernet.rs`, the carried files in
`tools/assemble_plantgather.py`.

| step | command | result | evidence |
|---|---|---|---|
| tests, red | `cargo test` with the new `tick` body at `todo!()` | **283 passed, 6 failed** — every new failure `not yet implemented: the harvest cooldown, the 400 FE cost and the output stacks` | `industrialforegoing/sdk-test-red-slice17.log` |
| tests | `cargo test` | **295 passed**, 0 failed (`Compiling industrialforegoing v0.2.16`) | `industrialforegoing/sdk-test.log` (regenerated after the final build) |
| format | `cargo fmt --check` (not `--all`) | clean; `src/runtime.rs` hash unchanged (`83ae3b111494…`) | `industrialforegoing/sdk-fmt-slice17.log` |
| lints | `cargo clippy --all-targets -- -D warnings` | exit 0, **0 warnings** | `industrialforegoing/sdk-clippy-slice17.log` |
| content | `python3 tools/assemble_plantgather.py --check`, `content::the_plant_gatherer_is_carried` | 14 plant-gatherer files identical; recipe's three tags ship | run in this report |
| package | `rm -f Cargo.lock && cargo horizon-mod build` ×2 | `cmp`-identical, no `Cargo.lock` | `industrialforegoing/sdk-build-slice17.log`, `sdk-build-2-slice17.log` |
| check | `cargo horizon-mod check industrialforegoing.hmod` | `industrialforegoing 0.2.16 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, **453,037 B**, 509 entries, sha256 `9b921a62ae8f50830a37ad4761bda17724b8359f7acda70f6915745587bfac6b`, exit 0 | `industrialforegoing/sdk-check-slice17.log` |
| fits (pin) | `cargo horizon-mod fits industrialforegoing.hmod --build 0a875da` | `0a875da  made for`, exit 0 | `industrialforegoing/sdk-fits-slice17.log` |
| fits (other) | `… --build 0b14f8a` | `0b14f8a  fits`, exit 0 | `industrialforegoing/sdk-fits-slice17.log` |
| runtime | packed `src/runtime.rs` = tree | identical, `83ae3b111494…` | gate row below |
| package gate | `python3 ports/tools/verify-sdk-package.py industrialforegoing` | 1/1 (runtime byte-identical, `package matches the tree`, R9 clean, no packed lock, 295 tests, 0 warnings, check + fits, skills) | run in this report |
| in-game, green | `launcher-game.sh build industrialforegoing` + `tools/e2e.sh` (phases A–DO) | **`PASS` + `EXIT:0`**; DO: mature wheat behind the machine → `Items` holds `minecraft:wheat` ×1 and `wheat_seeds` ×1..3, the cell reads `minecraft:air`, 10,000 → 9,600 FE, bar inside 1..12; the immature box stays at 10,000 FE with an empty output | `industrialforegoing-e2e/e2e-launcher.log` |
| control | the same script on `GAME=$CONTROL_GAME` (same Nightly, no package) | `FAIL: the machine item did not register` → `EXIT:1` | `…-control-slice17.log` |
| selftest, plantgather | `SELFTEST=plantgather` (wheat count 1 → asserted 2) | `FAIL: the exact harvested wheat count is 1, expected 2` → `EXIT:1` | `…-selftest-plantgather.log` |
| log gate | `python3 ports/tools/check-e2e-logs.py industrialforegoing-e2e --strict` | `logs=41 ok=41 missing-exit=0 bad-exit=0` | run in this report |
| skills gate | `python3 ports/tools/check-skills.py --notes industrialforegoing` | PASS | `verify-sdk-package.py` row |

Artifact: **453,037 B**, SHA-256
`9b921a62ae8f50830a37ad4761bda17724b8359f7acda70f6915745587bfac6b`, **509** entries, no packed
`Cargo.lock`, `src/runtime.rs` byte-identical to the tree (`83ae3b111494…`), `LICENSE` line 3
`Copyright (c) 2017 Buuz135`. The in-game reads that matter: the machine at `[330, 101, 0]`, facing
west, points at `[331, 101, 0]`; after one forced bar the container holds the crop's own drops, the
cell is air with no age, the buffer is 9,600 FE and the bar is inside the `floor(40 × 0.3) = 12`
branch. The equally powered box behind an immature crop keeps all 10,000 FE and an empty output.

## Notes on this run

- The artefact's `PlantGathererConfig.maxProgress` comment says 100 but its field is 40; the field
  is carried. The sludge/ether tanks and the ether-bar replant arm are recorded gaps, not faked.
- `ctl command "data get block <crop> age"` is refused for a block without a block entity; the
  post-harvest state is read from `ctl block` instead (`minecraft:air`, no `age`). The machine's
  container is read with `data get block <machine> Items` and parsed by exact id/count.
- Wheat's seed count is a binomial loot roll (the table declares `extra 3`, `p 0.5714286`), so the
  leg asserts the exact wheat id/count and the seeds' declared 1..3 range. The selftest flips the
  exact wheat count, so the assertion still fails on demand.
- The first green attempt failed at that random seed count; its log and three further runs that
  failed in *pre-existing* flaky legs before DO (aim, water flow, a stale `fill` readout) are kept
  as `e2e-launcher-flake{1..4}-slice17.log`. No assertion was weakened.
- The earlier ladders (0.2.0 migration, retired `.hnmod`, slices 1–16) are kept in `notes.md` as
  history.
