---
name: bootstrap
description: Placeholder skill shipped with the Bezel skills tree until a skills repository is published. Replace the flake skills input. Harness copies are generated from .agents/skills.
---

# bootstrap

This skill exists so the skills flake input is a valid skills directory
(`*/SKILL.md`) until you point the flake skills input at a skills repository.

Home Manager copies each skill directory as regular files into the user
directories the harnesses read: `~/.agents/skills`, `~/.claude/skills`,
`~/.grok/skills`, and `~/.cursor/skills`.
