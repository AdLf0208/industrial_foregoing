# Validation — `industrialforegoing` 0.2.2

The ladder actually run for the delivered package, cheapest first. Every command went through
`ports/tools/build-slot.sh` (`BUILD_LOCAL=1`, `CARGO_INCREMENTAL=0`, no `--target-dir`); the
in-game legs are launcher builds of this port at the campaign pin
**`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`** + mixins `e59da909…`.
Evidence: `ports/_shared/evidence/industrialforegoing/` and
`ports/_shared/evidence/industrialforegoing-e2e/`.

| step | command | result | evidence |
|---|---|---|---|
| red | `cargo test` with the three slice-2 helpers as `todo!()` | **23 passed, 3 failed** (`not yet implemented`) | `industrialforegoing/sdk-test-red.log` |
| tests | `cargo test` | **26 passed**, 0 failed | `industrialforegoing/sdk-test.log` |
| format | `cargo fmt --check` | clean | `industrialforegoing/sdk-fmt.log` |
| lints | `cargo clippy --all-targets -- -D warnings` | exit 0, 0 warnings | `industrialforegoing/sdk-clippy.log` |
| package | `rm -f Cargo.lock && cargo horizon-mod build` ×2 | `cmp`-identical, no packed `Cargo.lock` | `industrialforegoing/sdk-build.log`, `sdk-sha256.txt` |
| check | `cargo horizon-mod check industrialforegoing.hmod` | `industrialforegoing 0.2.2 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, exit 0 | `industrialforegoing/sdk-check.log` |
| uses | `cargo horizon-mod check industrialforegoing.hmod --uses` | exit 0 | `industrialforegoing/sdk-uses.txt` |
| fits (pin) | `cargo horizon-mod fits industrialforegoing.hmod --build 0a875da` | `0a875da  made for`, exit 0 | `industrialforegoing/sdk-fits.log` |
| fits (other) | `… --build 0b14f8a` | `0b14f8a  fits`, exit 0 | `industrialforegoing/sdk-fits-other.log` |
| load, before | 0.2.0 registration (redundant `block_part`) rebuilt at `0a875da`, launched | **refused**: `Mod parts refused: … Machine is attached to … twice`, `LAUNCH-EXIT:3` | `industrialforegoing/load-before-0a875da.log` |
| load, after | this package, the same launcher build at `0a875da` | `REFUSED-LINES:0`, `give … water_condensator` → `Gave 1 [Water Condensator]`, `placed -> industrialforegoing:water_condensator`, `Horizon: content from 1 mod(s)`, `EXIT:0` | `industrialforegoing/load-after-0a875da.log` |
| in-game, green | launcher build + `tools/e2e.sh` (phases A–L) | **`PASS` + `EXIT:0`** | `industrialforegoing-e2e/e2e-launcher.log` |
| control | the same script on `GAME=$CONTROL_GAME` (same Nightly, no package) | `Unknown item 'industrialforegoing:water_condensator'` → `EXIT:1` | `industrialforegoing-e2e/e2e-launcher-control.log` |
| selftest | `SELFTEST=1` (phase B 200 → 201) | `FAIL: the water is 200, expected 201` → `EXIT:1` | `…/e2e-launcher-selftest.log` |
| selftest, contents | `SELFTEST=contents` (phase L 5,000 → 5,001) | `FAIL: the restored tank is 5000, expected 5001` → `EXIT:1` | `…/e2e-launcher-selftest-contents.log` |
| red, in game | the same scenario against the **previous package** (0.2.1 at this pin) | `FAIL: the buffer the item carried is 0, expected 7000` → `EXIT:1` | `…/e2e-before-slice.log` |
| log gate | `python3 ports/tools/check-e2e-logs.py industrialforegoing-e2e --strict` | logs all carry a matching `EXIT:` | — |
| package gate | `python3 ports/tools/verify-sdk-package.py industrialforegoing` | **1/1** | — |
| skills gate | `python3 ports/tools/check-skills.py --notes ports/industrialforegoing` | PASS | — |

Artifact: **34,096 B**, SHA-256
`2672a80085cb0a258239408750b625fbee497f760693e473d823c0b947404826`, 25 entries, no packed
`Cargo.lock`, `src/runtime.rs` byte-identical to the tree (`7b377432…41b4d`).
