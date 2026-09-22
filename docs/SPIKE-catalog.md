# SPIKE — tiny tool catalog + schema dump

Give `cursor-agent-jev` a **tiny always-on catalog** of the PreToolUse tools,
and an on-demand **schema dump** for one tool. Same `jev-router`, same three
policy ids, no second TypeSafe client.

The index is class + `policyId` + tool names. JSON Schema is not in it.
`JEV_PERMISSION_MODE` is not this surface. Permission catalogs stay at their
shadow default.

## Always on

Every GateVerdict includes `catalog`:

```json
{
  "kind": "tiny",
  "entries": [
    {
      "class": "shell",
      "policyId": "omapi-loop-stop-policy@1",
      "tools": ["Bash", "run_terminal_command", "run_terminal_cmd"]
    }
  ]
}
```

`cursor-agent-jev` exports that object as `JEV_TOOL_CATALOG` before it execs
`cursor-agent`. `JEV_BYPASS=1` still skips the Choice and still exports the
index. The Choice `state` gets the same index (`buildGateState`). The API key
is not copied into that state.

## Schema dump

```sh
jev-router --catalog
jev-router --schema Bash
cursor-agent-jev --schema search_replace
cursor-agent-jev --schema-dump linear__save_issue
```

These commands print JSON and exit. They do not exec `cursor-agent` and they
do not call Jev.

| Tool | Result |
| --- | --- |
| Mapped (`Bash`, `Write`, `linear__save_issue`, …) | `call: schema-dump`, `decision: defer`, `autoAllow: false`, one JSON Schema |
| Unmapped (`read_file`, `use_tool`) | `decision: deny`, `schema: null`, exit 2 |
| `--schema` with no name | exit 2 |

`decision` is `defer` or `deny`. It is never `allow`. An MCP dump sets
`typedCall: false` and uses `omapi-route-workflow-policy@1`. A schema dump is
not a typed call. Ranking an intent into a FACET `tool_call` is
[SPIKE-calls.md](SPIKE-calls.md), and that surface stays shadow.

## Tests without live Jev

```sh
node packages/jev-router/test.mjs
nix flake check -L   # jev-tool-catalog, plus the existing router checks
```

## Out of scope

Flipping `JEV_PERMISSION_MODE` to `active`. Bend2. A second TypeSafe client.
A new policyId. Baking `TYPESAFE_API_KEY` into the flake.
