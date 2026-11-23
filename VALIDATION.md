# Validation — `industrialforegoing` 0.2.8

The ladder actually run for the delivered package, cheapest first. Every compile went through
`ports/tools/build-slot.sh` (`BUILD_LOCAL=1`, `CARGO_INCREMENTAL=0`, no `--target-dir`); `check` and
both `fits` runs are the tool's own non-compiling checks, run in the port. The in-game legs are
launcher builds of this port at the campaign pin
**`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`** + mixins `e59da909…`; the exported game's own
package reads `id = "industrialforegoing"`, `version = "0.2.8"`
(`evidence/industrialforegoing-e2e/launcher-build-slice8.log`, whose provenance stanza prints the
manifest and `libindustrialforegoing.so`). Evidence: `ports/_shared/evidence/industrialforegoing/`
and `ports/_shared/evidence/industrialforegoing-e2e/`.

| step | command | result | evidence |
|---|---|---|---|
| red (rules) | `cargo test` with slice 8's `choice`, `Tank::fill`, `Base::advance` and `data_matches` at `todo!()` | **142 passed, 6 failed**, every failure `panicked at src/fluidlaser.rs: not yet implemented` (e.g. `the_first_recipe_whose_catalyst_rarity_and_gate_pass_is_the_one_used`) | `industrialforegoing/sdk-test-red-slice8.log` |
| tests | `cargo test` | **148 passed**, 0 failed (`Compiling industrialforegoing v0.2.8`) | `industrialforegoing/sdk-test.log` |
| format | `cargo fmt --check` | clean (`FMT-CLEAN`) | `industrialforegoing/sdk-clippy.log` |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | exit 0, 0 warnings (`CLIPPY-EXIT:0`) | `industrialforegoing/sdk-clippy.log` |
| content | `python3 tools/verify_content.py` (tree and delivered `.hmod`), `--selftest`, `tools/assemble_fluidlaser.py --check` | `CONTENT OK` (355 files, both tree and package); the selftest catches 11 mutations, incl. the fluid base's ether recipe and a fluid recipe's NeoForge condition; the generator is `identical` (16 files) | `industrialforegoing/sdk-content-slice8.log` |
| package | `rm -f Cargo.lock && cargo horizon-mod build` ×2 | `cmp`-identical, sha256 `f34f98ee…` both times, **371** entries, no packed `Cargo.lock` | `industrialforegoing/sdk-build-slice8.log`, `…-2.log` |
| check | `cargo horizon-mod check industrialforegoing.hmod` | `industrialforegoing 0.2.8 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, **286,421 B**, sha256 `f34f98ee0c57a700595160189162ee80856e962df12218d04f7ab7529fbbbfc9`, exit 0 | `industrialforegoing/sdk-check.log` |
| fits (pin) | `cargo horizon-mod fits industrialforegoing.hmod --build 0a875da` | `0a875da  made for`, exit 0 | `industrialforegoing/sdk-fits.log` |
| fits (other) | `… --build 0b14f8a` | `0b14f8a  fits`, exit 0 | `industrialforegoing/sdk-fits.log` |
| package gate | `python3 ports/tools/verify-sdk-package.py industrialforegoing` | **1/1** (runtime byte-identical `83ae3b11…`, no packed lock, check+`fits`, 148 tests, 0 warnings, skills) | — |
| in-game, green | `launcher-game.sh build industrialforegoing --port … --rebuild` + `tools/e2e.sh` (phases A–BM) | **`PASS` + `EXIT:0`** — the fluid-base phases BI–BM are quoted in `notes.md` | `industrialforegoing-e2e/e2e-launcher.log` |
| control | the same script on `GAME=$CONTROL_GAME` (same Nightly, no package) | `FAIL: the machine item did not register` → `EXIT:1` | `…-control.log` |
| selftest | `SELFTEST=1` (phase B 200 → 201) | `FAIL: the water is 200, expected 201` → `EXIT:1` | `…-selftest.log` |
| selftest, fluid | `SELFTEST=fluid` (phase BJ's 10 mB of ether → 11) | `FAIL: the ether one bar yields is 11, expected 10` → `EXIT:1` | `…-selftest-fluid.log` |
| log gate | `python3 ports/tools/check-e2e-logs.py industrialforegoing-e2e --strict` | `logs=9 ok=9 missing-exit=0 bad-exit=0` | — |
| skills gate | `python3 ports/tools/check-skills.py --notes industrialforegoing` | PASS | `verify-sdk-package.py` row |

The other five failure logs (`contents`, `core`, `chamber`, `laser`, `latex`) fail at their own phase
(L, U, Z, BD, AF) — all **before** the fluid-base phases this slice added — and still read
`EXIT:1`; the gate above counts them.

Artifact: **286,421 B**, SHA-256
`f34f98ee0c57a700595160189162ee80856e962df12218d04f7ab7529fbbbfc9`, 371 entries, no packed
`Cargo.lock`, `src/runtime.rs` byte-identical to the tree (`83ae3b111494…`). The slice's own tests:
8 rules in `src/fluidlaser.rs`, 5 glue tests in `src/fluidbase.rs` and 1 content test (14 new). The
in-game read that matters: `Fluid Laser Base at [240, 101, 0]: tank
industrialforegoing:ether_gas 10/32000 mB, capability industrialforegoing:ether_gas 10 mB, progress
0/20, depth 101, …, recipe industrialforegoing:laser_drill_fluid/ether, recipes 2/3, lens
industrialforegoing:purple_laser_lens`, with the drill at `8000/10000 FE` (one charge) and the same
box at `depth 300` reading `recipe none`.

The 0.2.7 ladder (slice 7, kept as history): tests 134; `check` 265,740 B `68b63343…`; e2e phases
A–BH `PASS` + `EXIT:0`, control and five selftests `EXIT:1`, 8/8 logs. The 0.2.6 ladder (slice 6,
kept as history): tests 105; `check` 225,305 B `d9dbb231…`; e2e phases A–AM `PASS` + `EXIT:0`,
control and five selftests `EXIT:1`. The 0.2.5 ladder (slice 5): tests 75; `check` 189,268 B
`0876ea23…bad`. The 0.2.4 ladder (slice 4): tests 46; `check` 153,549 B `dd43090d…08f`. The 0.2.3
ladder (slice 3): tests 39; `check` 59,334 B `819f00fc…59e7c`. The 0.2.2 ladder (slice 2): tests 26;
`check` 34,096 B `2672a800…4826`.
