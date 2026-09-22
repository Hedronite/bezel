---
name: check
description: Bounded Jev-first check of a git diff. Code gathers evidence, Jev classifies, code applies thresholds. Empty findings are not approval.
---

# check

Thin overlay skill for a bounded diff check. Prefer this
workflow when the user asks to check a diff against a task.

## Run (shadow default)

```sh
# Key from the environment only — never flags or files.
export TYPESAFE_API_KEY   # host card store / your shell; omit to smoke without live Jev

# Worktree + staged, or inject a unified diff:
bezel-jev --check --intent "<task or request>"
bezel-jev --check --intent "<task>" --base origin/main
bezel-jev --check --intent "<task>" --diff-file path/to.diff
```

`JEV_MODE` defaults to `shadow`: Choice is logged; **shadow never blocks**.
Missing `TYPESAFE_API_KEY` in shadow continues — hunks land in `notChecked`.

## Result contract

Stdout is one JSON object:

| Field | Meaning |
| --- | --- |
| `findings` | Deterministic or Jev-threshold hits |
| `parked` | Unclear; a human should look |
| `notChecked` | Skipped, unjudged, or out of scope |
| `approval` | Always `false` on this path |
| `emptyFindingsAreNotApproval` | Always `true` |

**Empty `findings` is not approval.** Do not tell the user the change is
approved, LGTM, or safe to merge because the list is empty.

## Available-gate

`check` (and the `review` routing stub) require a git diff. No diff →
`status: "no_diff"` and `cannot_tell` — do not invent a pass.

## Do not

- Invoke a `stanley` binary, another agent binary, improve-worker, or promote loop
- Pass `TYPESAFE_API_KEY` as a flag or write it to a file
- Auto-activate agent-drafted skills
