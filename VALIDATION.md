# Validation — `industrialforegoing` 0.2.10

The ladder actually run for the delivered package, cheapest first. Every compile went through
`ports/tools/build-slot.sh` (`BUILD_LOCAL=1`, `CARGO_INCREMENTAL=0`, no `--target-dir`); `check` and
both `fits` runs are the tool's own non-compiling checks, run in the port. The in-game legs are
launcher builds of this port at the campaign pin
**`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`** + mixins `e59da909…`; the launcher's own output reads
`Compiling industrialforegoing (1 of 1)` and the exported manifest's `version = "0.2.10"`
(`evidence/industrialforegoing/launcher-build-slice11.log`). Evidence:
`ports/_shared/evidence/industrialforegoing/` and `ports/_shared/evidence/industrialforegoing-e2e/`.

The slice is **the mob crusher and the essence fluid** (slice 11): the block, its block entity, its
10,000 FE buffer, its 32,000 mB drain-only essence tank, its 18-slot output container, the two
death hooks that take the kill's loot and its experience, and the in-game legs CM–CR.

| step | command | result | evidence |
|---|---|---|---|
| tests, red | `cargo test` with the new rules/declarations at `todo!()` | **173 passed, 13 failed**, every failure `not yet implemented` | `industrialforegoing/sdk-test-red-slice11.log` |
| tests | `cargo test` | **186 passed**, 0 failed (`Compiling industrialforegoing v0.2.10`) | `industrialforegoing/sdk-test.log` |
| format | `cargo fmt --check` | clean | `industrialforegoing/sdk-fmt.log` |
| lints | `cargo clippy --all-targets -- -D warnings` | exit 0, 0 warnings | `industrialforegoing/sdk-clippy.log` |
| package | `rm -f Cargo.lock && cargo horizon-mod build` ×2 | `cmp`-identical, sha256 `c759203e…` both times, **421** entries, no packed `Cargo.lock` | `industrialforegoing/sdk-build-slice11.log`, `…-2.log`, `sdk-sha256-0.2.10.txt` |
| check | `cargo horizon-mod check industrialforegoing.hmod` | `industrialforegoing 0.2.10 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, **339,359 B**, sha256 `c759203e51229549e63fb4b113c76950e661576d3053472fa5e69dc5e627f4d4`, exit 0 | `industrialforegoing/sdk-check.log` |
| fits (pin) | `cargo horizon-mod fits industrialforegoing.hmod --build 0a875da` | `0a875da  made for`, exit 0 | `industrialforegoing/sdk-fits.log` |
| fits (other) | `… --build 0b14f8a` | `0b14f8a  fits`, exit 0 | `industrialforegoing/sdk-fits.log` |
| runtime | `cmp <(unzip -p … src/runtime.rs) src/runtime.rs` | identical, `83ae3b11…` | gate row below |
| package gate | `python3 ports/tools/verify-sdk-package.py industrialforegoing` | **1/1** (runtime byte-identical, every packed file the tree's bytes, no packed lock, 186 tests, 0 warnings, check+`fits`, skills) | `industrialforegoing/sdk-gate-0.2.10.log` |
| in-game, green | `launcher-game.sh build industrialforegoing --port …` + `tools/e2e.sh` (phases A–CR) | **`PASS` + `EXIT:0`** — the new CM–CR block is quoted in `notes.md` | `industrialforegoing-e2e/e2e-launcher-slice11.log` |
| control | the same script on `GAME=$CONTROL_GAME` (same Nightly, no package) | `FAIL: the machine item did not register` → `EXIT:1` | `…-control-slice11.log` |
| selftest, **crusher** | `SELFTEST=crusher` (phase CO 20 mB → 21) | `FAIL: the essence a cow gives (20 x 1 points) is 20, expected 21` → `EXIT:1` | `…-selftest-crusher.log` |
| selftest, base | `SELFTEST=1` (phase B 200 → 201) | `FAIL: the water is 200, expected 201` → `EXIT:1` | `…-selftest-slice11.log` |
| selftest, slaughter | `SELFTEST=slaughter` (phase CC 200 → 201) | `FAIL: the pink slime a cow gives ((int)(health * 20)) is 201, expected 200` → `EXIT:1` | `…-selftest-slaughter-slice11.log` |
| selftests, earlier slices | contents · core · chamber · laser · latex · fluid · nether | each fails at **its own** phase (logs from the slice-10 ladder, whose phases are unchanged and re-ran green in this run's A–CL) | `…-launcher-selftest-*.log` |
| log gate | `python3 ports/tools/check-e2e-logs.py industrialforegoing-e2e --strict` | `logs=16 ok=16 missing-exit=0 bad-exit=0` | — |
| skills gate | `python3 ports/tools/check-skills.py --notes industrialforegoing` | PASS | `verify-sdk-package.py` row |

Artifact: **339,359 B**, SHA-256
`c759203e51229549e63fb4b113c76950e661576d3053472fa5e69dc5e627f4d4`, **421** entries, no packed
`Cargo.lock`, `src/runtime.rs` byte-identical to the tree (`83ae3b111494…`). The in-game reads that
matter: `Mob Crusher at [260, 101, 0]: 4950/10000 FE, tank industrialforegoing:essence 20/32000 mB,
capability industrialforegoing:essence 20 mB, progress 7/100, victims 0, output 1x
minecraft:leather, 2x minecraft:beef, last xp 1, facing west` — one cow's **1 experience point is
20 mB**, for exactly 50 FE, with the loot in the machine; and the counterfactuals — the same box with
no mob spends nothing, and the same cow with `0 FE` stays alive with the tank unchanged. The first
in-game run failed only on the break leg's expected buffer (4,550 vs the script's 4,600, which forgot
the 50 FE the kill costs); the script was corrected, not the machine.

The 0.2.9 ladder (slice 9, kept as history): tests 166; `check` 318,727 B
`fa7300d371f22f81576f452d0c1994449d69ebaf0852c89a9491dcdc7479278d`; e2e phases A–CI `PASS` +
`EXIT:0`, control and seven selftests `EXIT:1`, 10/10 logs. The 0.2.8 ladder (slice 8): tests 148;
`check` 286,421 B `f34f98ee…`; 9/9 logs. The 0.2.7 ladder (slice 7): tests 134; `check` 265,740 B
`68b63343…`; 8/8 logs. The 0.2.6 ladder (slice 6): tests 105; `check` 225,305 B `d9dbb231…`. The
0.2.5 ladder (slice 5): tests 75; `check` 189,268 B `0876ea23…bad`. The 0.2.4 ladder (slice 4):
tests 46; `check` 153,549 B `dd43090d…08f`. The 0.2.3 ladder (slice 3): tests 39; `check` 59,334 B
`819f00fc…59e7c`. The 0.2.2 ladder (slice 2): tests 26; `check` 34,096 B `2672a800…4826`.
