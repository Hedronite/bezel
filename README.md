# omapi-overlay

A Nix flake overlay for agent harnesses. It adds **skills**, **Jev policy gates**, and a small **tool catalog**. You keep the harness you already run. This repository does not include harness source.

[![CI](https://img.shields.io/github/actions/workflow/status/VirtualMachinist/omapi-overlay/ci.yml?branch=main&style=flat&colorA=222222&colorB=3FB950)](https://github.com/VirtualMachinist/omapi-overlay/actions/workflows/ci.yml)
[![MIT license](https://img.shields.io/badge/License-MIT-58A6FF?style=flat&colorA=222222)](#license)
[![Nix](https://img.shields.io/badge/Nix-5277C3?style=flat&colorA=222222&logo=nixos&logoColor=white)](https://nixos.org)
[![TypeScript](https://img.shields.io/badge/TypeScript-3178C6?style=flat&colorA=222222&logo=typescript&logoColor=white)](https://www.typescriptlang.org)
[![Node](https://img.shields.io/badge/Node-339933?style=flat&colorA=222222&logo=nodedotjs&logoColor=white)](https://nodejs.org)

[What it is](#what-it-is) ·
[Quick start](#quick-start) ·
[Packages](#packages) ·
[Jev](#jev) ·
[Skills](#skills) ·
[Platforms](#platforms) ·
[Status](#status) ·
[Contributing](#contributing)

## What it is

| Piece | What it does |
| --- | --- |
| Skills | Flake input `inputs.skills`: directories that each contain `SKILL.md`. Home Manager symlinks that store path to the directory the harness reads. |
| Jev | `jev-router` asks Typesafe for a policy decision and prints one JSON verdict. Shadow, the default, logs the verdict and does not block. |
| Tooling | `jev-router --catalog` prints a small tool index. `--schema NAME` prints one JSON Schema. Reading a schema does not allow the tool. |
| Harness attach | `omapi` runs a host `omp` binary (`PATH` or `$OMP_BIN`), or a pinned release via `omapi-pinned`. `cursor-agent-jev` runs `cursor-agent` after the gate. |

Use `overlays.default`, `homeManagerModules.default`, or `packages.<system>.*`.

The commands are `omapi`, `jev-router`, `cursor-agent-jev`, and `grok-build-jev`.

## Quick start

From a clone:

```sh
nix build .#omapi
nix build .#jev-router
nix build .#cursor-agent-jev
nix build .#grok-build-jev
```

Default `omapi` runs `omp` from `PATH` or `$OMP_BIN`. If neither is present, it exits **127** and prints how to fix that.

To put the pinned [oh-my-pi v18.2.6](https://github.com/can1357/oh-my-pi/releases/tag/v18.2.6) binary in the Nix store (~180–240 MB):

```sh
nix build .#omapi-pinned
```

### Flake input (Home Manager)

```nix
{
  inputs.omapi-overlay.url = "github:VirtualMachinist/omapi-overlay";
}

{
  nixpkgs.overlays = [ inputs.omapi-overlay.overlays.default ];
  home-manager.users.you = {
    imports = [ inputs.omapi-overlay.homeManagerModules.default ];
    programs.omapi.enable = true;
  };
}
```

`programs.omapi.enable` installs `omapi` and, by default, `jev-router`, `cursor-agent-jev`, and `grok-build-jev`. It symlinks the skills input to `~/.config/omp/agent/skills`, the directory the wrapped harness loads.

## Packages

| Package | What it is |
| --- | --- |
| **`omapi`** | Runs host `omp` or `$OMP_BIN`. Exits 127 if neither exists. |
| **`omapi-pinned`** | Same wrap, pointed at a fetched [oh-my-pi v18.2.6](https://github.com/can1357/oh-my-pi/releases/tag/v18.2.6) binary. |
| **`jev-router`** | Node router. Asks Typesafe for a Choice and prints a JSON verdict. Loop-stop (`continue` / `stop` / `escalate`), `jev-router --check` for a shadow diff check, shadow permission catalogs, and a small tool catalog (`--catalog`, `--schema NAME`). |
| **`cursor-agent-jev`** | Runs `jev-router`, then execs `cursor-agent` (`cursor-agent` must already be on `PATH`). Shadow never blocks. Active honors continue and `gate: auto` only. `--catalog` and `--schema` print and do not exec. |
| **`grok-build-jev`** | Grok Build PreToolUse adapter. Calls `jev-router` and prints `defer` or `deny`. Does not install a host hook. |

## Jev

Jev is a **shadow-first** policy gate in front of `cursor-agent`. Shadow logs a decision and **does not block**. `TYPESAFE_API_KEY` is runtime-only. Set it in the process environment. The flake does not store it.

```sh
# TYPESAFE_API_KEY must already be in the environment
JEV_MODE=shadow cursor-agent-jev "<intent>" -- <cursor-agent args>
```

| Variable | Role |
| --- | --- |
| `JEV_MODE=shadow` | Default. Log the decision. Always run `cursor-agent`. |
| `JEV_MODE=active` | Honor `continue` / `gate: auto` only. `stop` / `escalate` exit 2 and do not exec. |
| `JEV_BYPASS=1` or `true` | Skip the router and run `cursor-agent`. Other values (`yes`, `false`, `0`) are not a bypass. |
| `JEV_ROUTER` | Override the router binary. |
| `JEV_STEP_DIGEST` | Optional step digest for the loop-stop decision. |
| `TYPESAFE_API_KEY` | Runtime only. [`@typesafe-ai/sdk`](https://www.npmjs.com/package/@typesafe-ai/sdk) reads it. |

Missing key: shadow continues unclassified. Active stops and waits (escalate / hold).

Loop-stop labels are `continue`, `stop`, and `escalate`, over the intent plus an optional digest.
Thresholds live in `omapi-loop-stop-policy@1` (`conf ≥ 0.6`, `p ≥ 0.55`, `margin ≥ 0.15`).
`choice: escalate` with `gate: hold` waits for a person. It does not retry on its own. How to run:
[docs/SPIKE-loop-stop.md](docs/SPIKE-loop-stop.md).

### Shadow check

`jev-router --check` reads a git diff, applies an available-gate, asks for a decision, then applies code thresholds. The result is `findings`, `parked`, and `notChecked`. Empty findings are **not** approval.

```sh
JEV_MODE=shadow jev-router --check --intent "<task>"
# or: --diff-file packages/jev-router/testdata/skip-marker.diff
```

Skill: `skills-stub/check`. How to run: [docs/SPIKE-stanley.md](docs/SPIKE-stanley.md).

Grok `PreToolUse` uses the same three policy ids and the same `JEV_BYPASS` predicate. The tool-class map, matcher (shell, write, and MCP), and the hook checklist are in [docs/POLICY-MAP.md](docs/POLICY-MAP.md). The hook calls `jev-router`. It is not a second TypeSafe client. Default mode stays shadow.

Shell, write, and MCP also get a permission catalog on those same policy ids (`allow` → continue, `deny` → stop, `ask` → escalate, or the writer parent for writes). It defaults to shadow. Set `JEV_PERMISSION_MODE=active` only after that surface has been re-checked. `JEV_MODE=active` does not turn it on. A missing `TYPESAFE_API_KEY` keeps that surface on shadow. How to run: [docs/SPIKE-permission.md](docs/SPIKE-permission.md).

### Tool catalog

Every verdict carries a small tool index (`catalog.kind = tiny`): class, policy id, and tool names. It does not carry JSON Schema. The wrap copies that index into `JEV_TOOL_CATALOG` for the `cursor-agent` process.

```sh
jev-router --catalog
cursor-agent-jev --schema Bash
```

`--schema` dumps one tool's schema and does not exec `cursor-agent`. A known tool is `decision: defer` (`autoAllow: false`). An unmapped name denies (exit 2). Neither path prints `{"decision":"allow"}`. A schema dump is not a typed call. How to run: [docs/SPIKE-catalog.md](docs/SPIKE-catalog.md). This index does not set `JEV_PERMISSION_MODE`.

MCP tool intent can also be ranked into a typed call (best and top matches) on that same route-workflow policy. The router prints a `tool_call` payload and does not invoke the tool. This surface defaults to shadow. `JEV_TYPED_CALL_MODE=active` turns it on, and the flake does not set that variable. Until you export it, MCP stays ask. How to run: [docs/SPIKE-calls.md](docs/SPIKE-calls.md).

### Grok Build harness

`grok-build-jev` is the entrypoint Grok Build calls. It does not keep a second class map or a second TypeSafe client. `hook` reads a PreToolUse event and prints `defer` or `deny`. `config` prints the hook JSON; this repo does not install it. Modes stay shadow until you export `active`. `JEV_BYPASS` is still only `1` or `true`.

```sh
node packages/jev-router/harness.mjs smoke
```

How to run: [docs/GROK-BUILD.md](docs/GROK-BUILD.md).

## Skills

Skills are a flake input, and that input is the only skills tree this overlay installs. This repo ships `skills-stub` so the input is a valid skills tree (`*/SKILL.md`) on day one. The stub includes `bootstrap` and a shadow `check` skill (`jev-router --check`). Home Manager symlinks that store path to `~/.config/omp/agent/skills`. When you have a skills repo, change `inputs.skills.url` and the lock. Do not copy a second `skills/` tree into this overlay.

## Platforms

Declared systems: `aarch64-darwin`, `x86_64-darwin`, `aarch64-linux`, `x86_64-linux`.

| System | Pinned binary (oh-my-pi v18.2.6) | Availability |
| --- | --- | --- |
| `aarch64-darwin` | `omp-darwin-arm64` | Pin present. No hosted runner in this workflow. |
| `x86_64-darwin` | `omp-darwin-x64` | Pin present. Evaluated against `nixpkgs-26.05-darwin` (nixpkgs 26.11 dropped Intel macOS). No hosted runner in this workflow. |
| `aarch64-linux` | `omp-linux-arm64` | Pin present. No hosted runner in this workflow. |
| `x86_64-linux` | `omp-linux-x64` | Pin present. GitHub Actions evaluates all four systems and builds overlay packages here. |

If a later release drops a platform build, `packages/omp-pin.nix` **throws** for that system. A skipped CI row is a skip with a reason, not a pretend build.

`nix flake check` and `nix build .#omapi .#jev-router .#cursor-agent-jev .#grok-build-jev` do **not** download the pin. `nix build .#omapi-pinned` does.

## Status

- Overlay packages evaluate on all four declared systems.
- CI on this repo **runs** the `x86_64-linux` job. The other three matrix rows skip (no matching hosted runner).
- `omapi-pinned` is opt-in (180–240 MB download).
- Jev defaults to shadow. `jev-router --check` is the shadow diff check. Empty findings are not approval. Loop-stop is shadow-first. Active `stop` and `escalate` do not exec.
- The tool catalog is a small always-on index of the existing policy ids. Full schema is `jev-router --schema NAME` and is not an allow. Permission catalogs stay shadow.
- `grok-build-jev` adapts those controls for Grok Build. Default modes stay shadow. The harness does not install a host hook.
- Skills ship as `skills-stub` until you change the input URL.

## Contributing

Issues and pull requests are welcome. Match CI from a clone:

```sh
nix flake check -L
nix build -L .#omapi .#jev-router .#cursor-agent-jev .#grok-build-jev
```

Do not commit secrets. `TYPESAFE_API_KEY` and other keys belong in the process environment, not the flake.

## License

MIT. `omapi-pinned` downloads published binaries from [oh-my-pi](https://github.com/can1357/oh-my-pi) v18.2.6. This overlay does not include that project's source. Install a host `omp` binary from [omp.sh](https://omp.sh), or set `OMP_BIN`.

- [GitHub](https://github.com/VirtualMachinist/omapi-overlay)
- [oh-my-pi releases](https://github.com/can1357/oh-my-pi/releases/tag/v18.2.6)
