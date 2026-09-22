<p align="center">
  <strong>omapi-overlay</strong> — a Nix flake overlay: stock <strong>omp</strong>, thin <code>omapi</code> wrap, skills input, optional Jev gates.
</p>

<p align="center">
  <a href="https://github.com/VirtualMachinist/omapi-overlay/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/VirtualMachinist/omapi-overlay/ci.yml?branch=main&style=flat&colorA=222222&colorB=3FB950" alt="CI"></a>
  <a href="#credits-and-license"><img src="https://img.shields.io/badge/License-MIT-58A6FF?style=flat&colorA=222222" alt="MIT license"></a>
  <a href="https://nixos.org"><img src="https://img.shields.io/badge/Nix-5277C3?style=flat&colorA=222222&logo=nixos&logoColor=white" alt="Nix"></a>
  <a href="https://www.typescriptlang.org"><img src="https://img.shields.io/badge/TypeScript-3178C6?style=flat&colorA=222222&logo=typescript&logoColor=white" alt="TypeScript"></a>
  <a href="https://nodejs.org"><img src="https://img.shields.io/badge/Node-339933?style=flat&colorA=222222&logo=nodedotjs&logoColor=white" alt="Node"></a>
  <a href="https://github.com/can1357/oh-my-pi/releases/tag/v18.2.6"><img src="https://img.shields.io/badge/omp-v18.2.6-8A2BE2?style=flat&colorA=222222" alt="omp v18.2.6 pin"></a>
</p>

<p align="center">
  <a href="#what-it-is">What it is</a> ·
  <a href="#quick-start">Quick start</a> ·
  <a href="#packages">Packages</a> ·
  <a href="#jev">Jev</a> ·
  <a href="#skills">Skills</a> ·
  <a href="#platforms">Platforms</a> ·
  <a href="#status">Status</a> ·
  <a href="#contributing">Contributing</a>
</p>

<p align="center">
  Wraps <a href="https://github.com/can1357/oh-my-pi">oh-my-pi</a> (omp), itself a fork of <a href="https://github.com/badlogic/pi-mono">Pi</a> by <a href="https://github.com/mariozechner">@mariozechner</a>.
</p>

---

This flake overlays stock [omp](https://github.com/can1357/oh-my-pi) with a thin **`omapi`** command, a skills flake input, and optional Jev gates for `cursor-agent`. Humans and widgets call `omapi`. There is no omp source tree in this repository.

## What it is

| | This overlay |
| --- | --- |
| Command you call | `omapi` |
| Engine | Stock omp — host `omp` / `$OMP_BIN`, or the documented release pin |
| Skills | Flake `inputs.skills` only |
| Jev | Optional `jev-router` + `cursor-agent-jev` (shadow by default) |
| Consume via | `overlays.default`, `homeManagerModules.default`, or `packages.<system>.*` |

### What it is not

| Claim | Reality |
| --- | --- |
| An omp source fork | No omp sources here. Features come from upstream omp. |
| A rebuilt `omp` | The wrap is `omapi`. Install omp from [omp.sh](https://omp.sh) or use `omapi-pinned`. |
| A skills product repo | Skills are a flake input. This tree ships a bootstrap stub only. |
| A Jev lab or key store | The router is Choice-only. Keys stay in the process environment. |

## Quick start

From a clone:

```sh
nix build .#omapi
nix build .#jev-router
nix build .#cursor-agent-jev
nix build .#grok-build-jev
```

Default `omapi` wraps a host `omp` (or `$OMP_BIN`). If neither is present, it exits **127** and tells you so.

To put the pinned upstream binary in the Nix store (~180–240 MB):

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

`programs.omapi.enable` installs `omapi` and, by default, `jev-router` and `cursor-agent-jev`. It also symlinks the skills input to `~/.config/omp/agent/skills`.

## Packages

| Package | What it is |
| --- | --- |
| **`omapi`** | Thin wrap of host `omp` / `$OMP_BIN`. Fail-closed if neither exists. |
| **`omapi-pinned`** | Same wrap, targeting a fetchurl pin of [oh-my-pi v18.2.6](https://github.com/can1357/oh-my-pi/releases/tag/v18.2.6). Not omp source. |
| **`jev-router`** | Node router: asks Typesafe Choice, prints a JSON verdict. Loop-stop (`continue` / `stop` / `escalate`), `jev-router --check` for a shadow diff check, shadow permission catalogs, and a tiny tool catalog (`--catalog`, `--schema NAME`). |
| **`cursor-agent-jev`** | Runs `jev-router`, then execs `cursor-agent` (`cursor-agent` must already be on `PATH`). Shadow never blocks; active honors continue/auto only. `--catalog` and `--schema` print and do not exec. |
| **`grok-build-jev`** | Grok Build PreToolUse adapter. Calls `jev-router` and prints `defer` or `deny`. Does not install a host hook. |

## Jev

Jev is a **shadow-first** decision gate in front of `cursor-agent`. Shadow logs a Choice and **never blocks**. `TYPESAFE_API_KEY` is runtime-only — set it in the process environment; the flake does not bake it.

```sh
# TYPESAFE_API_KEY must already be in the environment
JEV_MODE=shadow cursor-agent-jev "<intent>" -- <cursor-agent args>
```

| Variable | Role |
| --- | --- |
| `JEV_MODE=shadow` | Default. Log the Choice; always run `cursor-agent`. |
| `JEV_MODE=active` | Honor `continue` / `gate: auto` only. `stop` / `escalate` exit 2 (no exec). |
| `JEV_BYPASS=1` or `true` | Kill switch. Same predicate everywhere: skip the router and run `cursor-agent`. Other values (`yes`, `false`, `0`) are not a bypass. |
| `JEV_ROUTER` | Override the router binary. |
| `JEV_STEP_DIGEST` | Optional step / trajectory digest for the loop-stop Choice. |
| `TYPESAFE_API_KEY` | Runtime only. [`@typesafe-ai/sdk`](https://www.npmjs.com/package/@typesafe-ai/sdk) reads it. |

Missing key: shadow continues unclassified; active fail-closes (escalate / hold).

Loop-stop Choice is `{continue, stop, escalate}` over intent + optional digest.
Thresholds: `omapi-loop-stop-policy@1` (`conf ≥ 0.6`, `p ≥ 0.55`, `margin ≥ 0.15`).
`choice: escalate` + `gate: hold` is HITL — not auto-retry. How to run:
[docs/SPIKE-loop-stop.md](docs/SPIKE-loop-stop.md).

### Shadow check

`jev-router --check` reads git-diff evidence, applies an available-gate, asks a Choice, then applies code thresholds. The result is `findings`, `parked`, and `notChecked`. Empty findings are **not** approval.

```sh
JEV_MODE=shadow jev-router --check --intent "<task>"
# or: --diff-file packages/jev-router/testdata/skip-marker.diff
```

Skill: `skills-stub/check`. How to run: [docs/SPIKE-stanley.md](docs/SPIKE-stanley.md).

Grok Build `PreToolUse` uses the same three policy ids and the same `JEV_BYPASS` predicate. Tool class → `policyId` map, matcher (shell / write / MCP included), and the castle hook checklist: [docs/POLICY-MAP.md](docs/POLICY-MAP.md). The hook calls `jev-router`; it is not a second TypeSafe client. Default mode stays shadow.

Shell, write, and MCP also get a permission catalog that wraps those same policy ids (`allow`→continue, `deny`→stop, `ask`→escalate or the writer parent). It defaults to shadow. `JEV_PERMISSION_MODE=active` honors it only after Marci re-COMPATs; `JEV_MODE=active` does not flip it. A missing `TYPESAFE_API_KEY` keeps that surface on shadow. How to run: [docs/SPIKE-permission.md](docs/SPIKE-permission.md).

### Tool catalog

Every GateVerdict carries a tiny tool index (`catalog.kind = tiny`): class, policy id, and tool names. It does not carry JSON Schema. The wrap copies that index into `JEV_TOOL_CATALOG` for the `cursor-agent` process.

```sh
jev-router --catalog
cursor-agent-jev --schema Bash
```

`--schema` dumps one tool's schema and does not exec `cursor-agent`. A known tool is `decision: defer` (`autoAllow: false`). An unmapped name denies (exit 2). Neither path prints `{"decision":"allow"}`. A schema dump is not a typed call. How to run: [docs/SPIKE-catalog.md](docs/SPIKE-catalog.md). This index does not set `JEV_PERMISSION_MODE`.

MCP tool intent can also be ranked into a typed call (best and top-X) on that same route-workflow id. The call leaves the router as a FACET `tool_call`. The router does not invoke the tool. This surface defaults to shadow. `JEV_TYPED_CALL_MODE=active` is the flip, and it is not set in the flake. Until that export, MCP stays ask. How to run: [docs/SPIKE-calls.md](docs/SPIKE-calls.md).

### Grok Build harness

`grok-build-jev` is the entrypoint Grok Build calls. It does not keep a second class map or a second TypeSafe client. `hook` reads a PreToolUse event and prints `defer` or `deny`. `config` prints the hook JSON; this repo does not install it. Modes stay shadow until you export `active`. `JEV_BYPASS` is still only `1` or `true`.

```sh
node packages/jev-router/harness.mjs smoke
```

How to run: [docs/GROK-BUILD.md](docs/GROK-BUILD.md).

## Skills

Skills are a flake input — the sole source of truth. This repo ships `skills-stub` so the input is a valid omp skills tree (`*/SKILL.md`) on day one. The stub includes `bootstrap` and a shadow `check` skill (`jev-router --check`). Home Manager only symlinks that store path to `~/.config/omp/agent/skills`. When you have a real skills repo, change `inputs.skills.url` and the lock; do not copy a second `skills/` tree into this overlay.

## Platforms

Declared systems: `aarch64-darwin`, `x86_64-darwin`, `aarch64-linux`, `x86_64-linux`.

| System | omp pin (v18.2.6) | Availability |
| --- | --- | --- |
| `aarch64-darwin` | `omp-darwin-arm64` | Pin present. No hosted runner in this workflow. |
| `x86_64-darwin` | `omp-darwin-x64` | Pin present. Evaluated against `nixpkgs-26.05-darwin` (nixpkgs 26.11 dropped Intel macOS). No hosted runner in this workflow. |
| `aarch64-linux` | `omp-linux-arm64` | Pin present. No hosted runner in this workflow. |
| `x86_64-linux` | `omp-linux-x64` | Pin present. GitHub Actions evaluates all four systems and builds overlay packages here. |

If a future omp release drops a slice, `packages/omp-pin.nix` **throws** for that system. A skipped CI row is a skip with a reason, not a pretend build.

`nix flake check` and `nix build .#omapi .#jev-router .#cursor-agent-jev` do **not** download the pin. `nix build .#omapi-pinned` does.

## Status

- Overlay packages evaluate on all four declared systems.
- CI on this repo **runs** the `x86_64-linux` job; the other three matrix rows skip (no matching hosted runner).
- `omapi-pinned` is opt-in (180–240 MB fetchurl).
- Jev defaults to shadow. `jev-router --check` is the shadow diff check; empty findings are not approval. Loop-stop is shadow-first; active `stop`/`escalate` do not exec.
- Tool catalog is a tiny always-on index of the existing policy ids. Full schema is `jev-router --schema NAME` and is not an allow. Permission catalogs stay shadow.
- `grok-build-jev` adapts those controls for Grok Build. Default modes stay shadow. The harness does not install a host hook.
- Skills ship as `skills-stub` until you swap the input URL.

## Contributing

Issues and pull requests are welcome. Match CI from a clone:

```sh
nix flake check -L
nix build -L .#omapi .#jev-router .#cursor-agent-jev .#grok-build-jev
```

Do not commit secrets. `TYPESAFE_API_KEY` and other keys belong in the process environment, not the flake.

## Credits and license

MIT. This overlay wraps stock [oh-my-pi](https://github.com/can1357/oh-my-pi) (omp). omp is a fork of [Pi](https://github.com/badlogic/pi-mono) by [Mario Zechner](https://github.com/mariozechner).

- [GitHub](https://github.com/VirtualMachinist/omapi-overlay)
- [Upstream omp](https://github.com/can1357/oh-my-pi)
- [omp.sh](https://omp.sh)
