# Bezel permission Choice catalogs (shadow)

Extend the PreToolUse gate with a permission view for **shell**, **write**, and
**MCP**. This wraps the families that already exist. It is not a second
TypeSafe client and it does not add a policyId.

| Label | Parent |
| --- | --- |
| allow | continue |
| deny | stop |
| ask | escalate, or the writer parent for write |

Jev classifies. Code maps, and code denies first. The hook still emits
`defer` or `deny`. It never emits `{"decision":"allow"}`.

Depends on the PreToolUse map (`PRETOOL_CLASSES` / [POLICY-MAP.md](POLICY-MAP.md)).

## What is wrapped

| Class | policyId | What this PR adds |
| --- | --- | --- |
| shell | `omapi-loop-stop-policy@1` | One `allow` / `deny` / `ask` Choice on the existing `systemOne` call, only when the command body is present. Thresholds stay on the loop-stop policy. The command body is the content-aware gap. |
| write | `omapi-check-policy@1` | No new Choice. Deterministic writer flags deny. Empty findings stay ask (writer parent), not allow. |
| mcp | `omapi-route-workflow-policy@1` | No new permission Choice. Route-workflow is not permission to call the tool, so this catalog stays ask. Typed Calls are a separate shadow surface ([SPIKE-calls.md](SPIKE-calls.md)). |

Web and subagent stay on loop-stop with no permission object.

## Shadow default

`JEV_PERMISSION_MODE` defaults to **shadow**. Shadow logs the catalog and does
not block (`permission.blocked` stays false, `permission.honor` stays false).

`JEV_MODE=active` does **not** flip this surface. `cursor-agent-jev` still
decides exec from `JEV_MODE` alone.

Key absent forces shadow, including when `JEV_PERMISSION_MODE=active`. That
missing-key shadow path is the correct one (`missingKey: true`, not an active
deny from this catalog). A deterministic write deny is still recorded
(`codeDeny: true`) and still does not block while the key is absent.

## Turn on permission mode after a re-check

Do not honor this surface until it has been re-checked. After that:

```sh
export JEV_PERMISSION_MODE=active
```

Only the literal `active` honors. `yes` and `true` stay shadow. Unset the
variable to go back to shadow.

When it is honoring:

- deny / ask hold the hook (`decision: deny`, exit 2)
- allow becomes continue only when loop-stop thresholds pass and nothing else
  is a stop
- a loop stop still wins over a permission allow
- write empty findings and MCP workflow picks still do not allow

## How to run

```sh
export JEV_MODE=shadow
# export JEV_PERMISSION_MODE=shadow   # default; leave it until this surface is re-checked
# export TYPESAFE_API_KEY=…           # omit: permission stays shadow

jev-router --tool-name run_terminal_command --tool-input "npm test" --intent "run tests"
jev-router --tool-name search_replace --tool-input "src/app.js" --intent "edit app"
jev-router --tool-name linear__save_issue --intent "file a ticket"
```

`--check` is unchanged. It is the writer workflow, not this catalog.

## Tests without live Jev

```sh
node packages/jev-router/test.mjs
```

No key + shadow: permission `missingKey`, `blocked: false`, policyId set.
`TYPESAFE_API_KEY` is env-only.

## Out of scope

A tiny follow-up catalog (PR-D). Bend2. A second TypeSafe client. Baking
`TYPESAFE_API_KEY` into the flake. Defaulting `JEV_PERMISSION_MODE` or
`JEV_TYPED_CALL_MODE` to `active`.
