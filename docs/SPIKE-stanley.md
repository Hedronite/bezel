# Bezel shadow diff check (`bezel-jev --check`)

Bounded check: code gathers evidence, Jev classifies, code applies thresholds.
**Do not** vendor
[stanley-code](https://github.com/devagrawal09/stanley-code), install that
project's CLI, call another agent binary, or run an improve/promote loop.

Pattern source (MIT, reimplemented thin): deterministic `available(facts)` →
one Jev Choice → code thresholds → `{ findings, parked, notChecked }`. Empty
findings are not approval.

## How to run shadow check

```sh
# From a repo with a worktree/staged diff, or inject a fixture:
export JEV_MODE=shadow          # default; log only, never blocks
# export TYPESAFE_API_KEY=…    # host card store only; omit to continue unclassified

bezel-jev --check --intent "describe the change under test"
# optional:
#   --base origin/main
#   --repo /path/to/repo
#   --diff-file packages/jev-router/testdata/skip-marker.diff
```

Nix:

```sh
nix build .#bezel-jev
./result/bin/bezel-jev --check --intent probe \
  --diff-file packages/jev-router/testdata/skip-marker.diff
```

Skill (Home Manager skills input): `skills-stub/check/SKILL.md`.

## What you get

One JSON line on stdout. Stderr is `[bezel-jev] …` logs.

```json
{
  "workflow": "check",
  "approval": false,
  "emptyFindingsAreNotApproval": true,
  "status": "incomplete",
  "findings": [{ "flag": "skip_marker_added", "source": "deterministic" }],
  "parked": [],
  "notChecked": ["… TYPESAFE_API_KEY unset — unjudged (shadow continues)"]
}
```

No key + shadow: deterministic pre-pass still runs (`it.skip`, removed
`expect`/`assert`, lockfile). Jev hunks go to `notChecked`. **Never prints
`approved`.**

## Router shadow workflow Choice

Without `--check`, `bezel-jev "<intent>"` still does the writer-lane Choice.
In shadow it also:

1. Gathers facts (`diffPresent?`)
2. Applies `available()` — `check` / `review` need a diff (`review` is a stub)
3. Logs one Choice over `{check, review, cannot_tell}`
4. Applies `omapi-route-workflow-policy@1` in code (`conf ≥ 0.6`, `p ≥ 0.55`,
   `margin ≥ 0.15`). Unavailable or uncertain → `cannot_tell`
5. **Log only** — `blocked` stays false in shadow

## Tests without live Jev

```sh
node packages/jev-router/test.mjs
nix flake check -L   # includes jev-router-unit + jev-check-shadow
```

`JEV_BYPASS=1` or `JEV_BYPASS=true` still skips the exec Choice and never blocks. `--check` is not the kill switch. Shared predicate: [POLICY-MAP.md](POLICY-MAP.md).

Loop-stop / escalate is a sibling spike:
[docs/SPIKE-loop-stop.md](SPIKE-loop-stop.md). `--check` is unchanged.

## Out of scope (hard no)

stanley-code CLI/npm · another agent binary · improve-worker · `--promote-candidate` ·
baking `TYPESAFE_API_KEY` into the flake.
