# Validation — `industrialforegoing` 0.2.17

The ladder actually run for the delivered package, cheapest first. Rust compile steps went through
`ports/tools/build-slot.sh` (forwarded to `AdLf-PC`, `CARGO_INCREMENTAL=0`, no `--target-dir`,
no `BUILD_LOCAL=1`); `check` and both `fits` runs are the tool's own non-compiling checks, run in
the port. The in-game legs are launcher builds of this port at the campaign pin
**`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`** + mixins `e59da909…`; the launcher's own output
reads `Compiling industrialforegoing (1 of 1)` and the exported manifest's `version = "0.2.17"`.
Evidence: `ports/_shared/evidence/industrialforegoing/` and `ports/_shared/evidence/industrialforegoing-e2e/`.

The slice is **the Plant Sower** (`industrialforegoing:plant_sower`): the planting half of the
agriculture family, from gap 9. `src/runtime.rs` is untouched; the new pure rules are in
`src/plantsower.rs`, the glue in `src/plantsowernet.rs`, the carried files in
`tools/assemble_plantsower.py`.

| step | command | result | evidence |
|---|---|---|---|
| tests, red | `cargo test` with the new `tick` body at `todo!()` | **299 passed, 6 failed** — every new failure `not yet implemented: the planting cooldown, the 1000 FE cost and the 20 FE powered miss` | `industrialforegoing/sdk-test-red-slice18.log` |
| glue, red | `cargo test` with `area`/`soil_for` at `todo!()` | **310 passed, 2 failed** — `not yet implemented: the TOP_UP area and its soil` / `not yet implemented: the crop soils` | `industrialforegoing/sdk-test-red-slice18-glue.log` |
| tests | `cargo test` | **312 passed**, 0 failed (`Compiling industrialforegoing v0.2.17`) | `industrialforegoing/sdk-test.log` (regenerated after the final build) |
| format | `cargo fmt --check` (not `--all`) | clean; `src/runtime.rs` hash unchanged (`83ae3b111494…`) | `industrialforegoing/sdk-fmt-slice18.log` |
| lints | `cargo clippy --all-targets -- -D warnings` | exit 0, **0 warnings** | `industrialforegoing/sdk-clippy-slice18.log` |
| content | `python3 tools/assemble_plantsower.py --check`, `content::the_plant_sower_is_carried` | 13 plant-sower files identical; recipe's three tags ship | run in this report |
| package | `rm -f Cargo.lock && cargo horizon-mod build` ×2 | `cmp`-identical, no `Cargo.lock` | `industrialforegoing/sdk-build-slice18.log`, `sdk-build-2-slice18.log` |
| check | `cargo horizon-mod check industrialforegoing.hmod` | `industrialforegoing 0.2.17 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, **466,468 B**, 521 entries, sha256 `b0e540b3adb98a4d3bd89e3b3bec189802505b0e02754916fc61eba2f258d115`, exit 0 | `industrialforegoing/sdk-check-slice18.log` |
| fits (pin) | `cargo horizon-mod fits industrialforegoing.hmod --build 0a875da` | `0a875da  made for`, exit 0 | `industrialforegoing/sdk-fits-slice18.log` |
| fits (other) | `… --build 0b14f8a` | `0b14f8a  fits`, exit 0 | `industrialforegoing/sdk-fits-slice18.log` |
| runtime | packed `src/runtime.rs` = tree | identical, `83ae3b111494…` | gate row below |
| package gate | `python3 ports/tools/verify-sdk-package.py industrialforegoing` | 1/1 (runtime byte-identical, `package matches the tree`, R9 clean, no packed lock, 312 tests, 0 warnings, check + fits, skills) | run in this report |
| in-game, green | `launcher-game.sh build industrialforegoing` + `tools/e2e.sh` (phases A–DP) | **`PASS` + `EXIT:0`**; DP: one `wheat_seeds` plants `minecraft:wheat[age=0]` at `[340, 103, 0]` from farmland at `[340, 102, 0]`, buffer 10,000 → 9,000 FE, input 1 → 0, bar inside 1..8; empty powered box 10,000 → 9,980 FE; empty uncharged box 0 → 0 FE | `industrialforegoing-e2e/e2e-launcher-slice18.log` |
| control | the same script on `GAME=$CONTROL_GAME` (same Nightly, no package) | `FAIL: the machine item did not register` → `EXIT:1` | `…-control-slice18.log` |
| selftest, plantsower | `SELFTEST=plantsower` (planted crop age 0 → asserted 1) | `FAIL: the planted cell state (wheat default state) is minecraft:wheat[age=0], expected minecraft:wheat[age=1]` → `EXIT:1` | `…-selftest-plantsower.log` |
| log gate | `python3 ports/tools/check-e2e-logs.py industrialforegoing-e2e --strict` | `logs=61 ok=61 missing-exit=0 bad-exit=0` | run in this report |
| skills gate | `python3 ports/tools/check-skills.py --notes industrialforegoing` | PASS | `verify-sdk-package.py` row |

Artifact: **466,468 B**, SHA-256
`b0e540b3adb98a4d3bd89e3b3bec189802505b0e02754916fc61eba2f258d115`, **521** entries, no packed
`Cargo.lock`, `src/runtime.rs` byte-identical to the tree (`83ae3b111494…`). The in-game reads that
matter: the machine at `[340, 101, 0]`, facing west, points at `[340, 103, 0]` with farmland at
`[340, 102, 0]`; one seed plants the crop's default state, costs 1,000 FE and rests inside the
`floor(40 × 0.2) = 8` branch. The artefact's powered miss costs exactly 20 FE; only the uncharged
buffer leaves FE untouched (the brief's prose is recorded as discrepancy 83 in `notes.md`).

## Notes on this run

- The artefact's `RangeManager.RangeType.TOP_UP` points two blocks above the machine, not behind it;
  the brief's "behind" wording is superseded. Its `maxProgress` comment says 100 but the field is 40.
- The 8-tick plant cooldown is shorter than the `ctl block` / `data get` read chain: the first run
  read 8,980 FE because a second (powered-miss) attempt ran before the buffer read. The leg now
  reads the exact FE first after a two-tick wait; the failed log is kept as
  `e2e-launcher-slice18-cooldown-race.log` and no assertion was weakened.
- Eight pre-DP flakes (a stale gatherer cooldown, a chamber that did not place, the water-flow count,
  an aim failure, an unanswered chat path and an aim timeout) are kept as named logs; the green run
  and the `SELFTEST=plantsower` failure both reached phase DP.
- The forwarded slot runner pulls the remote `ports/_shared/evidence/` back at the end of a lease;
  `sdk-test.log` was therefore regenerated through a path outside `ports/` and copied in after the
  last forwarded run, so the log shows `Compiling industrialforegoing v0.2.17` and 312 tests.
- The earlier ladders (0.2.0 migration, retired `.hnmod`, slices 1–17) are kept in `notes.md` as
  history.
