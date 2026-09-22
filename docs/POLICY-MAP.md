# Bezel PreToolUse policy ids

Grok hooks and Bezel share one map and one `JEV_BYPASS` predicate.
The hook calls `jev-router`. It does not construct a TypeSafe client.

Canonical matcher and class table: `PRETOOL_MATCHER` / `PRETOOL_CLASSES` in
`packages/jev-router/policy.mjs`. If this page and that module disagree, the
module wins — `node packages/jev-router/test.mjs` checks the fence below.

## GateVerdict

`bezel-jev` prints one JSON object. `cursor-agent-bezel` and the Grok hook
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
| `permission` | shell / write / mcp only | Permission catalog. Shadow unless `JEV_PERMISSION_MODE=active` and a key is set |
| `catalog` | tiny tool index, `kind: tiny` | Code, from `PRETOOL_CLASSES`. No schemas |
| `calls` | MCP, or `--calls` | Typed Calls. Shadow unless `JEV_TYPED_CALL_MODE=active` and a key is set |

`pretool` is `{ matched, class, policyId, choiceFamily, toolName, mismatch }`.
It names which existing policy the tool class is accountable to. It does not
start another Choice call. Pass `--tool-name` / `JEV_TOOL_NAME` (and optionally
`--tool-class` / `JEV_TOOL_CLASS`). Argv wins over the env var. If the name and
`--tool-class` disagree, the stamp uses the class and sets `mismatch: true`.

`TYPESAFE_API_KEY` stays in the process environment. It is not a verdict field
and it is not Jev state.

## Policy map

Same three policy ids already in `policy.mjs`. Permission catalogs wrap these
ids. They do not add a policyId. See [Permission catalogs](#permission-catalogs).

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
Shell with a command body adds one permission Choice on that same call
(`allow` / `deny` / `ask`), and code maps it onto `omapi-loop-stop-policy@1`.
Write and MCP do not add a permission Choice. Write wraps the check writer.
MCP wraps route-workflow. Typed Calls add one `call` Choice on that same
systemOne when a tool catalog is present, then code ranks best and top-X.
The stamp remains the policyId. See [Typed Calls](#typed-calls).

## Matcher

Replace `hooks.PreToolUse[0].matcher` with this value. It keeps the existing
hook tokens (`web_search`, `WebFetch`, `spawn_subagent`, `Task`) and adds shell,
write, and MCP.

```pretool-matcher
web_search|WebSearch|web_fetch|WebFetch|spawn_subagent|Task|Bash|run_terminal_command|run_terminal_cmd|Write|Edit|MultiEdit|search_replace|[A-Za-z0-9][A-Za-z0-9_.-]*__[A-Za-z0-9_.-]+
```

One `PreToolUse` entry. Do not add a second hook group for the new classes.

## JEV_BYPASS

One predicate, in `isJevBypass` and in `packages/cursor-agent-bezel.nix`:

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

`JEV_PERMISSION_MODE=active` is a second gate. While it is honoring, a
permission stop / escalate denies even if `JEV_MODE` is shadow, and a
permission continue does not override a loop stop. The hook still emits
`defer` or `deny`, never `allow`. Default permission mode is shadow, so this
row does not fire until this permission surface has been re-checked. See [Permission catalogs](#permission-catalogs).

`JEV_TYPED_CALL_MODE=active` is a third gate, only for `calls`. While it is
honoring, a typed-call stop / escalate denies even if `JEV_MODE` is shadow,
and a typed-call continue does not override a loop stop or a permission stop.
The hook still emits `defer` or `deny`, never `allow`. Default is shadow.
Grok hooks leave MCP on ask until this surface is merged. After merge,
MCP stays ask until an operator exports `JEV_TYPED_CALL_MODE=active`. Do not
set that variable in the hook JSON or the flake.

`decision` is only `defer` or `deny`.

## Hook apply checklist

Hooks are not vendored here. Apply this on the machine that runs the Grok hooks. Do not add a
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
router (honor `JEV_ROUTER`, default `jev-router`). Pass the command body or
write path as `--tool-input` when the event has one. Shell permission stays
`ask` until that body is present. Do not add a second router or a second client.

```sh
jev-router --tool-name "$toolName" --tool-input "$toolInput" --intent "$toolName"
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

## Permission catalogs

PR-B. One client. Three existing policy ids. Shadow default.

| Class | policyId | Catalog | Parent map |
| --- | --- | --- | --- |
| `shell` | `omapi-loop-stop-policy@1` | new `allow` / `deny` / `ask` Choice, same call, only when the command body is present | allow→continue, deny→stop, ask→escalate. Thresholds are the loop-stop policy. No command body → ask, not allow. |
| `write` | `omapi-check-policy@1` | none. Wraps the check writer | Code deny (`secret_path`, `skip_marker_added`, `assertions_removed`, `test_file_deleted`) → deny→stop. Anything else, including empty findings → ask→writer parent. Empty findings are not approval. |
| `mcp` | `omapi-route-workflow-policy@1` | none. Wraps route-workflow. A honoring typed call may map allow→continue for a read tool | ask→escalate unless `calls.honor` selected a read tool. A workflow pick is not permission. |

Code deny wins over a model allow. A permission continue does not clear a loop stop. The hook never emits `{"decision":"allow"}`.

`permission.policyId` is one of those three ids. Classes outside this table (`web`, `subagent`, `read_file`) get no `permission` object, so they cannot carry an orphan policyId.

Key absent forces this surface to shadow (`reason: missing_key` or a logged code deny with `blocked: false`). That stays correct even if `JEV_PERMISSION_MODE=active`. `JEV_MODE=active` does not honor the catalog.

### Turn on permission mode after a re-check

Do not export `JEV_PERMISSION_MODE=active` until this permission surface has been re-checked. Shadow is the default.

After that re-check:

```sh
export JEV_PERMISSION_MODE=active
```

Only the literal `active` honors. `yes`, `true`, and `JEV_MODE=active` do not. Unset the variable to return to shadow. `cursor-agent-jev` still keys exec off `JEV_MODE`. The PreToolUse hook reads `permission.honor` via `pretoolHookDecision`.

How to run: [SPIKE-permission.md](SPIKE-permission.md).

## Tool catalog

The same three policy ids, as a tiny index on every GateVerdict (`catalog`).
`cursor-agent-jev` exports that index as `JEV_TOOL_CATALOG` when it execs,
including under `JEV_BYPASS` (the gate is skipped; the index is not).

Full JSON Schema is a separate call. It is not on the verdict and it is not
an allow.

| Path | Stdout | `cursor-agent` |
| --- | --- | --- |
| `jev-router --catalog` / `cursor-agent-jev --catalog` | `{ "kind": "tiny", "entries": […] }` | not exec'd |
| `jev-router --schema NAME` / `cursor-agent-jev --schema NAME` | `{ "call": "schema-dump", … }` | not exec'd |

A mapped tool dumps one schema with `decision: defer` and `autoAllow: false`.
An unmapped name (`read_file`, `use_tool`) is `decision: deny`, `schema: null`,
exit 2. `decision` is never `allow`.

`buildGateState` puts the tiny index on the existing `systemOne` state and
strips `TYPESAFE_API_KEY`. There is still one TypeSafe client. No new
policyId. An MCP schema dump sets `typedCall: false` because the dump is not
a typed call. This index does not read `JEV_PERMISSION_MODE` and does not
flip permission catalogs to active.

How to run: [SPIKE-catalog.md](SPIKE-catalog.md).

## Typed Calls

PR-C. One client. The route-workflow policy id. No new policyId. The tiny
index above is not this surface.

MCP and `--calls` attach `calls` on the GateVerdict. The model Choice, when an
operator tool catalog is present, is one more question on the existing
`systemOne` (`call`, plus a closed-set question per argument). Code ranks
**best** and **top-X** (default 3, max 8). Code does not promote the next tool
when the winner is denied (`autoPromote: false`).

Transport is a FACET v2.1.3 `tool_call`: an OpDesc plus a GuardDecision in
`calls.facet` / `calls.artifact`. The router does not initiate the call
(`initiated: false`). There is no jsonrpc method and no `tools/call`.

| Effect | tool_call | tool_expose |
| --- | --- | --- |
| `read` | allow, if thresholds and closed-set args pass | listed in `canonical.tools` |
| `write`, `payment`, `filesystem`, `external`, `network` | deny `F454` | omitted, guard `denied` |
| missing or invalid | deny `F456` | omitted |
| function name is not a FACET identifier | deny `F452`, no OpDesc | omitted |

Anything that is not a confident read call stays **ask** (no catalog, missing
key, uncertain, open argument, `cannot_tell`). Code deny wins over a model
allow. A continue does not clear a loop stop.

Key absent forces this surface to shadow, including when
`JEV_TYPED_CALL_MODE=active`. `JEV_MODE=active` does not honor it.
`JEV_PERMISSION_MODE=active` does not honor it either.

### Flip

Do not export this until you mean to honor MCP tool calls. Shadow is the
default. Grok hooks leave MCP on ask until this surface is merged.
After merge, still leave the variable unset unless you want the gate to deny
or continue from `calls`.

```sh
export JEV_TYPED_CALL_MODE=active
```

Only the literal `active` honors. `yes` and `true` stay shadow. Unset the
variable to return to shadow. This is not set in the flake.

How to run: [SPIKE-calls.md](SPIKE-calls.md).

## Grok Build harness

`grok-build-bezel` is the PreToolUse entrypoint for the controls on this page.
It calls `bezel-jev` and maps the GateVerdict with `pretoolHookDecision`.
It does not construct a TypeSafe client and it does not keep a second class
map. `grok-build-bezel config` prints one hook group whose `matcher` is
`PRETOOL_MATCHER`. The printed document does not set `JEV_MODE`,
`JEV_PERMISSION_MODE`, `JEV_TYPED_CALL_MODE`, or `TYPESAFE_API_KEY`.

`JEV_BYPASS` stays the one kill switch. The harness does not flip any mode
to `active`. Installing the printed config on a Grok host is an operator
step. This repository does not write that host config.

How to run: [GROK-BUILD.md](GROK-BUILD.md).

## Out of scope

Bend2, sec-routing, batteries. A second TypeSafe client. Changing default
`JEV_MODE`, `JEV_PERMISSION_MODE`, or `JEV_TYPED_CALL_MODE` to `active`.
Baking keys into the flake.
