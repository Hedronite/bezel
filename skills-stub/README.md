# Bezel skills-stub

This directory is the skills input for the Bezel flake until you point
`inputs.skills` at your own skills repo. Bezel is not Omahedron.
`github:VirtualMachinist/omahedron-skills` is not public yet, so the flake uses this path:

```nix
inputs.skills.url = "path:./skills-stub";
```

When omahedron-skills exists, change only that URL (and lock). Do **not** add
a second repo-root `skills/` tree — the Home Manager module only symlinks
`${inputs.skills}`.

Shipped skills:

| id | Role |
| --- | --- |
| `bootstrap` | Placeholder so the input is a valid skills directory (`*/SKILL.md`) |
| `check` | Shadow Jev-first diff check (`bezel-jev --check`). Empty findings ≠ approval |

See [docs/SPIKE-stanley.md](../docs/SPIKE-stanley.md).
