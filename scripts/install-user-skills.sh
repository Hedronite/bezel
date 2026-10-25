#!/usr/bin/env bash
# Copy Bezel skills into each harness's user directory as regular files.
#
#   ~/.agents/skills    omp (compatible user path) and Codex
#   ~/.claude/skills    Claude Code personal skills
#   ~/.grok/skills      Grok user skills
#   ~/.cursor/skills    Cursor user skills
#
# A second run does not rewrite a file whose bytes already match.
# --check reports drift and writes nothing. Unrelated skill directories
# already in those folders are left in place. A symlinked skills directory
# is replaced so the copy is not written through the link.

set -euo pipefail

script_dir=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
# shellcheck source=skill-mirror.sh
source "$script_dir/skill-mirror.sh"

mode=apply
if [[ ${1:-} == --check ]]; then
  mode=check
  shift
fi

if [[ $# -ne 2 ]]; then
  printf 'usage: install-user-skills.sh [--check] SOURCE_DIR HOME\n' >&2
  exit 2
fi

source_dir=$1
home=$2

if [[ -L $source_dir || ! -d $source_dir ]]; then
  skill_fail "skills source is not a real directory: $source_dir"
fi
if [[ -L $home || ! -d $home ]]; then
  skill_fail "home is not a real directory: $home"
fi

destinations=(
  "$home/.agents/skills"
  "$home/.claude/skills"
  "$home/.grok/skills"
  "$home/.cursor/skills"
)

for dest in "${destinations[@]}"; do
  skill_sync_user_root "$source_dir" "$dest" "$mode"
done
