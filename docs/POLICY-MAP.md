# PreToolUse ↔ omapi policyId (Stitch S1)

Castle Grok hooks and this overlay share one map and one `JEV_BYPASS` predicate.
The hook calls `jev-router`. It does not construct a TypeSafe client.

Canonical matcher and class table: `PRETOOL_MATCHER` / `PRETOOL_CLASSES` in
`packages/jev-router/policy.mjs`. If this page and that module disagree, the
module wins — `node packages/jev-router/test.mjs` checks the fence below.

## GateVerdict

`jev-router` prints one JSON object. `cursor-agent-jev` and the castle hook
both read it. There is no second verdict type.

| Field | Values | Who decides |
| --- | --- | --- |
| `choice` | `continue`, `stop`, `escalate`, `bypass`, `unclassified` | Code, after Choice thresholds |
| `gate` | `auto`, `hold` | Code |
| `blocked` | bool | Code. Shadow stays false |
| `exec` | bool | `gateVerdictAllowsExec` |
| `hitl` | bool | `choice == escalate` |
| `bypass` | bool | `isJevBypass` |
| `mode` | `shadow` (default), `active` | `JEV_MODE` |
| `pretool` | stamp, only when a tool name/class was passed | The map below |

`pretool` is `{ matched, class, policyId, choiceFamily, toolName, mismatch }`.
It names which existing policy the tool class is accountable to. It does not
start another Choice call. Pass `--tool-name` / `JEV_TOOL_NAME` (and optionally
`--tool-class` / `JEV_TOOL_CLASS`). Argv wins over the env var. If the name and
`--tool-class` disagree, the stamp uses the class and sets `mismatch: true`.

`TYPESAFE_API_KEY` stays in the process environment. It is not a verdict field
and it is not Jev state.

## Policy map

Same three policy ids already in `policy.mjs`. No new catalog (permission
catalogs are PR-B).

| Class | policyId | Choice family | Grok tool names |
| --- | --- | --- | --- |
| `web` | `omapi-loop-stop-policy@1` | `continue`, `stop`, `escalate` | `web_search`, `WebSearch`, `web_fetch`, `WebFetch` |
| `subagent` | `omapi-loop-stop-policy@1` | `continue`, `stop`, `escalate` | `spawn_subagent`, `Task` |
| `shell` | `omapi-loop-stop-policy@1` | `continue`, `stop`, `escalate` | `Bash`, `run_terminal_command`, `run_terminal_cmd` |
| `write` | `omapi-check-policy@1` | `none`, `test_safety`, `task_mismatch`, `cannot_tell` (plus the concern noul) | `Write`, `Edit`, `MultiEdit`, `search_replace` |
| `mcp` | `omapi-route-workflow-policy@1` | `check`, `review`, `cannot_tell` | qualified `server__tool` (example `linear__save_issue`, `mcp__filesystem__read_file`) |

Why those families:

- Shell, web, and subagent are continuation / exec decisions. That is the loop-stop gate `cursor-agent-jev` already honors.
- Write is the diff the check gate judges. Empty findings are not approval.
- MCP is a capability choice. Unavailable or uncertain stays `cannot_tell`.

Read-class tools (`read_file`, `grep`, `list_dir`) are outside this map.
`use_tool` and `search_tool` are dispatchers, not MCP tool names. Grok's
PreToolUse event uses the qualified name, so the matcher must not be `use_tool`.

Shell lists both public spellings on purpose. The hook alias table maps `Bash`
to `run_terminal_command`. The shell tool id list uses `run_terminal_cmd`.

The live router still asks the existing loop-stop Choice for every class.
S1 does not add a per-class question set. The stamp is how castle and the
overlay name the same policyId until a later catalog (PR-B) exists.

## Matcher

Replace `hooks.PreToolUse[0].matcher` with this value. It keeps today's castle
tokens (`web_search`, `WebFetch`, `spawn_subagent`, `Task`) and adds shell,
write, and MCP.

```pretool-matcher
web_search|WebSearch|web_fetch|WebFetch|spawn_subagent|Task|Bash|run_terminal_command|run_terminal_cmd|Write|Edit|MultiEdit|search_replace|[A-Za-z0-9][A-Za-z0-9_.-]*__[A-Za-z0-9_.-]+
```

One `PreToolUse` entry. Do not add a second hook group for the new classes.

## JEV_BYPASS

One predicate, in `isJevBypass` and in `packages/cursor-agent-jev.nix`:

```sh
[ "${JEV_BYPASS:-0}" = "1" ] || [ "${JEV_BYPASS:-}" = "true" ]
```

| Value | Bypass? |
| --- | --- |
| unset, empty, `0`, `false`, `yes`, `TRUE` | no |
| `1`, `true` | yes |

What the bypass does:

| Lane | Effect |
| --- | --- |
| `cursor-agent-jev` | Skip the router and exec `cursor-agent`. |
| `jev-router` (loop-stop / default) | Skip Choice. Print `choice: bypass`, `gate: auto`, `blocked: false`, exit 0. |
| `jev-router --check` | Not the kill switch. The check workflow still runs. |
| Grok `jev-pretool.sh` | Skip `jev-router`. Exit 0 with `{"decision":"defer"}`. |

Bypass wins even if a cached verdict says stop. The wrap never reads a verdict
when the predicate matches; the hook must not either.

Never emit `{"decision":"allow"}`. Defer skips the Jev call and leaves Grok's
own permission flow in place. The wrap can exec `cursor-agent` because that
process *is* the gate. The hook is not.

`JEV_MODE` stays `shadow` unless the operator exports `active`. Do not set
`active` in the hook JSON `env`, and do not change the flake default.

## Hook decision (code deny, no auto-allow)

`pretoolHookDecision` in `policy.mjs` is the table. The hook implements it.
It does not ask the model to allow.

| Verdict | Stdout | Exit |
| --- | --- | --- |
| `bypass: true` | `{"decision":"defer","reason":"bypass"}` | 0 |
| `mode` is not `active` | `{"decision":"defer","reason":"shadow"}` | 0 |
| active, and `gateVerdictAllowsExec` (continue, or `gate: auto`, and not blocked, and not stop/escalate) | `{"decision":"defer","reason":"exec"}` | 0 |
| active otherwise | `{"decision":"deny","reason":"jev choice=… gate=…"}` | 2 |

Shadow never blocks. Active stop / escalate / hold is a code deny (exit 2),
not a hook crash. Grok fail-opens on timeout, exit 1, and malformed output.
In active mode a router failure or unparseable stdout is still an explicit
deny. In shadow that failure stays defer.

`decision` is only `defer` or `deny`.

## Castle apply checklist

Hooks are not vendored here. Apply this on the castle host. Do not add a
second client, a second bypass variable, or a second matcher entry.

### 1. `~/.grok/hooks/jev-omapi.json`

Field `hooks.PreToolUse[0].matcher`.

Current value:

```text
web_search|WebFetch|spawn_subagent|Task
```

New value: the `pretool-matcher` fence above (also `PRETOOL_MATCHER`).

Leave the rest of that group alone:

- `hooks.PreToolUse` stays a single group (do not append another object for shell/write/MCP).
- `hooks[0].type` stays `"command"`.
- `hooks[0].command` stays `bin/jev-pretool.sh` (resolves to `~/.grok/hooks/bin/jev-pretool.sh`).
- Do not set `env.JEV_MODE`. Do not put `TYPESAFE_API_KEY` in `env`.

### 2. `~/.grok/hooks/bin/jev-pretool.sh`

One script. At the top, the same predicate as the wrap:

```sh
if [ "${JEV_BYPASS:-0}" = "1" ] || [ "${JEV_BYPASS:-}" = "true" ]; then
  printf '%s\n' '{"decision":"defer","reason":"bypass"}'
  exit 0
fi
```

Then read `toolName` from the PreToolUse JSON on stdin and call the existing
router (honor `JEV_ROUTER`, default `jev-router`):

```sh
jev-router --tool-name "$toolName" --intent "$toolName"
```

Map the GateVerdict with the table above (`jq` on `.choice`, `.gate`,
`.blocked`, `.mode`, `.bypass`, `.pretool.policyId`). Do not import
`@typesafe-ai/sdk`. Do not echo the API key into stdout, stderr, or the
router's `state`.

## Tests

```sh
node packages/jev-router/test.mjs
nix flake check -L   # jev-router-unit, jev-bypass (1, true, and not yes)
```

## Out of scope

Permission catalogs (PR-B). Bend2, sec-routing, batteries. A second TypeSafe
client. Changing default `JEV_MODE` to `active`. Baking keys into the flake.
