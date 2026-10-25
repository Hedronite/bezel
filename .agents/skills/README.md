# Bezel skills

This directory is the canonical skills tree and the flake input:

```nix
inputs.skills.url = "path:./.agents/skills";
```

When you have a skills repo, change only that URL (and lock). Do **not** add
a repo-root `skills/` tree, `.omp/skills/`, or `.codex/skills/`.

`scripts/sync-skills` rewrites the regular-file copies in `.claude/skills/`,
`.grok/skills/`, and `.cursor/skills/` from this tree. Edit here, then sync.
Those copies are not a second source.

Shipped skills:

| id | Role |
| --- | --- |
| `bootstrap` | Placeholder so the input is a valid skills directory (`*/SKILL.md`) |
| `check` | Shadow Jev-first diff check (`jev-router --check`). Empty findings ≠ approval |
| `laws` | Bend2 `LAWS.bend` for the bridle. The harness loads it. Bezel does not become the harness |
| `lapis` | Read-only vault search and read through `lapis --json` (`lapis.nu`) |
| `facet` | Read-only request history through `facet … --json` (`facet.nu`). No bodies, blobs, or `request run` |
| `geode` | Metadata-only custody status through `geode --output json` (`geode.nu`). Anything else is refused |

`lapis`, `facet`, and `geode` are the default agent interface for those tools.
Each directory is a short `SKILL.md` plus one Nushell script. Every command
prints the same JSON envelope: `{ ok, command, exit, data, error }`. The
Nushell that runs them is the one pinned by this flake's nixpkgs lock. MCP
servers for those tools stay supported; this tree does not turn them off.

`programs.bezel.enable` copies each skill directory, as regular files, into:

- `~/.agents/skills`
- `~/.claude/skills`
- `~/.grok/skills`
- `~/.cursor/skills`

A second run leaves a file alone when its bytes already match.
`scripts/install-user-skills.sh --check` reports drift and writes nothing.
Unrelated skill directories already in those folders are left in place.

See [docs/SPIKE-stanley.md](../../docs/SPIKE-stanley.md).
