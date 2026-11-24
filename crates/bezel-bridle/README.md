# bezel-bridle

Bridle for a coding harness. Not the harness, and not the crates.io crate [`bezel`](https://crates.io/crates/bezel) (a gpui component library).

`bezel-bridle` is the Rust body of Bezel's `jev-router`: loop-stop, the shadow diff check, permission catalogs, the tiny tool catalog, typed MCP calls, and the Grok Build `hook`, `config`, and `smoke` subcommands. The Nix package installs it as `bin/bezel-bridle`, with `bin/jev-router` exec'ing it.

`0.2.1` is the current release. `0.2.0` and `0.1.0` stay published, and their git tags are not moved. `bezel-bridle --version` prints `bezel-bridle 0.2.1`.

The crate does not call a model and does not exec an agent. Its one live command, `facet request run --environment typesafe --no-record`, is built in `cli.rs` and compiled off (`LIVE_ON = false`). Verdicts are computed offline, with recorded fixtures in `packages/jev-router/testdata` standing in for the model.

The JavaScript prototype has been removed. Its assertions live on as the unit tests in `src/oracle.rs`, which the Nix check `jev-router-unit` runs, and the `tests/oracle_*.rs` suites, which run only under `cargo test` from a clone:

```sh
cargo test --manifest-path crates/bezel-bridle/Cargo.toml
```

Bend laws ship as the `laws` skill in the Bezel pack. Proofs follow the modules they check.
