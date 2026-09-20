# skills-stub (bootstrap placeholder)

`github:VirtualMachinist/omahedron-skills` is not public yet (404). This
directory is the **sole skills source of truth** for the overlay flake via:

```nix
inputs.skills.url = "path:./skills-stub";
```

When omahedron-skills exists, change only that URL (and lock). Do **not** add
a second repo-root `skills/` tree — the Home Manager module only symlinks
`${inputs.skills}`.
