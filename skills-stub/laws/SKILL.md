---
name: laws
description: Bend2 LAWS.bend for the Bezel bridle. The harness stays the harness. This skill carries the laws the bridle checks.
---

# laws

Bezel is not a harness. It is the bridle on one: the piece that sits on Grok, omp, or Cursor and decides what that harness may do. This skill is how that bridle travels inside the Bezel skill pack.

## What you are holding

`LAWS.bend` states invariants. `PROOF.bend` checks them once the Rust modules exist. The JavaScript prototype at `v0.1.0` is the oracle, not the thing Bend proves.

Draft these four laws. Do not invent a second runtime.

- A shadow verdict cannot set `auto_allow`.
- A code deny never calls transport.
- Empty findings are not approval.
- A `state` constructor rejects a secret-shaped key.

## Where it sits

The harness loads this skill from the Bezel pack. Bezel does not replace the harness, and it does not exec the agent. `initiated` stays false. The publish name, if `bezel` is taken, is `bezel-bridle`.

## Do not

- Treat this skill as permission to delete the JavaScript before parity
- Link `@typesafe-ai/sdk` from Rust
- Publish the crate `bezel`
- Flip the live PreToolUse matcher
- Run `cargo` on castle
