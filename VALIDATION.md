# Validation — `industrialforegoing` 0.2.9

The ladder actually run for the delivered package, cheapest first. Every compile went through
`ports/tools/build-slot.sh` (`BUILD_LOCAL=1`, `CARGO_INCREMENTAL=0`, no `--target-dir`); `check` and
both `fits` runs are the tool's own non-compiling checks, run in the port. The in-game legs are
launcher builds of this port at the campaign pin
**`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`** + mixins `e59da909…`; the exported game's own
package reads `id = "industrialforegoing"`, `version = "0.2.9"`
(`evidence/industrialforegoing-e2e/launcher-build-slice9.log`, whose provenance stanza prints the
manifest, the library and the delivered sha). Evidence: `ports/_shared/evidence/industrialforegoing/`
and `ports/_shared/evidence/industrialforegoing-e2e/`.

| step | command | result | evidence |
|---|---|---|---|
| red (rules + glue) | `cargo test` with slice 9's `yields_for`, `Factory::tick`/`rest`, `tanks`, `behind`, `animal_category`, `read_tanks`/`write_tanks`, `carried_patch` and `clamp_patch` at `todo!()` | **149 passed, 17 failed**, every failure `panicked at src/… not yet implemented` (e.g. `slaughter::tests::a_cow_gives_ten_meat_and_eighty_slime`, `mobslaughter::tests::the_factory_declares_two_eight_thousand_mb_tanks`) | `industrialforegoing/sdk-test-red-slice9.log` |
| tests | `cargo test` | **166 passed**, 0 failed (`Compiling industrialforegoing v0.2.9`) | `industrialforegoing/sdk-test.log` |
| format | `cargo fmt --check` | clean | `industrialforegoing/sdk-fmt.log` |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | exit 0, 0 warnings | `industrialforegoing/sdk-clippy.log` |
| content | `python3 tools/verify_content.py` (tree **and** the delivered `.hmod`), `--selftest`, `tools/assemble_slaughter.py --check` | `CONTENT OK` (391 files, both tree and package); the selftest catches **17** mutations, incl. the factory's declaration, the pink slime ingot's tag, an eight-for-one pink slime block, the pink slime ball's `outputFluid` and the pink slime block's friction; the generator is `identical` (39 files) | `industrialforegoing/sdk-content-slice9.log` |
| package | `rm -f Cargo.lock && cargo horizon-mod build` ×2 | `cmp`-identical, sha256 `fa7300d3…` both times, **409** entries, no packed `Cargo.lock` | `industrialforegoing/sdk-build-slice9.log`, `…-2.log` |
| check | `cargo horizon-mod check industrialforegoing.hmod` | `industrialforegoing 0.2.9 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, **318,727 B**, sha256 `fa7300d371f22f81576f452d0c1994449d69ebaf0852c89a9491dcdc7479278d`, exit 0 | `industrialforegoing/sdk-check.log` |
| fits (pin) | `cargo horizon-mod fits industrialforegoing.hmod --build 0a875da` | `0a875da  made for`, exit 0 | `industrialforegoing/sdk-fits.log` |
| fits (other) | `… --build 0b14f8a` | `0b14f8a  fits`, exit 0 | `industrialforegoing/sdk-fits.log` |
| package gate | `python3 ports/tools/verify-sdk-package.py industrialforegoing` | **1/1** (runtime byte-identical `83ae3b11…`, no packed lock, check+`fits`, 166 tests, 0 warnings, skills) | — |
| in-game, green | `launcher-game.sh build industrialforegoing --port …` (`Compiling industrialforegoing (1 of 1)`) + `tools/e2e.sh` (phases A–CI) | **`PASS` + `EXIT:0`** — phase BN (the chamber's `outputFluid`) and CA–CI (the factory) are quoted in `notes.md` | `industrialforegoing-e2e/e2e-launcher.log` |
| control | the same script on `GAME=$CONTROL_GAME` (same Nightly, no package) | `FAIL: the machine item did not register` → `EXIT:1` | `…-control.log` |
| selftest | `SELFTEST=1` (phase B 200 → 201) | `FAIL: the water is 200, expected 201` → `EXIT:1` | `…-selftest.log` |
| selftest, slaughter | `SELFTEST=slaughter` (phase CC's 200 mB of pink slime → 201) | `FAIL: the pink slime a cow gives ((int)(health * 20)) is 201, expected 200` → `EXIT:1` | `…-selftest-slaughter.log` |
| probe (the open question) | the same package in a clean world: the chamber's `pink_slime_ball` twice | 150 then 300 mB of `minecraft:water` in the output tank | `industrialforegoing/probe-chamber-outputfluid.log` |
| log gate | `python3 ports/tools/check-e2e-logs.py industrialforegoing-e2e --strict` | `logs=10 ok=10 missing-exit=0 bad-exit=0` | — |
| skills gate | `python3 ports/tools/check-skills.py --notes industrialforegoing` | PASS | `verify-sdk-package.py` row |

The other six failure logs (`contents`, `core`, `chamber`, `laser`, `latex`, `fluid`) are the earlier
slices' own selftests; each fails at its own phase (L, U, Z, BD, AF, BJ) and still reads `EXIT:1`,
which is why the gate above counts them.

Artifact: **318,727 B**, SHA-256
`fa7300d371f22f81576f452d0c1994449d69ebaf0852c89a9491dcdc7479278d`, **409** entries, no packed
`Cargo.lock`, `src/runtime.rs` byte-identical to the tree (`83ae3b111494…`). The slice's own tests:
8 rules in `src/slaughter.rs`, 9 glue tests in `src/mobslaughter.rs` and 1 content test (18 new).
The in-game read that matters: `Mob Slaughter Factory at [250, 101, 0]: 4600/10000 FE, tank
industrialforegoing:meat 10/8000 mB, capability industrialforegoing:meat 10 mB, tank
industrialforegoing:pink_slime 200/8000 mB, capability industrialforegoing:pink_slime 200 mB,
progress 13/100, victims 0, facing west` after a cow, and `Dissolution Chamber at [220, 101, 0]:
1090/10000 FE, input empty 0/8000 mB, output minecraft:water 150/16000 mB, progress 0/0, … items 0,
recipes 25/25` after the pink slime ball.

The 0.2.8 ladder (slice 8, kept as history): tests 148; `check` 286,421 B `f34f98ee…`; e2e phases
A–BM `PASS` + `EXIT:0`, control and five selftests `EXIT:1`, 9/9 logs. The 0.2.7 ladder (slice 7):
tests 134; `check` 265,740 B `68b63343…`; 8/8 logs. The 0.2.6 ladder (slice 6): tests 105; `check`
225,305 B `d9dbb231…`. The 0.2.5 ladder (slice 5): tests 75; `check` 189,268 B `0876ea23…bad`. The
0.2.4 ladder (slice 4): tests 46; `check` 153,549 B `dd43090d…08f`. The 0.2.3 ladder (slice 3): tests
39; `check` 59,334 B `819f00fc…59e7c`. The 0.2.2 ladder (slice 2): tests 26; `check` 34,096 B
`2672a800…4826`.
