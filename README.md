# omapi-overlay

Stock **omp** pin + thin **`omapi`** wrap + skills flake input + Jev/cursor-agent gates.

This is an **overlay**, not an omp source fork. Writers stay Kimi/GLM via Cursor auth in omp. Jev is gates only (shadow default). Tower brain-box stays off.

## Overlay, not a fork

| | This repo | Forever omp fork |
| --- | --- | --- |
| Binary humans/widget call | `omapi` (thin wrap) | a rebuilt `omp` |
| Upstream features | stock omp pin / host `omp` | reimplemented in-tree |
| Skills | `inputs.skills` only | copied into the fork |
| Jev | router-only package | vendored lab + secrets |

Do not vendor omp sources here. Do not copy Eli `jev-lab` / `config.json` / card material into a derivation.

## Install

Flake inputs (Home Manager example):

```nix
{
  inputs.omapi-overlay.url = "github:VirtualMachinist/omapi-overlay";
  # skills is already an input of omapi-overlay (sole SoT).
}

{
  nixpkgs.overlays = [ inputs.omapi-overlay.overlays.default ];
  home-manager.users.you = {
    imports = [ inputs.omapi-overlay.homeManagerModules.default ];
    programs.omapi.enable = true;
  };
}
```

Direct packages:

```sh
nix build .#omapi            # wrap of host omp / OMP_BIN (fail closed)
nix build .#omapi-pinned     # wrap of the documented fetchurl pin (180–240MB)
nix build .#jev-router
nix build .#cursor-agent-jev
```

Runtime wrap (default `omapi`): install stock omp from [omp.sh](https://omp.sh) or set `OMP_BIN` to the upstream binary. If neither is present, `omapi` exits 127 and tells you so.

Pinned wrap (`omapi-pinned`): fetchurl of [can1357/oh-my-pi v18.2.6](https://github.com/can1357/oh-my-pi/releases/tag/v18.2.6) per system. Hashes from that release’s `SHA256SUMS.txt`. Not omp source.

Copy host templates (no secrets in them):

- `templates/mcp.json`
- `templates/models.yml.example`

### wrapProgram contract

`packages/omapi-wrap.nix` uses real `makeWrapper`:

- wrap `${ompBinary}/bin/omp` → `$out/bin/omapi`
- `--prefix PATH` with node (and jev-router when passed)
- `--set-default OMAPI_OVERLAY 1`
- optional `$out/libexec/omapi-planes-shim` via `--run` when `OMAPI_PLANES=1`

Never `--set` `TYPESAFE_API_KEY` or any secret. Never `builtins.getEnv` at build. `HOME`, `OMP_*`, `OMAPI_*`, `JEV_*`, `CURSOR_*` pass through at runtime.

## Skills input (sole SoT)

```nix
inputs.skills.url = "path:./skills-stub"; # bootstrap — omahedron-skills 404s
# later: inputs.skills.url = "github:VirtualMachinist/omahedron-skills";
```

Home Manager only does:

```nix
xdg.configFile."omp/agent/skills".source = skills;
```

There is no repo-root `skills/` product tree. Swap the input URL when the skills repo exists; do not fork a second copy into this overlay.

## Shadow Jev

Default `JEV_MODE=shadow`. Choice is logged; **shadow never blocks**.

```sh
export TYPESAFE_API_KEY=   # host card store / your shell — not the flake
JEV_MODE=shadow JEV_ROUTER="$(command -v jev-router)" \
  cursor-agent-jev "<intent>" -- <cursor-agent args>
```

| Env | Role |
| --- | --- |
| `JEV_MODE=shadow` | default; log Choice, always exec `cursor-agent` |
| `JEV_MODE=active` | honor `gate: auto` only (flip is out of scope for this GOAL) |
| `JEV_BYPASS=1` | skip router, exec `cursor-agent` |
| `JEV_ROUTER` | override router binary |
| `TYPESAFE_API_KEY` | runtime only; `@typesafe-ai/sdk` reads it |

`jev-router` is Choice-only (`packages/jev-router/`, `@typesafe-ai/sdk`). No secret config in the package. Missing key: shadow continues unclassified; active fail-closes.

## Arch honesty

Declared systems: `aarch64-darwin`, `x86_64-darwin`, `aarch64-linux`, `x86_64-linux`.

| system | omp pin (v18.2.6) | Expectation |
| --- | --- | --- |
| aarch64-darwin | `omp-darwin-arm64` | Primary (castle/tower) — pin present; build when you want the binary in the store |
| x86_64-darwin | `omp-darwin-x64` | Declared and pinned (Jupi r1 missed this slice). nixpkgs 26.11 dropped Intel macOS — this flake evaluates that slice against `nixpkgs-26.05-darwin`, not by omitting the system |
| aarch64-linux | `omp-linux-arm64` | Mesh minis / Bot — pin present |
| x86_64-linux | `omp-linux-x64` | lathe — pin present; GHA runner can eval/build overlay packages |

If a future release drops a slice, `packages/omp-pin.nix` **throws** for that system (no fake Hydra green). CI on this repo only **runs** the `x86_64-linux` job; the other three matrix rows **skip with a reason** (no matching hosted runner). Skipping is not a pretend build.

`nix flake check` / `nix build .#omapi .#jev-router .#cursor-agent-jev` do **not** download the omp pin. `nix build .#omapi-pinned` does (180–240MB).

## Out of scope

- Migrating live castle `~/.local/bin/omapi`
- Flipping Jev from shadow to active
- Omapilot widget chrome
- Replacing omp upstream features
