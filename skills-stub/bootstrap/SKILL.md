---
name: bootstrap
description: Placeholder skill shipped with omapi-overlay skills-stub until omahedron-skills is published. Replace the flake skills input; do not copy a second skills tree into this overlay repo.
---

# bootstrap

This skill exists so the skills flake input is a valid skills directory
(`*/SKILL.md`) while `omahedron-skills` is unpublished.

Home Manager installs the input at `~/.config/omp/agent/skills`, the directory
the wrapped harness loads.
