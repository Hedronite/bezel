---
name: bootstrap
description: Placeholder skill shipped with the Bezel skills-stub until a skills repository is published. Replace the flake skills input; do not copy a second skills tree into this repo.
---

# bootstrap

This skill exists so the skills flake input is a valid skills directory
(`*/SKILL.md`) until you point the flake skills input at a skills repository.

Home Manager installs the input at `~/.config/omp/agent/skills`, the directory
the wrapped harness loads.
