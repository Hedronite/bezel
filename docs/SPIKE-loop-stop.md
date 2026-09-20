# SPIKE — loop-stop / escalate (Suraj #4 + #10)

Give `jev-router` a **continue / stop / escalate** Choice over intent plus an
optional step / trajectory digest, and honor it in `cursor-agent-jev`.
**Do not** vendor [stanley-code](https://github.com/devagrawal09/stanley-code),
call Pi, or auto-promote a candidate.

Jev classifies; **code** applies `omapi-loop-stop-policy@1`. Escalate is
human-in-the-loop (`choice: escalate`, `gate: hold`) — not an auto-retry.

The PR #3 shadow **check** workflow is unchanged (`jev-router --check`).

## Thresholds (`omapi-loop-stop-policy@1`)

Sibling of `omapi-route-workflow-policy@1`. Same numbers, extra digest cap.

| Rule | Value | Effect |
| --- | --- | --- |
| `minConfidence` | `0.6` | Below → not a confident continue/stop |
| `minProbability` | `0.55` | Below → not a confident continue/stop |
| `minMargin` | `0.15` | Below → not a confident continue/stop |
| `maxDigestChars` | `4000` | Step digest is clipped before state / JSON |

Code mapping (not the model):

- **continue** + thresholds met → `choice: continue`, `gate: auto`
- **stop** + thresholds met → `choice: stop`, `gate: hold`
- **escalate** (even if uncertain) → `choice: escalate`, `gate: hold`, `hitl: true`
- Uncertain continue/stop in **shadow** → `choice: unclassified`, `gate: auto` (does not block)
- Uncertain continue/stop in **active** → `choice: escalate`, `gate: hold` (fail closed to HITL)
- `autoRetry` and `autoPromote` are **always** `false`

## How to run

```sh
export JEV_MODE=shadow          # default; log Choice, still exec cursor-agent
# export TYPESAFE_API_KEY=…    # host card store only; omit to continue unclassified
# export JEV_STEP_DIGEST='step 3: tests failed on auth'

jev-router --loop-stop --intent "<task>" --step-digest "optional digest"
# or the wrap:
cursor-agent-jev "<intent>" --step-digest "optional digest" -- <cursor-agent args>
```

Nix:

```sh
nix build .#jev-router .#cursor-agent-jev
./result/bin/jev-router --loop-stop --intent probe --step-digest "step one"
```

`--check` is the PR #3 workflow and is unchanged:

```sh
jev-router --check --intent "<task>" --diff-file packages/jev-router/testdata/skip-marker.diff
```

## Shadow vs active (`cursor-agent-jev`)

| Mode | Choice | Exec `cursor-agent`? |
| --- | --- | --- |
| `shadow` (default) | any, including stop/escalate | **yes** — log only, never blocks |
| `active` | `continue` or `gate: auto` | yes |
| `active` | `stop` | **no** — exit 2 |
| `active` | `escalate` | **no** — exit 2, HITL, no auto-retry |

`JEV_BYPASS=1` still skips the router.

## Escalate JSON (HITL)

```json
{
  "choice": "escalate",
  "gate": "hold",
  "hitl": true,
  "autoRetry": false,
  "autoPromote": false,
  "blocked": true,
  "exec": false,
  "loop": {
    "policy": "omapi-loop-stop-policy@1",
    "outcome": "escalate",
    "hitl": true,
    "autoRetry": false,
    "autoPromote": false
  }
}
```

A human decides. The wrap does not retry, promote, or exec.

## Tests without live Jev

```sh
node packages/jev-router/test.mjs
nix flake check -L   # jev-router-unit + jev-check-shadow + jev-loop-stop-wrap
```

No key + shadow: unclassified, `exec: true`. Fake router fixtures cover wrap
shadow/active exec rules. `TYPESAFE_API_KEY` is env-only.

## Out of scope (hard no)

Pi fallback · Stanley CLI/npm · improve-worker · `--promote-candidate` ·
full trajectory LLM verify · UI action choice · product-native Lapis/Facet
gates · baking `TYPESAFE_API_KEY` into the flake.
