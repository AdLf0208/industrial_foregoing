# Working on a native Horizon mod (SDK format, 2026-11-18)

> **Build slots (v2):** every build/test/e2e goes through the campaign runner
> (`ports/tools/build-slot.sh` from the campaign root). It **blocks in the kernel** until a
> slot is free — a freed slot is taken in ~0.07 s — so **never poll**: no
> `while ! slot; sleep` loop, no background build, no bare `cargo` outside a lease, never
> `--target-dir`. Read `../_shared/build-slots-usage.md` before running anything; observe with
> `status` / `watch` / `events` / `stats`. The runner does **not** cd into `--port`, so pass
> `-- bash -c 'cd ports/industrialforegoing && …'`, and use `BUILD_LOCAL=1` (the far box has no
> copy of this port).

This repository is the port, not Horizon and not horizon-mixins. Never modify the original
checkout or player resources; do not commit `target/`, `.horizon/`, exported sources or
Minecraft assets.

**There is no staging step and no mixin plan.** One flat `.hmod` holds
`horizon-mod.toml` (the only identity, `api = "0.2"`, `[native]` pins), `Cargo.toml` (its only
dependency is `horizon-mod-api` from the fetched `.horizon/sdk/`), `src/lib.rs` (the one
`#[mod_api::horizon_mod] impl Hooks` and the `Machine` Part) and `src/runtime.rs` (the pure
logic, the port's rule, unit-tested). `assets/` and `data/` are the carried content and travel
in the same file. The retired `.hnmod` layout is history: `../_shared/evidence/industrialforegoing/retired-hnmod-0.1.0/`.

Keep every gameplay rule and number in `src/runtime.rs` with its test; the glue in `src/lib.rs`
only reads the game, calls `runtime`, and writes the result back. `src/runtime.rs` is
byte-identical to the retired port's file — do not "tidy" it.

Validate, in this order, each inside a lease:

```sh
cargo test                                                    # runtime + glue tests
cargo fmt --check                                             # NOT --all: that checks the SDK
cargo clippy --all-targets -- -D warnings
rm -f Cargo.lock && cargo horizon-mod build                   # target/horizon-mod/<id>.hmod
cp target/horizon-mod/industrialforegoing.hmod ./industrialforegoing.hmod
cargo horizon-mod check industrialforegoing.hmod              # the registry's own check
cargo horizon-mod fits industrialforegoing.hmod --build nightly
sha256sum industrialforegoing.hmod                            # quote it in notes.md
```

A `Cargo.lock` is packed and changes the artifact: remove it immediately before the build and
keep it gitignored. The in-game leg is a **launcher build** at the campaign pin:

```sh
ports/tools/launcher-game.sh build industrialforegoing --port ports/industrialforegoing
eval "$(ports/tools/launcher-game.sh env industrialforegoing)"   # GAME/CTL/CONTROL_GAME/RES
ports/tools/build-slot.sh run --job W1-industrialforegoing --port ports/industrialforegoing \
    --step e2e -- bash ports/industrialforegoing/tools/e2e.sh
```

`CONTROL_GAME` is the same Nightly without the package, so `GAME=$CONTROL_GAME` is the control
leg (`EXIT:1`); `SELFTEST=1` and `SELFTEST=contents` are the fail-capable legs. The scenario needs
the **offscreen** renderer (screens and `click-slot` need it), survival for a block break that
drops something, and a pickaxe on the iron-block template. `BUILD_LOCAL=1` is required: the far
box has no copy of this port.
