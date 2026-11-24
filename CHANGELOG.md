# Changelog

## Unreleased

- **Phantom skill.** `.agents/skills/phantom` is Phantom's code-graph skill, a byte copy of Phantom `main`, and `scripts/sync-skills` copies it into the three harness directories. `scripts/check-skill-copies.sh` covers it. `scripts/check-skill-cli.sh` runs it against a fake `phantom-fractal` that records its argv, with no real binary: no read sends `index`, `index` takes one path and no flag, and SKILL.md says where `index` writes. An agent may run `index` whenever the index is missing or stale; it writes only to Phantom's own data directory, never to the repository. The skill keeps Phantom's own envelope.
- **omp overlay.** `bezel` and `bezel-pinned` install `share/bezel/omp-overlay.yml`, which sets `tools.outputMaxColumns: 8192` and nothing else, so one-line JSON skill replies are not cut at omp's 768-byte default. `bezel-omp` passes it as `--config` ahead of the caller's arguments; a caller's own `--config` still wins. `BEZEL_OMP_OVERLAY=0` passes no overlay. `scripts/check-omp-overlay.sh` checks the default, the opt-out, and the argument order against a fake `omp`.
- **Rust router.** `jev-router` is the `bezel-bridle` binary for every argument, and the package build refuses any JavaScript. The ten `.mjs` files are removed; their assertions are the crate's `tests/oracle_*.rs` suites, run by the Nix check `jev-router-unit`. `grok-build-jev` `smoke` and `config` come from the crate.
- **Live transport.** The only live command is `facet request run --environment typesafe --no-record`, built in `cli.rs`. It is compiled off (`LIVE_ON = false`): verdicts are computed offline, and no TypeSafe SDK is linked.
- **Read-only skills.** Lapis, Facet, and Geode skills ship in `.agents/skills`, each a `SKILL.md` plus a Nushell body over the tool's JSON CLI. Skill plus CLI is the default; MCP stays supported.
- **Harness skill copies.** Home Manager copies each skill directory, as regular files, into `~/.agents/skills`, `~/.claude/skills`, `~/.grok/skills`, and `~/.cursor/skills`. A drifted file or a symlink stops the install unless `--force` is passed, which keeps the file's mode.
- **Nickel and `goal-pack`.** Nickel is pinned beside Nushell from the same nixpkgs lock. `goal-pack` renders a goal pack from one Nickel intent, and its contracts judge the pack before any write.
- **Antigravity.** `programs.bezel.antigravity.enable` (default off) registers `~/.agents/skills` in `~/.gemini/config/skills.json`.
- **Private-name guard.** `scripts/check-private-hashes.sh` scans a tree against an adopter's own salted hash list and never prints the names. It is opt-in.
- **Platforms.** `x86_64-darwin` is dropped.
- **Docs.** The README, `docs/`, and the crate README describe the Rust router and the off live transport. `LICENSE` and `AGENTS.md` are added. The orphaned `packages/jev-router/package*.json` and the unused `templates/` are removed.
- The policy ids and the nixpkgs pin are unchanged. No tag.

## 0.2.1

Compatible patch. `v0.2.0` is not moved.

- `scripts/bend-gate.sh` runs `bend` on the shipped proof and exits non-zero when a law name is missing.
- The Rust public API is unchanged.

## 0.2.0

Breaking on 0.x, per Cargo's rule. `0.1.0` is unchanged on crates.io and the git tag `v0.1.0` is not moved.

- `deterministic_flags` treats key, pem, and credentials paths as secrets, not only `.env`.
- `write_from_flags` reports `writer_parent` when flags are present.
- Hook, calls, and permission surfaces match the JavaScript oracle.
- `LAWS.bend` and `PROOF.bend` ship with the pack. Bend is not the hook.
- `bezel-bridle --version` prints `bezel-bridle 0.2.0`.

## 0.1.0

Name reservation. Code-deny flags, the `0.4` write-concern bar, and an empty check that never approves.
