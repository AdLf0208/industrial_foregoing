# Validation — `industrialforegoing` 0.2.12

The ladder actually run for the delivered package, cheapest first. Every compile went through
`ports/tools/build-slot.sh` (`BUILD_LOCAL=1`, `CARGO_INCREMENTAL=0`, no `--target-dir`); `check` and
both `fits` runs are the tool's own non-compiling checks, run in the port. The in-game legs are
launcher builds of this port at the campaign pin
**`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`** + mixins `e59da909…`; the launcher's own output reads
`Compiling industrialforegoing (1 of 1)` and the exported manifest's `version = "0.2.12"`
(`evidence/industrialforegoing-e2e/launcher-build-slice13.log`, `native-mods.json`). Evidence:
`ports/_shared/evidence/industrialforegoing/` and `ports/_shared/evidence/industrialforegoing-e2e/`.

The slice is **the Animal Rancher** (slice 13): the machine's single-block working area, 100-tick bar,
400 FE an operation, 8,000 mB milk tank and 15-slot output container — the sheep's wool, the cow's and
goat's milk, the squid's dye — and the in-game legs CZ–DG.

| step | command | result | evidence |
|---|---|---|---|
| tests, red | `cargo test` with the new rules at `todo!()` | **206 passed, 15 failed**, every failure `not yet implemented` | `industrialforegoing/sdk-test-red-slice13.log` |
| tests | `cargo test` | **232 passed**, 0 failed (`Compiling industrialforegoing v0.2.12`) | `industrialforegoing/sdk-test.log` |
| format | `cargo fmt --check` | clean | `industrialforegoing/sdk-clippy.log` (tail) |
| lints | `cargo clippy --all-targets -- -D warnings` | exit 0, 0 warnings | `industrialforegoing/sdk-clippy.log` |
| content | `python3 tools/assemble_rancher.py --check`; `python3 tools/verify_content.py` (`--package` too); `--selftest` | `14 rancher files: identical`; `CONTENT OK: 433 content files` for the tree **and** the delivered `.hmod`; `selftest: OK` (both rancher mutations caught) | `industrialforegoing/sdk-content-slice13.log` |
| package | `rm -f Cargo.lock && cargo horizon-mod build` ×2 | `cmp`-identical, sha256 `28eef176…` both times, **457** entries, no packed `Cargo.lock` | `industrialforegoing/sdk-check.log` row |
| check | `cargo horizon-mod check industrialforegoing.hmod` | `industrialforegoing 0.2.12 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, **390,790 B**, sha256 `28eef176332c277538d8fc5d23bf60aac8eb226407b6a5895cfa47cbd9618826`, exit 0 | `industrialforegoing/sdk-check.log` |
| fits (pin) | `cargo horizon-mod fits industrialforegoing.hmod --build 0a875da` | `0a875da  made for`, exit 0 | `industrialforegoing/sdk-fits.log` |
| fits (other) | `… --build 0b14f8a` | `0b14f8a  fits`, exit 0 | `industrialforegoing/sdk-fits.log` |
| recipe types | `python3 ports/tools/check-recipe-types.py industrialforegoing` | `industrialforegoing: ok` | `industrialforegoing/sdk-recipe-types.log` |
| runtime | `cmp <(unzip -p … src/runtime.rs) src/runtime.rs` | identical, `83ae3b11…` | gate row below |
| package gate | `python3 ports/tools/verify-sdk-package.py industrialforegoing` | **1/1** (runtime byte-identical, every packed file the tree's bytes, no packed lock, 232 tests, 0 warnings, check+`fits`, skills) | `industrialforegoing/sdk-gate-0.2.12.log` |
| in-game, green | `launcher-game.sh build industrialforegoing --port …` + `tools/e2e.sh` (phases A–DG) | **`PASS` + `EXIT:0`** — the new CZ–DG block is quoted in `notes.md`; all 20 earlier legs re-ran green | `industrialforegoing-e2e/e2e-launcher-slice13.log` |
| control | the same script on `GAME=$CONTROL_GAME` (same Nightly, no package) | `FAIL: the machine item did not register` → `EXIT:1` | `…-control-slice13.log` |
| selftest, base | `SELFTEST=1` (phase B 200 → 201) | `FAIL: the water is 200, expected 201` → `EXIT:1` | `…-selftest-slice13.log` |
| selftest, **rancher** | `SELFTEST=rancher` (phase DA 1,000 mB → 1,001) | `FAIL: the milk one milking gives is 1000, expected 1001` → `EXIT:1` | `…-selftest-rancher.log` |
| earlier selftests | contents · core · chamber · laser · latex · fluid · nether · slaughter · crusher · duplicator | their logs are unchanged (they predate this package) and the green run re-ran every phase they flip | `…-launcher-selftest-*.log` |
| log gate | `python3 ports/tools/check-e2e-logs.py industrialforegoing-e2e --strict` | `logs=24 ok=24 missing-exit=0 bad-exit=0` | `industrialforegoing-e2e/check-e2e-logs-slice13.log` |
| skills gate | `python3 ports/tools/check-skills.py --notes industrialforegoing` | PASS | `verify-sdk-package.py` row |

Artifact: **390,790 B**, SHA-256
`28eef176332c277538d8fc5d23bf60aac8eb226407b6a5895cfa47cbd9618826`, **457** entries, no packed
`Cargo.lock`, `src/runtime.rs` byte-identical to the tree (`83ae3b111494…`). The in-game reads that
matter: `Animal Rancher at [300, 101, 0]: 4600/10000 FE, tank industrialforegoing:milk 1000/8000 mB,
capability industrialforegoing:milk 1000 mB, animals 1 (sheep 0, milkable 1, squid 0), output empty,
progress 30/100, facing west` — one cow for exactly 400 FE and 1,000 mB; and the counterfactuals — a
sheep sheared to `Sheared: 1b` and `1x minecraft:red_wool`, where the same box again gives nothing; an
empty box spending nothing; 399 FE leaving the cow and the tank untouched; a full 15-slot container
and four failing draws each refusing the squid's dye; and the broken machine's item carrying
`energy: 4600` / `Amount: 1000` while its two wool dropped beside it.

The 0.2.11 ladder (slice 12, kept as history): tests 206; `check` 366,772 B `f7154dc5…`; e2e phases
A–CY `PASS` + `EXIT:0`, control and selftests `EXIT:1`, 20/20 logs. The 0.2.10 ladder (slice 11,
kept as history): tests 186; `check` 339,359 B `c759203e…`; e2e phases A–CR `PASS` + `EXIT:0`, 16/16
logs. The 0.2.9 ladder (slice 9): tests 166; `check` 318,727 B `fa7300d3…`; 10/10 logs. The 0.2.8
ladder (slice 8): tests 148; `check` 286,421 B `f34f98ee…`; 9/9 logs. The 0.2.7 ladder (slice 7):
tests 134; `check` 265,740 B `68b63343…`; 8/8 logs. The 0.2.6 ladder (slice 6): tests 105; `check`
225,305 B `d9dbb231…`. The 0.2.5 ladder (slice 5): tests 75; `check` 189,268 B `0876ea23…bad`. The
0.2.4 ladder (slice 4): tests 46; `check` 153,549 B `dd43090d…08f`. The 0.2.3 ladder (slice 3): tests
39; `check` 59,334 B `819f00fc…59e7c`. The 0.2.2 ladder (slice 2): tests 26; `check` 34,096 B
`2672a800…4826`.

## Notes on this run

- The launcher reuses a build whose `horizon-mod.toml` is **byte-identical**, so every
  `native-*` tag carrying this mod was removed before the rebuild; the new tag is
  `native-0a875da-dda36f024567` and its manifest reads `version = "0.2.12"` (the src changed while the
  manifest did not — the tag-removal is what makes the rebuild honest).
- One e2e run failed **phase C** (`the sources is 2, expected 1`) before any of this slice's phases —
  the condensator counting a second water source from the previous leg (the flow interference slice
  2's notes record). Three later runs, including the delivered green one, were clean; no code this
  slice changed was in that path.
- Three build pairs were watched before the delivered one: `29a89f44…` (before the in-game fix that
  spends the harness's pinned draw on the attempt tick, not the next tick), `1e33eb39…` (before the
  tick stopped rewriting a tank it did not fill) and `28eef176…` — the delivered artifact, whose
  green, control and selftest runs are all the ones quoted above. The launcher tag that carries it is
  `native-0a875da-cf2a427bb411`; every `native-*` tag carrying this mod is removed before each
  rebuild, because the launcher reuses a build whose `horizon-mod.toml` is byte-identical while the
  source may have changed.
