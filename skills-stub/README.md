# Bezel skills-stub

This directory is the skills input for the Bezel flake until you point
`inputs.skills` at your own skills repo. The flake uses this path:

```nix
inputs.skills.url = "path:./skills-stub";
```

When you have a skills repo, change only that URL (and lock). Do **not** add
a second repo-root `skills/` tree — the Home Manager module only symlinks
`${inputs.skills}`.

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

Copy or link one directory into another harness's skills folder
(`~/.agents/skills/` or `~/.claude/skills/`). With Home Manager, the whole
input is already linked at `~/.config/omp/agent/skills`.

See [docs/SPIKE-stanley.md](../docs/SPIKE-stanley.md).
