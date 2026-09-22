# SCORE — omapi-overlay identity (Marci → Eli)

Thesis: this repo is a harness-agnostic overlay (skills + Jev gates + tooling), not an omp-fork aesthetic.

Read `origin/main` at `084f4b6`, plus open #9 (splash) and #10 (Grok harness, stacked on #9). No Geode Herdr tree is in this repo; none was touched. Jev gates, POLICY-MAP behavior, permission catalogs, and typed calls were not edited.

Calls: **STEAL-cut** (do it), **WATCH** (true snag, do not cut in this pass), **HOLD** (leave it).

## Already in flight

| Item | Call | Why |
| --- | --- | --- |
| the mark PNG under `assets/` and the README "oma-on-π" alt text | STEAL-cut | Owned by #9 (deletion). Prose PR #12 is stacked on that branch and does not delete the asset again. |
| `grok-build-jev` package and `docs/GROK-BUILD.md` | HOLD | #10. Harness work, not an identity cut. |

## Judgment

| Item | Call | Why |
| --- | --- | --- |
| Centered README line "itself a fork of Pi" | STEAL-cut | Decorative fork aesthetic. Cut in #12. Upstream credit stays in Credits. |
| README omp `v18.2.6` badge | STEAL-cut | Makes the pin the product. Cut in #12. Pin facts stay under Packages and Platforms. |
| README tagline "stock omp, thin omapi wrap, optional Jev" | STEAL-cut | Leads with the omp engine. Cut in #12. |
| README lede "Humans and widgets call `omapi`" / Jev "for `cursor-agent`" | STEAL-cut | omp as the UX, Cursor as the only gate. Cut in #12. |
| "What it is" table (command = `omapi`, engine = stock omp, Jev optional) | STEAL-cut | Same omp-first framing. Cut in #12. |
| First denial row "An omp source fork" | STEAL-cut | Keeps "fork" as the headline boundary. Cut in #12; the fact (no omp sources here) stays. |
| Flake `description` "Not an omp source fork" | STEAL-cut | Public nix one-liner. Cut in #12. No package behavior change. |
| `omapi` / `omp-pin` meta "not a source fork" | STEAL-cut | Same phrase in `nix search` output. Cut in #12. Still a wrap and a fetchurl pin. |
| skills-stub "valid omp skills directory" | STEAL-cut | The input is `*/SKILL.md`. Cut in #12. |
| Home Manager enable blurb "stock omp wrap + …" | STEAL-cut | Module summary only. Cut in #12. Symlink path unchanged. |
| `docs/SPIKE-stanley.md` "Skill (omp / Home Manager…)" | STEAL-cut | Same omp-only skill wording. Cut in #12. Workflow unchanged. |
| README Skills sentence "valid omp skills tree" | WATCH | Same snag as the stub. #10's harness hunk uses that paragraph as context, so #12 left it. One-line follow-up after #10. |
| `programs.omapi.package` description "thin wrap of stock omp" | HOLD | That option is the wrap package. Accurate, not a banner. |
| CI / license / Nix / TypeScript / Node badge row | WATCH | Ordinary repo chrome. It does not name a harness. |
| Credits: "omp is a fork of Pi" | HOLD | License attribution for oh-my-pi and Pi. Not splash. |
| `~/.config/omp/agent/skills` symlink | HOLD | Live omp install path. Moving it is behavior. #12 only says it is the omp harness location. |
| `packages.default = omapi` | HOLD | Changing the default package is behavior. |
| `omahedron-skills` input comment (`flake.nix`, skills-stub) | HOLD | Unpublished skills URL. Not an OS-fork claim. |
| `templates/models.yml.example` "Cursor / omp auth stays on the host" | HOLD | Secret-handling constraint. The writer example is not a harness ranking. |
| `omapi-planes-shim` | HOLD | Pre-exec filter. Not branding. |
| Cline (or any unnamed harness) second-class copy | HOLD | No Cline strings in the tree. Do not invent an adapter. |
| Launch splash inside `omapi` / `cursor-agent-jev` | HOLD | Not present. The mark lived in README and `assets/` only (#9). |
| POLICY-MAP, permission, typed-call, loop-stop docs and code | HOLD | Gate behavior. Out of this pass. |

## Checked, not a snag

No ASCII art. No second skills tree. No omp source vendor. Grok is already a peer of Cursor on the policy-id map in `docs/POLICY-MAP.md`; the omp-first wording was the README lede, not a weaker gate.
