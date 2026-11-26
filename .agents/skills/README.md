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
| `lapis` | Vault search and read through `lapis --json` (`lapis.nu`), and `reindex` of one note in the index. It writes no note |
| `facet` | Read-only request history through `facet … --json` (`facet.nu`). No bodies, blobs, or `request run` |
| `geode` | Metadata-only custody status through `geode --output json` (`geode.nu`). Anything else is refused |
| `phantom` | Code graph through `phantom-fractal --json` (`phantom.nu`): where, callers, impact, path, co-change. Copied byte for byte from Phantom `main` at `ee8a28b7173c35a7849f37176229e300f5838c6d` |
| `tops-verify` | Recipe only (`SKILL.md`): `phantomq diff-impact`, a focused edit, then `cargo-tops gate`, `gotops gate`, or `webtops gate` |
| `goal-pack` | Multi-role goal pack from one compact Nickel intent: hub, REVIEW, SPEC, CHECKLIST, RUBRIC, GOAL, schemas, briefs. Contracts judge the fence cap, standing rows, forbidden phrases, ownership, and step ids before any file is written |

`lapis`, `facet`, and `geode` are the default agent interface for those tools.
Each directory is a short `SKILL.md` plus one Nushell script. The Nushell that
runs them is the one pinned by this flake's nixpkgs lock. MCP servers for those
tools stay supported; this tree does not turn them off.

`facet` and `geode` print one JSON envelope:
`{ ok, command, exit, data, error }`. `lapis` prints
`{ ok, command, data, meta? }` or
`{ ok: false, command, error: { kind, message, exit } }`.

`goal-pack` is the Tier-2 skill: `SKILL.md`, `law.ncl`, and `materialize.nu`.
You write a plain-data `intent.ncl`; `law.ncl` renders every pack file and
holds the contracts; `materialize.nu` runs `nickel export`, checks identity,
seat roles, and path safety, then writes bytes atomically. A pack composes
`law.ncl & org overlay & intent`. Additive lists are per layer (`forbidden`
from the law, `org_forbidden` from the overlay, `pack_forbidden` from the
intent) and the law concatenates them; an upper layer binds
`law.list_floor <its rules>` to its own field, so a later `| priority 10` or
`| force` can add rules but never silently drop one (`LAYER_WEAKENED`). Caps
and floors follow the same rule: `fence_max` may tighten only
(`FENCE_CAP_WEAKENED`), standing rows S1-S4 and forbidden phrases may only
grow. Nickel resolves through `--nickel`, `$NICKEL_BIN`, or `PATH`; both
interpreters come from the flake's nixpkgs pin. `scripts/check-skill-cli.sh`
exercises the skill under `nix flake check`.

`phantom` is Phantom's own skill, copied from Phantom `main` and never edited
here; `scripts/check-skill-cli.sh` checks it against a fake `phantom-fractal`.
It keeps Phantom's envelope, `{ ok, command, data, meta, exit: 0 }` or
`{ ok: false, command, error, kind, stderr, exit }`, and exits with `exit`.
It needs `phantom-fractal` on `PATH` or `$PHANTOM_FRACTAL_BIN`. `index` may
run at any time; it writes Phantom's own data directory, never the repository.

`tops-verify` is composition, not code: it names the calls to make and keeps the
gate outside `phantom.nu`. `scripts/check-skill-cli.sh` checks that every
`phantomq` command it names is in `phantom.nu help`.

`programs.bezel.enable` copies each skill directory, as regular files, into:

- `~/.agents/skills`
- `~/.claude/skills`
- `~/.grok/skills`
- `~/.cursor/skills`

A second run leaves a file alone when its bytes already match, and it keeps
that file's mode. If a managed file already exists with different bytes, or
the path is a symlink, `scripts/install-user-skills.sh` prints a drift report
and writes nothing. `--force` replaces the bytes and keeps the existing mode.
`--check` reports drift and writes nothing. Unrelated skill directories
already in those folders are left in place.

See [docs/SPIKE-stanley.md](../../docs/SPIKE-stanley.md).
