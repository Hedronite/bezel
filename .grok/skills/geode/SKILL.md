---
name: geode
description: >
  Check Geode vault health and metadata through the geode CLI: entry names,
  integrity, recipients, snapshots, policy, audit chain, custody status. Use
  when a task asks whether a vault is intact, who can open it, or what it
  holds by name. This skill never reads or prints secret values. Prefer it
  over the geode MCP server. The MCP server remains available.
---

# geode — custody status skill (metadata only)

The body is the fixed script `geode.nu` next to this file. Every wrapped
command uses `geode --output json`. The vault path comes from the argument or
`$GEODE_VAULT`; the identity key from `$GEODE_KEY_FILE` as usual. The script
never puts the key on the command line. Do not reimplement the calls.

## Commands

```bash
nu geode.nu list [vault] [prefix]      # entry names and sizes, no contents
nu geode.nu verify [vault] [--sample 0.05]   # default --cheap
nu geode.nu recipients [vault]         # recipient types and key ids (public)
nu geode.nu snapshots [vault]
nu geode.nu policy [vault]
nu geode.nu can <principal> <op> <path> [vault]   # dry run; deny is exit 3
nu geode.nu audit
nu geode.nu custody
nu geode.nu keyring
```

Each command prints one JSON envelope: `{ ok, command, exit, data, error }`.
`can` keeps `ok: true` when geode exits 3 (deny). `audit` keeps `ok: true`
when geode exits 2 (chain failed).

## Never run from this skill

`geode.nu` refuses anything that is not in the list above, including `cat`,
`open`, `mount`, `run`, `agent`, `keygen`, `seal`, `sign`, `vault init`,
`vault add-recipient`, `vault rotate`, `policy set`, `keyring add`,
`custody unlock/lock`, `gc`, and `git unlock`. Those read secret bytes, put
them in an environment or on disk, or change custody. They need a human, and
their output must never be pasted into notes, logs, or chat.

Entry names can themselves be sensitive. Quote them only where the reader is
allowed to know the vault holds them.
