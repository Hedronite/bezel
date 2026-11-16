# Changelog

## Unreleased

- **Phantom skill.** `.agents/skills/phantom` is Phantom's code-graph skill, a byte copy of Phantom `main`, and `scripts/sync-skills` copies it into the three harness directories. `scripts/check-skill-copies.sh` covers it. `scripts/check-skill-cli.sh` runs it against a fake `phantom-fractal` that records its argv, with no real binary: no read sends `index`, `index` takes one path and no flag, and SKILL.md says where `index` writes. An agent may run `index` whenever the index is missing or stale; it writes only to Phantom's own data directory, never to the repository. The skill keeps Phantom's own envelope.
- **omp overlay.** `bezel` and `bezel-pinned` install `share/bezel/omp-overlay.yml`, which sets `tools.outputMaxColumns: 8192` and nothing else, so one-line JSON skill replies are not cut at omp's 768-byte default. `bezel-omp` passes it as `--config` ahead of the caller's arguments; a caller's own `--config` still wins. `BEZEL_OMP_OVERLAY=0` passes no overlay. `scripts/check-omp-overlay.sh` checks the default, the opt-out, and the argument order against a fake `omp`.
- The README's Skills, Packages, and Names sections describe both.
- `bezel-bridle`, the Jev router, the nixpkgs pin, and the flake inputs are unchanged. No tag.

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
