# Changelog

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
