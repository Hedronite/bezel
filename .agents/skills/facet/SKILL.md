---
name: facet
description: >
  Inspect Facet HTTP request history, sessions, pins, and stored-value
  metadata through the facet CLI. Use when a task needs to know what requests
  ran, their status and timing, or to compare two runs. Prefer this skill over
  the facet MCP server: it adds no tool schemas to context. The MCP server
  remains available.
---

# facet — request history skill

Read-only. The body is the fixed script `facet.nu` next to this file. Every
wrapped command appends `--json` (facet expects it after the subcommand).
Do not reimplement the calls.

## Commands

```bash
nu facet.nu history [path] [--limit 20] [--request sel] [--status 200] [--session id]
nu facet.nu last [path] [--request sel]
nu facet.nu sessions [--open]
nu facet.nu session <id|current>
nu facet.nu pins [path]
nu facet.nu env [path] [--environment e]   # names only, never values
nu facet.nu diff <idA> <idB> [path]        # hashes only
nu facet.nu doctor
```

Each command prints one JSON envelope: `{ ok, command, exit, data, error }`.
`diff` uses `exit` 1 to mean the runs differ (`data.same` is false); that is
still `ok: true`.

## Not wrapped, on purpose

`facet.nu` refuses these and does not call facet:

- `history --bodies`, `history --id`, `blob`, `diff --bodies`: these can print
  recorded request or response bodies and headers, which may carry tokens.
- `request run`, including `request run --dry-run`: a preview resolves
  variables, so it can show a hydrated secret.
- `replay`, `env set`, `env delete`, `pin`, `gc --yes`,
  `session start/end`: these change state or send traffic. Run them only when
  a human has explicitly asked, and never pass a secret value on the command line.
