#!/usr/bin/env bash
# Fail when a harness skill copy differs by a byte or is a symlink.
# No args: this repo. Four args: canonical, claude, grok, and cursor skill roots.

set -euo pipefail

script_dir=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
# shellcheck source=skill-mirror.sh
source "$script_dir/skill-mirror.sh"

if [[ $# -eq 0 ]]; then
  root=$(CDPATH= cd -- "$script_dir/.." && pwd)
  canonical=$root/.agents/skills
  copies=(
    "$root/.claude/skills"
    "$root/.grok/skills"
    "$root/.cursor/skills"
  )
elif [[ $# -eq 4 ]]; then
  canonical=$1
  copies=("$2" "$3" "$4")
else
  printf 'usage: check-skill-copies.sh [canonical claude grok cursor]\n' >&2
  exit 2
fi

repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd)
if [[ -f $repo_root/flake.nix ]]; then
  if [[ -d $repo_root/skills ]]; then
    skill_fail "repo-root skills/ is a second product tree"
  fi
  if [[ -d $repo_root/.omp/skills ]]; then
    skill_fail ".omp/skills shadows the other harness copies"
  fi
  if [[ -d $repo_root/.codex/skills ]]; then
    skill_fail ".codex/skills is not used; Codex reads .agents/skills"
  fi
fi

for dest in "${copies[@]}"; do
  skill_sync_root "$canonical" "$dest" check
done
