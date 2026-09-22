# Bezel

Bezel is a harness overlay. It supplements an agent harness you already run with skills, a Jev toolkit, and gate adapters. You keep the harness. This repository does not include harness source.

[![CI](https://img.shields.io/github/actions/workflow/status/VirtualMachinist/bezel/ci.yml?branch=main&style=flat&colorA=222222&colorB=3FB950)](https://github.com/VirtualMachinist/bezel/actions/workflows/ci.yml)
[![MIT license](https://img.shields.io/badge/License-MIT-58A6FF?style=flat&colorA=222222)](#license)
[![Nix](https://img.shields.io/badge/Nix-5277C3?style=flat&colorA=222222&logo=nixos&logoColor=white)](https://nixos.org)
[![Node](https://img.shields.io/badge/Node-339933?style=flat&colorA=222222&logo=nodedotjs&logoColor=white)](https://nodejs.org)

[What Bezel is](#what-bezel-is) ·
[What Bezel is not](#what-bezel-is-not) ·
[Quick start](#quick-start) ·
[Packages](#packages) ·
[Names](#names) ·
[Jev](#jev) ·
[Skills](#skills) ·
[Platforms](#platforms) ·
[Status](#status) ·
[Contributing](#contributing)

## What Bezel is

Bezel is the supplement, not the harness. It ships the suite in one Nix flake: skills, the Jev toolkit, and adapters that call that toolkit.

| Piece | What you get |
| --- | --- |
| Skills | Flake input `inputs.skills`. Each skill is a directory with `SKILL.md`. Home Manager symlinks that store path to the directory the harness already reads. |
| Jev | `jev-router` asks Typesafe for one policy decision and prints one JSON verdict. Shadow, the default, logs the verdict and does not block. |
| Toolkit | `jev-router --catalog` prints a small tool index. `--schema NAME` prints one JSON Schema. Reading a schema does not allow the tool. `--check` is a shadow diff check. |
| Cursor | `cursor-agent-jev` runs the gate, then execs `cursor-agent`. |
| Grok Build | `grok-build-jev` is the PreToolUse adapter. It calls `jev-router` and prints `defer` or `deny`. It does not install a host hook. |
| omp, optional | Package `bezel` installs `bezel-omp`, which runs upstream `omp` from `PATH` or `$OMP_BIN`. `bezel-pinned` points that same wrap at a fetched release binary. `omapi` is a symlink to `bezel-omp` for one release. |

Bend2 is a later engine. It is not in this tree. The Jev router is still JavaScript. A Rust rewrite is a later phase.

Use `overlays.default`, `homeManagerModules.default`, or `packages.<system>.*`.

## What Bezel is not

| Not this | What that means |
| --- | --- |
| Omarchy | Bezel is not a Linux distribution, a desktop, or an OS image. |
| An omp fork | There is no omp source here. `bezel-omp` execs the upstream `omp` binary and then gets out of the way. |
| A splash | Launch output is the wrapped program's output. There is no banner and no TTY gate. |

## Quick start

From a clone:

```sh
nix build .#bezel
nix build .#jev-router
nix build .#cursor-agent-jev
nix build .#grok-build-jev
```

`bezel-omp` runs `omp` from `PATH` or `$OMP_BIN`. If neither is present, it exits **127** and prints how to fix that. The package attribute is `bezel`. The binary is not named `bezel`.

The pinned [oh-my-pi v18.2.6](https://github.com/can1357/oh-my-pi/releases/tag/v18.2.6) binary is opt-in (~180–240 MB):

```sh
nix build .#bezel-pinned
```

### Flake input (Home Manager)

The product name is Bezel. The repository is [VirtualMachinist/bezel](https://github.com/VirtualMachinist/bezel). An input that still says `omapi-overlay` follows GitHub's redirect.

```nix
{
  inputs.bezel.url = "github:VirtualMachinist/bezel";
}

{
  nixpkgs.overlays = [ inputs.bezel.overlays.default ];
  home-manager.users.you = {
    imports = [ inputs.bezel.homeManagerModules.default ];
    programs.bezel.enable = true;
  };
}
```

`programs.bezel.enable` installs `bezel` (`bezel-omp`), `jev-router`, `cursor-agent-jev`, and `grok-build-jev`. It symlinks the skills input to `~/.config/omp/agent/skills`, the directory the omp harness loads. `programs.omapi` is deprecated; it renames to `programs.bezel`.

## Packages

| Package | What it is |
| --- | --- |
| **`bezel`** | Installs `bezel-omp`, which runs host `omp` or `$OMP_BIN`. Exits 127 if neither exists. Also installs `omapi` as a symlink to `bezel-omp` for one release. |
| **`bezel-pinned`** | Same wrap, pointed at a fetched [oh-my-pi v18.2.6](https://github.com/can1357/oh-my-pi/releases/tag/v18.2.6) binary. |
| **`jev-router`** | Node router. Asks Typesafe for a Choice and prints a JSON verdict. Loop-stop (`continue` / `stop` / `escalate`), `jev-router --check` for a shadow diff check, shadow permission catalogs, and a small tool catalog (`--catalog`, `--schema NAME`). |
| **`cursor-agent-jev`** | Runs `jev-router`, then execs `cursor-agent` (`cursor-agent` must already be on `PATH`). Shadow never blocks. Active honors continue and `gate: auto` only. `--catalog` and `--schema` print and do not exec. |
| **`grok-build-jev`** | Grok Build PreToolUse adapter. Calls `jev-router` and prints `defer` or `deny`. Does not install a host hook. |

`packages.default` is `bezel`.

## Names

The flake input and the primary package attribute are `bezel`. `packages.default` is `bezel`.

| What | Name in this release |
| --- | --- |
| Product | Bezel |
| Flake input / package | `bezel`, `bezel-pinned` |
| omp wrap binary | `bezel-omp` |
| One-release symlink | `omapi` → `bezel-omp` |
| Same package attrs | `.#omapi` is `bezel`. `.#omapi-pinned` is `bezel-pinned`. |
| Jev commands | `jev-router`, `cursor-agent-jev`, `grok-build-jev` |
| Home Manager | `programs.bezel`. `programs.omapi` is deprecated and renames to `programs.bezel`. |
| Skills path | `~/.config/omp/agent/skills` (the omp harness path) |
| Env | `BEZEL_*`. Legacy `OMAPI_*` is still read. The wrap sets both `BEZEL_OVERLAY` and `OMAPI_OVERLAY`. |

Gate version strings stay `omapi-loop-stop-policy@1`, `omapi-check-policy@1`, and `omapi-route-workflow-policy@1`. Those ids are the policy contract. The typed-call host profile id stays `omapi-jev-router`. Changing either would change verdicts, not the product name. `omp-pin` and `omp-runtime` stay those attribute names.

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

Skills are a flake input, and that input is the only skills tree Bezel installs. This repo ships `skills-stub` so the input is a valid skills tree (`*/SKILL.md`) on day one. The stub includes `bootstrap` and a shadow `check` skill (`jev-router --check`). Home Manager symlinks that store path to `~/.config/omp/agent/skills`. When you have a skills repo, change `inputs.skills.url` and the lock. Do not copy a second `skills/` tree into this overlay.

## Platforms

Declared systems: `aarch64-darwin`, `x86_64-darwin`, `aarch64-linux`, `x86_64-linux`.

| System | Pinned binary (oh-my-pi v18.2.6) | Availability |
| --- | --- | --- |
| `aarch64-darwin` | `omp-darwin-arm64` | Pin present. No hosted runner in this workflow. |
| `x86_64-darwin` | `omp-darwin-x64` | Pin present. Evaluated against `nixpkgs-26.05-darwin` (nixpkgs 26.11 dropped Intel macOS). No hosted runner in this workflow. |
| `aarch64-linux` | `omp-linux-arm64` | Pin present. No hosted runner in this workflow. |
| `x86_64-linux` | `omp-linux-x64` | Pin present. GitHub Actions evaluates all four systems and builds overlay packages here. |

If a later release drops a platform build, `packages/omp-pin.nix` **throws** for that system. A skipped CI row is a skip with a reason, not a pretend build.

`nix flake check` and `nix build .#bezel .#jev-router .#cursor-agent-jev .#grok-build-jev` do **not** download the pin. `nix build .#bezel-pinned` does.

## Status

- Overlay packages evaluate on all four declared systems.
- CI on this repo **runs** the `x86_64-linux` job. The other three matrix rows skip (no matching hosted runner).
- `bezel-pinned` is opt-in (180–240 MB download).
- Jev defaults to shadow. `jev-router --check` is the shadow diff check. Empty findings are not approval. Loop-stop is shadow-first. Active `stop` and `escalate` do not exec.
- The tool catalog is a small always-on index of the existing policy ids. Full schema is `jev-router --schema NAME` and is not an allow. Permission catalogs stay shadow.
- `grok-build-jev` adapts those controls for Grok Build. Default modes stay shadow. The harness does not install a host hook.
- Skills ship as `skills-stub` until you change the input URL.
- Nix packaging stays. The router stays JavaScript in this change.

## Contributing

Issues and pull requests are welcome. Match CI from a clone:

```sh
nix flake check -L
nix build -L .#bezel .#jev-router .#cursor-agent-jev .#grok-build-jev
```

Do not commit secrets. `TYPESAFE_API_KEY` and other keys belong in the process environment, not the flake.

## License

MIT. `bezel-pinned` downloads published binaries from [oh-my-pi](https://github.com/can1357/oh-my-pi) v18.2.6. This overlay does not include that project's source. Install a host `omp` binary from [omp.sh](https://omp.sh), or set `OMP_BIN`.

Bezel is not Omarchy.

- [This repository](https://github.com/VirtualMachinist/bezel)
- [oh-my-pi releases](https://github.com/can1357/oh-my-pi/releases/tag/v18.2.6)
