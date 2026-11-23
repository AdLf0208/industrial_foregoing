# Validation — `industrialforegoing` 0.2.11

The ladder actually run for the delivered package, cheapest first. Every compile went through
`ports/tools/build-slot.sh` (`BUILD_LOCAL=1`, `CARGO_INCREMENTAL=0`, no `--target-dir`); `check` and
both `fits` runs are the tool's own non-compiling checks, run in the port. The in-game legs are
launcher builds of this port at the campaign pin
**`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`** + mixins `e59da909…`; the launcher's own output reads
`Compiling industrialforegoing (1 of 1)` and the exported manifest's `version = "0.2.11"`
(`evidence/industrialforegoing/launcher-build-slice12.log`). Evidence:
`ports/_shared/evidence/industrialforegoing/` and `ports/_shared/evidence/industrialforegoing-e2e/`.

The slice is **the Mob Duplicator and the Mob Imprisonment Tool** (slice 12): the machine's 50,000 FE
buffer, 8,000 mB fill-only essence tank, one-slot tool input and spawn tick, the tool's capture and
release, and the in-game legs CS–CY.

| step | command | result | evidence |
|---|---|---|---|
| tests, red | `cargo test` with the new rules/declarations at `todo!()` | **192 passed, 14 failed**, every failure `not yet implemented` | `industrialforegoing/sdk-test-red-slice12.log` |
| tests | `cargo test` | **206 passed**, 0 failed (`Compiling industrialforegoing v0.2.11`) | `industrialforegoing/sdk-test.log` |
| format | `cargo fmt --check` | clean | `industrialforegoing/sdk-fmt.log` |
| lints | `cargo clippy --all-targets -- -D warnings` | exit 0, 0 warnings | `industrialforegoing/sdk-clippy.log` |
| content | `python3 tools/assemble_duplicator.py --check`; `python3 tools/verify_content.py` | `24 duplicator files: identical`; `CONTENT OK: 422 content files` | `industrialforegoing/sdk-content-slice12.log` |
| package | `rm -f Cargo.lock && cargo horizon-mod build` ×2 | `cmp`-identical, sha256 `f7154dc5…` both times, **444** entries, no packed `Cargo.lock` | `industrialforegoing/sdk-build-slice12.log`, `…-2.log`, `sdk-sha256-slice12.txt` |
| check | `cargo horizon-mod check industrialforegoing.hmod` | `industrialforegoing 0.2.11 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, **366,772 B**, sha256 `f7154dc52f6c9a343a11d5b757b1c674bb924c92ba199f2d73a1ccc91fa962ad`, exit 0 | `industrialforegoing/sdk-check.log` |
| fits (pin) | `cargo horizon-mod fits industrialforegoing.hmod --build 0a875da` | `0a875da  made for`, exit 0 | `industrialforegoing/sdk-fits.log` |
| fits (other) | `… --build 0b14f8a` | `0b14f8a  fits`, exit 0 | `industrialforegoing/sdk-fits.log` |
| recipe types | `python3 ports/tools/check-recipe-types.py industrialforegoing` | `industrialforegoing: ok` | `industrialforegoing/sdk-recipe-types.log` |
| runtime | `cmp <(unzip -p … src/runtime.rs) src/runtime.rs` | identical, `83ae3b11…` | gate row below |
| package gate | `python3 ports/tools/verify-sdk-package.py industrialforegoing` | **1/1** (runtime byte-identical, every packed file the tree's bytes, no packed lock, 206 tests, 0 warnings, check+`fits`, skills) | `industrialforegoing/sdk-gate-0.2.11.log` |
| in-game, green | `launcher-game.sh build industrialforegoing --port …` + `tools/e2e.sh` (phases A–CY) | **`PASS` + `EXIT:0`** — the new CS–CY block is quoted in `notes.md` | `industrialforegoing-e2e/e2e-launcher-slice12.log` |
| control | the same script on `GAME=$CONTROL_GAME` (same Nightly, no package) | `Unknown item 'industrialforegoing:water_condensator'` → `EXIT:1` | `…-control-slice12.log` |
| selftest, base | `SELFTEST=1` (phase B 200 → 201) | `FAIL: the water is 200, expected 201` → `EXIT:1` | `…-selftest-slice12.log` |
| selftest, **duplicator** | `SELFTEST=duplicator` (phase CU one cow → two) | `FAIL: the cows the operation spawned is 1, expected 2` → `EXIT:1` | `…-selftest-duplicator.log` |
| earlier selftests | contents · core · chamber · laser · latex · fluid · nether · slaughter · crusher | their logs are unchanged and the green run re-ran every phase | `…-launcher-selftest-*.log`, `…-selftest-*.log` |
| log gate | `python3 ports/tools/check-e2e-logs.py industrialforegoing-e2e --strict` | `logs=20 ok=20 missing-exit=0 bad-exit=0` | `industrialforegoing-e2e/check-e2e-logs-slice12.log` |
| skills gate | `python3 ports/tools/check-skills.py --notes industrialforegoing` | PASS | `verify-sdk-package.py` row |

Artifact: **366,772 B**, SHA-256
`f7154dc52f6c9a343a11d5b757b1c674bb924c92ba199f2d73a1ccc91fa962ad`, **444** entries, no packed
`Cargo.lock`, `src/runtime.rs` byte-identical to the tree (`83ae3b111494…`). The in-game reads that
matter: `Mob Duplicator at [280, 101, 0]: 45000/50000 FE, tank empty 0/8000 mB, capability empty 0
mB, tool minecraft:cow, needs 120 mB, nearby 1` — one cow for exactly 5,000 FE and 120 mB; and the
counterfactuals — the same machine with 0 essence spawns nothing and spends nothing, and 33 cows in
the working column refuse the attempt while holding `5000 FE` and `120 mB`. The tool captured a cow,
the machine read `minecraft:cow`/120 mB, the tool released a cow on a block face and emptied itself,
and the broken machine's item carried `energy: 4999` / `Amount: 120` while its tool dropped beside
it.

The 0.2.10 ladder (slice 11, kept as history): tests 186; `check` 339,359 B
`c759203e…`; e2e phases A–CR `PASS` + `EXIT:0`, control and selftests `EXIT:1`, 16/16 logs. The
0.2.9 ladder (slice 9): tests 166; `check` 318,727 B `fa7300d3…`; 10/10 logs. The 0.2.8 ladder
(slice 8): tests 148; `check` 286,421 B `f34f98ee…`; 9/9 logs. The 0.2.7 ladder (slice 7): tests
134; `check` 265,740 B `68b63343…`; 8/8 logs. The 0.2.6 ladder (slice 6): tests 105; `check`
225,305 B `d9dbb231…`. The 0.2.5 ladder (slice 5): tests 75; `check` 189,268 B `0876ea23…bad`.
The 0.2.4 ladder (slice 4): tests 46; `check` 153,549 B `dd43090d…08f`. The 0.2.3 ladder (slice 3):
tests 39; `check` 59,334 B `819f00fc…59e7c`. The 0.2.2 ladder (slice 2): tests 26; `check` 34,096 B
`2672a800…4826`.
