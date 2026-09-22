# Bezel on Grok Build

`grok-build-bezel` is the PreToolUse adapter for the controls already in
`bezel-jev`. Grok Build calls the adapter. The adapter calls `bezel-jev`
(`jev-router` is the same program).
Policy ids, the class map, permission catalogs, the tiny tool index, and
FACET calls stay in those modules. This process does not construct a
TypeSafe client and does not copy `PRETOOL_CLASSES`.

The adapter does not install a host hook and does not change a live Grok
config. `config` prints a hook document an operator can review.

## Launch

`grok-build-bezel` does not print a banner on stdout or stderr. `catalog`,
`config`, `smoke`, and `hook` write their JSON. `hook` also writes one
decision line on stderr. There is no banner to hide, and no TTY gate.

## Boundary

| Piece | Role |
| --- | --- |
| `grok-build-bezel hook` | Read the PreToolUse event. Spawn `bezel-jev`. Map the GateVerdict with `pretoolHookDecision`. |
| `bezel-jev` | The only TypeSafe client. Stamps `pretool.policyId`, permission, catalog, and `calls`. |
| `grok-build-bezel catalog` / `schema` | Tiny index, then one schema on demand. A schema dump is not a typed call and not an allow. |
| `grok-build-bezel config` | One `PreToolUse` group. `matcher` is `PRETOOL_MATCHER`. |

`hook` writes `{ "decision": "defer" }` or `{ "decision": "deny" }`. It never
writes `allow`. Defer leaves Grok's own permission flow in place. Deny exits 2.

Tools outside the map (`read_file`, `use_tool`) are `unmatched` and defer.
That is not an approval. An empty tool name, malformed event, or empty router
verdict is uncertain: shadow stays defer, a honoring surface denies.

## Shadow and activation

Nothing here defaults a mode to `active`. The printed config does not set
`JEV_MODE`, `JEV_PERMISSION_MODE`, `JEV_TYPED_CALL_MODE`, or
`TYPESAFE_API_KEY`. Export those in the process environment when you mean to.

| Variable | Default | What `active` honors |
| --- | --- | --- |
| `JEV_MODE` | `shadow` | Loop-stop. `continue` / `gate: auto` defer. `stop` / `escalate` / hold deny. |
| `JEV_PERMISSION_MODE` | `shadow` | Shell / write / MCP permission catalogs. Does not follow `JEV_MODE`. |
| `JEV_TYPED_CALL_MODE` | `shadow` | FACET tool call on MCP. Does not follow `JEV_MODE` or `JEV_PERMISSION_MODE`. |

Only the literal `active` honors. `yes` and `true` stay shadow. A missing
`TYPESAFE_API_KEY` forces the permission and typed-call surfaces back to
shadow, including when the operator asked for `active`.

Shadow observes: the Choice is logged, `blocked` stays false, the hook
defers. Active enforces: deny / ask / escalate exit 2. A permission or typed
continue does not clear a loop stop.

Uncertain and empty Jev answers are not approval. An uncertain shell label
becomes ask. Empty write findings stay ask (`empty_findings_not_approval`).
A missing verdict denies when any surface above is honoring, and stays
shadow otherwise.

## Kill switch

One predicate, `isJevBypass`: `JEV_BYPASS=1` or `JEV_BYPASS=true`. The hook
skips `jev-router` and prints `{"decision":"defer","reason":"bypass"}`.
`yes`, `TRUE`, `0`, and `false` are not a bypass. Bypass wins over a
stop-shaped verdict.

## Irreversible actions

The adapter does not run the tool. `calls.initiated` stays false. A typed
`write` / `payment` / `filesystem` / `external` / `network` effect is a code
deny (`F454`) when typed calls are honoring. The denied winner is not
replaced by the next tool. A confident `read` still defers; it does not
allow. Humans keep Grok's permission prompt for anything the hook does not
deny.

## Smoke

No key and no network:

```sh
node packages/jev-router/harness.mjs smoke
```

From a built package:

```sh
nix build .#grok-build-bezel
./result/bin/grok-build-bezel smoke
```

The report checks four things: a `Bash` PreToolUse event maps to the shell
policy id, the catalog is tiny and has no JSON Schema, the MCP fixture
becomes a FACET `tool_call`, and a permission deny is observed in shadow and
enforced when `JEV_PERMISSION_MODE` is active. `ok` is true only when those
hold. The report contains no `decision: allow`.

`nix flake check` runs that smoke against the packaged binary.

## Config

```sh
grok-build-bezel config
```

Point the command at `grok-build-bezel hook` on `PATH`. Pass the command body
or write path through the event's `toolInput` (`command` or `path`). For MCP,
set `JEV_TOOLS_FILE` to an operator catalog (see
[SPIKE-calls.md](SPIKE-calls.md)). Do not put the API key in the hook JSON.

Hook timeout in the printed document is 30 seconds. Grok treats a timed-out
hook as fail-open, so a short timeout does not enforce an active deny.
