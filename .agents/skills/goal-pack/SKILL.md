---
name: goal-pack
description: >
  Materialize a multi-role goal pack (hub, REVIEW, SPEC, CHECKLIST, RUBRIC,
  GOAL, schemas, briefs) from one compact Nickel intent file. Contracts in
  law.ncl hold the fence cap, standing rows, forbidden phrases, ownership,
  and step IDs before a single byte is written. Use when a PM needs the same
  pack skeleton every time and the law should judge it, not the author.
---
# goal-pack — Nickel-authored goal packs

A goal pack is one Nickel document: `law.ncl & org overlay & pack intent`,
exported as a record of files. The skill ships the law; you write the intent.

```
<skill>/law.ncl          contracts + renderer (edit only to change the law)
<skill>/materialize.nu   runs nickel export, checks paths, writes files
<pack>/intent.ncl        plain data: product, slug, roles, steps, prose
<pack>/org-overlay.ncl   optional: org-level rules (build, merge, forbidden)
<pack>/pack.ncl          ((import "<skill>/law.ncl") & (import "./org-overlay.ncl") & (import "./intent.ncl")).out
```

`materialize.nu` renders nothing. Nickel renders every file; the script only
validates (identity, seat roles, path safety) and writes bytes atomically.

## Commands

```bash
nu materialize.nu pack.ncl --vault <dir> [--facts facts.json] [--force] [--dry-run] [--nickel <path>]
```

- `--vault` is required unless `--dry-run`.
- Nickel resolves in this order: `--nickel`, `$NICKEL_BIN`, `nickel` on `PATH`.
  No store path is hardcoded; pin the binary with `$NICKEL_BIN` in CI.
- `--facts facts.json` verifies every declared role against `seats[].role`
  and refuses with `G_OWNER_UNKNOWN` before any write.
- `--force` rewrites pack files but never touches `create_only` files
  (`STATUS.md`), and it replaces `schema/<slug>/` only after asserting the
  target is exactly `<vault>/foundry/<product>/schema/<slug>`.
- `--force` refuses with `PACK_LOCKED` when the existing hub (flat
  `<slug>.md` or upper-case `<SLUG>.md`) has any status other than
  `awaiting-approval`, including a missing or unparseable status. Nothing is
  written and `schema/<slug>/` is untouched.
- `--force` refuses with `PATH_UNSAFE` when the input `pack.ncl` resolves
  inside `<vault>/foundry/<product>/schema/<slug>`; keep intent files at
  `foundry/<product>/intent/<slug>.ncl`, never under `schema/<slug>/`.
- `--facts facts.json` verifies roles and renders live seats. The GOAL fence
  `SEATS` line and `schema/<slug>/goal.instance.json` `squad` carry real
  `pane_id`/`tab_id`, harness, model, writes, and `idle_is_fail`; `recon`
  comes from `occupancy`. Without facts, the fallback is bare roles and
  `recon: "unverified"`. Intent may set `layout = 'nested` (default `'flat`)
  to place REVIEW/SPEC/CHECKLIST/RUBRIC/GOAL under `reviews/`, `specs/`,
  `checklists/`, `rubrics/`, and `goals/`; the hub, schema, briefs, and
  wikilinks follow the same nested paths.

Each run prints one JSON line and nothing else:

```
{ ok, command, exit, data, error }
```

`ok` says the command executed. `exit` is the verdict: `0` success, `2`
refused (contract, identity, seat, collision, unsafe path), `127` could not
invoke `nickel`. Refusals still exit the process `0` so callers read the
envelope, never `$?`.

If `nickel` cannot be invoked at all (bad `--nickel`, unset `$NICKEL_BIN`,
and no `nickel` on `PATH`), the run prints
`{ ok: false, command: "materialize", exit: 127, data: null, error: "..." }`,
writes nothing, and still exits the process `0` with empty stderr.

## Intent

```nickel
{
  product = "demo",
  slug = "m1",
  title = "export pipeline",
  date = "2026-11-22",
  roles = ["pm", "backend", "qa"],
  review_body = "## Findings\n\n- F1: ...",
  spec_body = "## Target\n\n...",
  pack_forbidden = ["placeholder text in shipped copy"],
  steps = [
    { id = "G0", title = "endpoint", role = "backend", paths = ["src/api", "tests/api"],
      pass = "api tests green", fail = "any api test red or skipped" },
    { id = "G1", title = "e2e matrix", role = "qa", paths = ["tests/e2e"],
      pass = "matrix green and evidence filed", fail = "any case red or skipped",
      depends_on = ["G0"] },
  ],
}
```

`paths` are relative to the product repo and become the owner map; two writers
may not claim the same path or a path prefix of another's. `roles` must
include every step role and every role must be a lower-case identifier
(`[a-z0-9][a-z0-9-]{0,63}`).

## Overlay model (Tier 2)

An org overlay is plain data plus, where it sets a rule, a floor contract:

```nickel
let law = import "<skill>/law.ncl" in
let rules = ["no force-push", "no private names in commit text"] in
{
  org_forbidden | law.list_floor rules = rules,
  build_rule = "build on the host the build board names; never on the cockpit host",
  merge_rule = "writers never merge; merge after green CI and a clear at the head",
}
```

Additive lists are layered, never shared:

| Field | Owner | Law behavior |
|---|---|---|
| `forbidden` | law floor | guarded by `list_floor` against the law's own rules |
| `org_forbidden` | org overlay | overlay binds `list_floor`; law concatenates |
| `pack_forbidden` | pack intent | intent adds rules for this pack |

`forbidden_all = forbidden @ org_forbidden @ pack_forbidden` feeds the GOAL
fence and `goal.instance.json`. A downstream layer may add items and may never
silently drop an upper layer's items: field contracts survive every merge,
including `| priority 10` and `| force`. Dropping one is `LAYER_WEAKENED`.

The protection is opt-in per field: an overlay list set **without**
`law.list_floor` is unprotected, and a later layer can replace it. Bind the
floor to every additive list an upper layer owns.

The same monotone rule holds for the other knobs: `fence_max` may tighten
(`FENCE_CAP_WEAKENED` above 3,800), `standing` rows S1–S4 may not disappear
(`STANDING_WEAKENED`), and `forbidden_phrases` may only grow
(`PHRASES_WEAKENED`).

## Contracts and codes

| Code | Raised when |
|---|---|
| `FENCE_OVER_CAP` | rendered GOAL fence exceeds `fence_max` (default 3,800) |
| `FENCE_NON_ASCII` | fence holds a non-ASCII character |
| `FENCE_FORBIDDEN` | fence repeats a forbidden phrase (`do not run /goal`, `says GO`) |
| `FENCE_CAP_WEAKENED` | an overlay raises `fence_max` above the law cap |
| `STANDING_WEAKENED` | standing rows S1–S4 were dropped |
| `PHRASES_WEAKENED` | `forbidden_phrases` dropped a law phrase |
| `LAYER_WEAKENED` | an additive list dropped an upper layer's entry |
| `OWNER_OVERLAP` | two writers claim the same path or a path prefix |
| `OWNER_ROLE_UNDECLARED` | a step role is absent from `roles` |
| `STEP_ID_DUP` | two steps share an id |
| `STEP_ID_INVALID` | a step id does not match `^G[0-9]+[a-z]?$` |
| `IDENT_INVALID` | product, slug, or role is not a lower-case identifier |
| `G_OWNER_UNKNOWN` | a role is not present in `--facts` seats |
| `STAGE_COLLISION` | a target file exists and is not `create_only` |
| `PATH_UNSAFE` | a `files[]` path leaves the product, has an empty segment or `..`, the destructive `schema/<slug>/` target did not assert, or the input pack resolves inside `schema/<slug>/` |
| `PACK_LOCKED` | `--force` targets a hub whose status is not `awaiting-approval` |
| `NICKEL_ERROR` | Nickel rejected the pack and no coded diagnostic line exists |

Coded violations arrive as `CODE: detail` lines. A whole-pack contract reports
every violation at once (`PACK_INVALID: N violation(s)` with one entry per
violation in `notes[]`). An uncoded Nickel error keeps every message line and
note in `NICKEL_ERROR.detail`.

## Traps

- `doc` is a reserved word. Do not name a field or function `doc`.
- Record literals are recursive: `{ x = x }` and `fun x => { x = x }` refer to
  themselves and loop forever. Bind with `let` or rename the parameter.
- Arrays do not merge. Two layers setting the same array field at the same
  priority fail with `cannot merge unequal arrays`; raise the priority to pick
  a whole value, and use per-layer list fields when the values should add up.
- A field contract judges the final merged value, not the layer that wrote it.
  That is why upper-layer floors survive `| priority 10` and `| force`.

## Safety

- Written paths must stay under `foundry/<product>/`, with no empty segment,
  no `..`, no trailing slash.
- `schema/<slug>/` is replaced only after the exact target is asserted.
- A locked hub is never overwritten: `--force` checks the existing hub status
  before touching any bytes.
- Intent files live at `foundry/<product>/intent/<slug>.ncl`; an input pack
  under `schema/<slug>/` is refused with `PATH_UNSAFE`.
- Staging happens in a sibling temp directory outside the vault root and is
  removed on success or failure, so no staging directory appears inside the
  vault.
- Identity is checked twice: contracts in `law.ncl`, and an independent check
  in `materialize.nu` (`IDENT_INVALID`) that does not trust the law.
