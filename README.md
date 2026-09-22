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
| Jev | `bezel-jev` asks Typesafe for one policy decision and prints one JSON verdict. Shadow, the default, logs the verdict and does not block. |
| Toolkit | `bezel-jev --catalog` prints a small tool index. `--schema NAME` prints one JSON Schema. Reading a schema does not allow the tool. `--check` is a shadow diff check. |
| Cursor | `cursor-agent-bezel` runs the gate, then execs `cursor-agent`. |
| Grok Build | `grok-build-bezel` is the PreToolUse adapter. It calls `bezel-jev` and prints `defer` or `deny`. It does not install a host hook. |
| omp, optional | `bezel` runs upstream `omp` from `PATH` or `$OMP_BIN`. `bezel-pinned` points that same wrap at a fetched release binary. |

Bend2 is a later engine. It is not in this tree. The Jev router is still JavaScript. A Rust rewrite is a later phase.

Use `overlays.default`, `homeManagerModules.default`, or `packages.<system>.*`.

## What Bezel is not

| Not this | What that means |
| --- | --- |
| Omarchy | Bezel is not a Linux distribution, a desktop, or an OS image. |
| Omahedron | Bezel is not the skills product. Skills are a flake input. This tree ships a bootstrap stub until you point that input elsewhere. |
| An omp fork | There is no omp source here. The `bezel` command execs the upstream `omp` binary and then gets out of the way. |
| A splash | Launch output is the wrapped program's output. There is no banner and no TTY gate. |

## Quick start

From a clone:

```sh
nix build .#bezel
nix build .#bezel-jev
nix build .#cursor-agent-bezel
nix build .#grok-build-bezel
```

`bezel` runs `omp` from `PATH` or `$OMP_BIN`. If neither is present, it exits **127** and prints how to fix that.

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

`programs.bezel.enable` installs `bezel`, `bezel-jev`, `cursor-agent-bezel`, and `grok-build-bezel`. It symlinks the skills input to `~/.config/omp/agent/skills`, the directory the omp harness loads. `programs.omapi` is a renamed-option alias for `programs.bezel`.

## Packages

| Package | What it is |
| --- | --- |
| **`bezel`** | Runs host `omp` or `$OMP_BIN`. Exits 127 if neither exists. Also installs `omapi` as a compatibility name for the same program. |
| **`bezel-pinned`** | Same wrap, pointed at a fetched [oh-my-pi v18.2.6](https://github.com/can1357/oh-my-pi/releases/tag/v18.2.6) binary. |
| **`bezel-jev`** | Node router. Asks Typesafe for a Choice and prints a JSON verdict. Loop-stop (`continue` / `stop` / `escalate`), `bezel-jev --check` for a shadow diff check, shadow permission catalogs, and a small tool catalog (`--catalog`, `--schema NAME`). |
| **`cursor-agent-bezel`** | Runs `bezel-jev`, then execs `cursor-agent` (`cursor-agent` must already be on `PATH`). Shadow never blocks. Active honors continue and `gate: auto` only. `--catalog` and `--schema` print and do not exec. |
| **`grok-build-bezel`** | Grok Build PreToolUse adapter. Calls `bezel-jev` and prints `defer` or `deny`. Does not install a host hook. |

`packages.default` is `bezel`.

## Names

One transition. The previous flake attributes are the same packages, not a second implementation.

| Flake attribute | Command | Previous attribute | Previous command |
| --- | --- | --- | --- |
| `bezel` | `bezel` | `omapi` | `omapi` |
| `bezel-pinned` | `bezel` | `omapi-pinned` | `omapi` |
| `bezel-jev` | `bezel-jev` | `jev-router` | `jev-router` |
| `cursor-agent-bezel` | `cursor-agent-bezel` | `cursor-agent-jev` | `cursor-agent-jev` |
| `grok-build-bezel` | `grok-build-bezel` | `grok-build-jev` | `grok-build-jev` |

Gate version strings stay `omapi-loop-stop-policy@1`, `omapi-check-policy@1`, and `omapi-route-workflow-policy@1`. Those ids are the policy contract. The typed-call host profile id stays `omapi-jev-router`. Changing either would change verdicts, not the product name.

The omp wrap sets `BEZEL_OVERLAY=1` and still sets `OMAPI_OVERLAY=1`. `BEZEL_PLANES=1` and `OMAPI_PLANES=1` both run the same pre-exec filter.

## Jev

Jev is a **shadow-first** policy gate in front of `cursor-agent`. Shadow logs a decision and **does not block**. `TYPESAFE_API_KEY` is runtime-only. Set it in the process environment. The flake does not store it.

```sh
# TYPESAFE_API_KEY must already be in the environment
JEV_MODE=shadow cursor-agent-bezel "<intent>" -- <cursor-agent args>
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

`bezel-jev --check` reads a git diff, applies an available-gate, asks for a decision, then applies code thresholds. The result is `findings`, `parked`, and `notChecked`. Empty findings are **not** approval.

```sh
JEV_MODE=shadow bezel-jev --check --intent "<task>"
# or: --diff-file packages/jev-router/testdata/skip-marker.diff
```

Skill: `skills-stub/check`. How to run: [docs/SPIKE-stanley.md](docs/SPIKE-stanley.md).

Grok `PreToolUse` uses the same three policy ids and the same `JEV_BYPASS` predicate. The tool-class map, matcher (shell, write, and MCP), and the hook checklist are in [docs/POLICY-MAP.md](docs/POLICY-MAP.md). The hook calls `bezel-jev`. It is not a second TypeSafe client. Default mode stays shadow.

Shell, write, and MCP also get a permission catalog on those same policy ids (`allow` → continue, `deny` → stop, `ask` → escalate, or the writer parent for writes). It defaults to shadow. Set `JEV_PERMISSION_MODE=active` only after that surface has been re-checked. `JEV_MODE=active` does not turn it on. A missing `TYPESAFE_API_KEY` keeps that surface on shadow. How to run: [docs/SPIKE-permission.md](docs/SPIKE-permission.md).

### Tool catalog

Every verdict carries a small tool index (`catalog.kind = tiny`): class, policy id, and tool names. It does not carry JSON Schema. The wrap copies that index into `JEV_TOOL_CATALOG` for the `cursor-agent` process.

```sh
bezel-jev --catalog
cursor-agent-bezel --schema Bash
```

`--schema` dumps one tool's schema and does not exec `cursor-agent`. A known tool is `decision: defer` (`autoAllow: false`). An unmapped name denies (exit 2). Neither path prints `{"decision":"allow"}`. A schema dump is not a typed call. How to run: [docs/SPIKE-catalog.md](docs/SPIKE-catalog.md). This index does not set `JEV_PERMISSION_MODE`.

MCP tool intent can also be ranked into a typed call (best and top matches) on that same route-workflow policy. The router prints a `tool_call` payload and does not invoke the tool. This surface defaults to shadow. `JEV_TYPED_CALL_MODE=active` turns it on, and the flake does not set that variable. Until you export it, MCP stays ask. How to run: [docs/SPIKE-calls.md](docs/SPIKE-calls.md).

### Grok Build harness

`grok-build-bezel` is the entrypoint Grok Build calls. It does not keep a second class map or a second TypeSafe client. `hook` reads a PreToolUse event and prints `defer` or `deny`. `config` prints the hook JSON; this repo does not install it. Modes stay shadow until you export `active`. `JEV_BYPASS` is still only `1` or `true`.

```sh
node packages/jev-router/harness.mjs smoke
```

How to run: [docs/GROK-BUILD.md](docs/GROK-BUILD.md).

## Skills

Skills are a flake input, and that input is the only skills tree Bezel installs. This repo ships `skills-stub` so the input is a valid skills tree (`*/SKILL.md`) on day one. The stub includes `bootstrap` and a shadow `check` skill (`bezel-jev --check`). Home Manager symlinks that store path to `~/.config/omp/agent/skills`. When you have a skills repo, change `inputs.skills.url` and the lock. Do not copy a second `skills/` tree into this overlay.

## Platforms

Declared systems: `aarch64-darwin`, `x86_64-darwin`, `aarch64-linux`, `x86_64-linux`.

| System | Pinned binary (oh-my-pi v18.2.6) | Availability |
| --- | --- | --- |
| `aarch64-darwin` | `omp-darwin-arm64` | Pin present. No hosted runner in this workflow. |
| `x86_64-darwin` | `omp-darwin-x64` | Pin present. Evaluated against `nixpkgs-26.05-darwin` (nixpkgs 26.11 dropped Intel macOS). No hosted runner in this workflow. |
| `aarch64-linux` | `omp-linux-arm64` | Pin present. No hosted runner in this workflow. |
| `x86_64-linux` | `omp-linux-x64` | Pin present. GitHub Actions evaluates all four systems and builds overlay packages here. |

If a later release drops a platform build, `packages/omp-pin.nix` **throws** for that system. A skipped CI row is a skip with a reason, not a pretend build.

`nix flake check` and `nix build .#bezel .#bezel-jev .#cursor-agent-bezel .#grok-build-bezel` do **not** download the pin. `nix build .#bezel-pinned` does.

## Status

- Overlay packages evaluate on all four declared systems.
- CI on this repo **runs** the `x86_64-linux` job. The other three matrix rows skip (no matching hosted runner).
- `bezel-pinned` is opt-in (180–240 MB download).
- Jev defaults to shadow. `bezel-jev --check` is the shadow diff check. Empty findings are not approval. Loop-stop is shadow-first. Active `stop` and `escalate` do not exec.
- The tool catalog is a small always-on index of the existing policy ids. Full schema is `bezel-jev --schema NAME` and is not an allow. Permission catalogs stay shadow.
- `grok-build-bezel` adapts those controls for Grok Build. Default modes stay shadow. The harness does not install a host hook.
- Skills ship as `skills-stub` until you change the input URL.
- Nix packaging stays. The router stays JavaScript in this change.

## Contributing

Issues and pull requests are welcome. Match CI from a clone:

```sh
nix flake check -L
nix build -L .#bezel .#bezel-jev .#cursor-agent-bezel .#grok-build-bezel
```

Do not commit secrets. `TYPESAFE_API_KEY` and other keys belong in the process environment, not the flake.

## License

MIT. `bezel-pinned` downloads published binaries from [oh-my-pi](https://github.com/can1357/oh-my-pi) v18.2.6. This overlay does not include that project's source. Install a host `omp` binary from [omp.sh](https://omp.sh), or set `OMP_BIN`.

Bezel is not Omarchy and not Omahedron.

- [This repository](https://github.com/VirtualMachinist/bezel)
- [oh-my-pi releases](https://github.com/can1357/oh-my-pi/releases/tag/v18.2.6)
