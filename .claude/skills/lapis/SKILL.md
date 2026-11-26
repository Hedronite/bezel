---
name: lapis
description: >
  Search and read a Lapis notes vault through the lapis CLI. Use when a task
  needs notes, doctrine, project context, wikilink neighbors, or recent notes
  from the vault. Prefer this skill over the lapis MCP server: it adds no tool
  schemas to context and its body is deterministic.
---

# lapis — vault read skill

`lapis` owns the vault and its index. This skill reads the vault through the
`--json` CLI contract; `reindex` writes the index, never a note. Never open
`.lapis/` index files directly.

## Configuration

The vault path is resolved in this order:

1. `--vault <path>` passed to any command
2. `$LAPIS_VAULT`
3. the path recorded by `lapis init` in the lapis config

## Commands

```bash
lapisq search "phrase" [--limit 5] [--domain d] [--mode bm25|hybrid|vector]
lapisq read path/to/note.md [--section "Heading"] [--max-bytes N]
lapisq list [--prefix projects/] [--limit 200] [--offset 0]
lapisq neighbors path/to/note.md
lapisq resolve "[[Note Name]]"
lapisq recent [--limit 10]
lapisq status [--brief]
lapisq doctor
lapisq reindex path/to/note.md
```

`lapisq` is a PATH shim for `nu lapis.nu` (`~/.local/bin/lapisq` → `lapis.nu`), e.g. `lapisq doctor`. Non-interactive ssh needs `~/.local/bin` on PATH.

Each command prints JSON: `{ ok, command, data }` on success, plus `meta`
(`truncated`, `next`) when there is more to page, or (`stale`) when the index
lags the disk. Failure prints
`{ ok: false, command, error: { kind, message, exit } }` and exits with `exit`.
Branch on `error.kind` (`path`, `sandbox`, `not_found`, `bad_args`, …).

- `search`: one hit per note with `path`, `title`, `heading`, `score` and a
  `snippet` of up to 200 characters, so a hit can be judged without a `read`.
  `--mode bm25` (the default) skips the vector arm: about 0.2 s against 5 s
  for `hybrid`, usually with the same top hits. Use `hybrid` for fuzzy,
  concept-level queries.
- `read`: frontmatter (`hal`) plus body. `--section` keeps the part under the
  first heading containing that text; `--max-bytes` caps the body.
  `body_bytes` and `body_truncated` say what was cut.
- `list`: index metadata only; never walks the vault. The prefix is filtered in
  the index, so a folder is never reported empty because of paging.
- `neighbors`: hop-1 wikilink edges.
- `recent`: newest first by mtime.
- `status` / `doctor`: vault root and index health (exit 2 when the index is down).
  `status --brief` answers only whether the vault and index are up.
- `reindex`: index one note after it was written by hand, so search finds it.
  It writes the index only, and running it twice is harmless. Takes the same
  paths as `read`; a missing note fails with `kind: path`.
- `stale` (in search `meta`, and in `status`): notes newer on disk than in the
  index, or missing from it. Non-zero means a miss may be the index, not the
  vault: `lapisq reindex` the note you expected, then search again. Needs a lapis
  build that reports it; older builds leave it out.

## Rules

- Read before you cite: give the note path and quote the passage.
- Writes (`create`, `append`, `task`, `trash`) are not part of this skill.
  Run them only when a human has explicitly asked. `reindex` is not a write:
  it changes no note.
- If a path is missing, start `neighbors` from a hub note rather than guessing.
