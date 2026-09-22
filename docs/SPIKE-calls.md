# SPIKE — typed Calls (MCP / tool intent)

Route an MCP or tool intent into one typed call: a Choice over the operator's
catalog, closed-set arguments, then code picks **best** and **top-X**. The
result rides on the existing GateVerdict as `calls`. Transport is a FACET
v2.1.3 `tool_call`. This process does not call the tool.

Same TypeSafe client as the rest of `jev-router`. Same policy id as
route-workflow (`omapi-route-workflow-policy@1`). No new policyId. The API
key stays in the environment and is scrubbed out of catalog state.

The tiny tool index (`--catalog` / `--schema`) is a different surface. A
schema dump sets `typedCall: false` and does not rank a call. See
[SPIKE-catalog.md](SPIKE-catalog.md).

Depends on the PreToolUse map ([POLICY-MAP.md](POLICY-MAP.md)).

## What code does

Jev may name a tool. Code decides.

| Outcome | Label | Parent |
| --- | --- | --- |
| Confident `read` tool, closed-set args filled | allow | continue |
| `write` / `payment` / `filesystem` / `external` / `network` | deny | stop (`F454`) |
| Missing or invalid effect | deny | stop (`F456`) |
| Name is not a FACET identifier | deny | stop (`F452`), no OpDesc |
| No catalog, missing key, uncertain, open arg, `cannot_tell` | ask | escalate |

A denied winner is not replaced by the next row. `autoPromote` is false.
`initiated` is false. The hook still emits `defer` or `deny`, never `allow`.

`canonical.tools` lists read tools only. Other effects are omitted and the
artifact records a denied `tool_expose`.

## Shadow default

`JEV_TYPED_CALL_MODE` defaults to **shadow**. Shadow logs `calls` and does
not block. `JEV_MODE=active` does not flip this surface.
`JEV_PERMISSION_MODE=active` does not flip it either.

Key absent forces shadow, including when the operator asked for active.

Castle Grok hooks leave MCP on ask until this surface is merged. After
merge, MCP stays ask until the flip below. Do not put the variable in the
hook JSON or the flake.

## Flip

```sh
export JEV_TYPED_CALL_MODE=active
```

Only the literal `active` honors. `yes` and `true` stay shadow. Unset the
variable to go back to shadow.

When it is honoring:

- a read call that passes thresholds becomes continue only if loop-stop is not a stop
- a code deny or an ask holds the hook (`decision: deny`, exit 2)
- a typed continue does not clear a loop stop or a permission stop

## How to run

```sh
export JEV_MODE=shadow
# export JEV_TYPED_CALL_MODE=shadow   # default; leave it unset to keep MCP on ask
# export TYPESAFE_API_KEY=…           # omit: typed calls stay shadow

jev-router --calls --tools-file tools.json --top 3 --intent "list the engineering issues"
jev-router --tool-name linear__list_issues --tools-file tools.json --intent "list the engineering issues"
```

`tools.json` is an operator catalog, not a second protocol:

```json
{
  "interface": "Mcp",
  "tools": [
    {
      "name": "linear__list_issues",
      "description": "List Linear issues for a team",
      "effect": "read",
      "args": [
        {
          "name": "team",
          "question": "Which team should the issues come from?",
          "stated": "Does the intent name a team?",
          "optional": true,
          "options": { "eng": "Engineering", "ops": "Operations" }
        }
      ]
    }
  ]
}
```

`--check` is unchanged. It does not rank calls.

## Tests without live Jev

```sh
node packages/jev-router/test.mjs
```

No key + `JEV_TYPED_CALL_MODE=active`: `calls.missingKey`, `honor: false`,
`blocked: false`, `transport: facet`, `initiated: false`.

## Out of scope

Bend2, sec-routing, batteries. Invoking the MCP tool. A second TypeSafe
client. Baking `TYPESAFE_API_KEY` into the flake. Defaulting
`JEV_TYPED_CALL_MODE` to `active`. Putting JSON Schema on the tiny index.
