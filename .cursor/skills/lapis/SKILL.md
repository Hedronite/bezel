---
name: lapis
description: >
  Search and read a Lapis notes vault through the lapis CLI. Use when a task
  needs notes, doctrine, project context, wikilink neighbors, or recent notes
  from the vault. Prefer this skill over the lapis MCP server: it adds no tool
  schemas to context and its body is deterministic. The MCP server remains
  available.
---

# lapis — vault read skill

`lapis` owns the vault and its index. This skill only reads, through the
fixed script `lapis.nu` next to this file. Never open `.lapis/` index files
directly, and do not reimplement the calls.

## Configuration

The vault path is resolved in this order:

1. `--vault <path>` passed to any command
2. `$LAPIS_VAULT`
3. the path recorded by `lapis init` in the lapis config

## Commands

```bash
nu lapis.nu search "phrase" --limit 5 [--domain d]
nu lapis.nu read path/to/note.md
nu lapis.nu list [--prefix projects/]
nu lapis.nu neighbors path/to/note.md
nu lapis.nu resolve "[[Note Name]]"
nu lapis.nu recent [--limit 10]
nu lapis.nu status
nu lapis.nu doctor
```

Each command prints one JSON envelope: `{ ok, command, exit, data, error }`.

- `search`: BM25 over chunks (vectors only if an embedder is configured); one hit per note. `data` is `path` and `title` only.
- `read`: frontmatter (`hal`) plus body.
- `list`: index metadata only; never walks the vault.
- `neighbors`: hop-1 wikilink edges.
- `recent`: newest first. Each row is read by name: `path`, `title`, and `mtime_iso` (Lapis 0.4.2) or `mtime` (older builds). `mtime_iso` wins when both are present.
- `status` / `doctor`: vault root and index health (`exit` 2 when the index is down).

## Rules

- Read before you cite: give the note path and quote the passage.
- Writes (`create`, `append`, `task`, `trash`) are refused by `lapis.nu`.
  Run them only when a human has explicitly asked, and not through this skill.
- If a path is missing, start `neighbors` from a hub note rather than guessing.
