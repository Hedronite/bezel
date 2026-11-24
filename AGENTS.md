# AGENTS.md

Rules for an agent working in this repository. Read [README.md](README.md) first.

## Layout

- `.agents/skills/` is the only skills source. `.claude/skills/`, `.grok/skills/`, and `.cursor/skills/` are regular-file copies. Edit the canonical tree, then run `scripts/sync-skills`. CI fails when a copy drifts.
- `.agents/skills/phantom/` is a byte copy of Phantom `main`. Do not edit it here; re-copy it and record the commit in `.agents/skills/README.md`.
- `crates/bezel-bridle/` is the Rust router. `packages/jev-router/` builds it and holds the recorded fixtures in `testdata/`.
- `docs/POLICY-MAP.md` is read by a crate test. Keep its `pretool-matcher` fence byte-equal to `PRETOOL_MATCHER` in `crates/bezel-bridle/src/policy.rs`.

## Checks

Run before every pull request:

```sh
nix flake check -L
nix build -L .#bezel .#jev-router .#cursor-agent-jev .#grok-build-jev
```

`cargo test --manifest-path crates/bezel-bridle/Cargo.toml` is the fast loop for router changes.

## Rules

- Never commit secrets. `TYPESAFE_API_KEY` is runtime only: never `--set` it in a wrapper and never read it at build time.
- Keep every Jev surface shadow by default. Do not flip `LIVE_ON`, and do not default any `JEV_*_MODE` to `active`.
- Do not change the policy ids (`omapi-*-policy@1`) or the host profile id. They are the verdict contract.
- Keep private host, seat, person, and path names out of tracked files and commit text. `scripts/check-private-hashes.sh` scans against your own salted list.
- One pull request per change. Do not push `main`, force-push, or tag without the maintainer.
