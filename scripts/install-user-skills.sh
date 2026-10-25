#!/usr/bin/env bash
# Copy Bezel skills into each harness's user directory as regular files.
#
#   ~/.agents/skills    omp (compatible user path) and Codex
#   ~/.claude/skills    Claude Code personal skills
#   ~/.grok/skills      Grok user skills
#   ~/.cursor/skills    Cursor user skills
#
# A second run does not rewrite a file whose bytes already match, and it
# leaves that file's mode alone. When a managed file already exists with
# different bytes, is a symlink, or is not a regular file, the run prints a
# drift report and writes nothing. --force replaces those bytes and keeps the
# mode the regular file already had. --check reports drift and writes nothing.
# Unrelated skill directories already in those folders are left in place. A
# symlinked skills directory is replaced so the copy is not written through
# the link.

set -euo pipefail

script_dir=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
# shellcheck source=skill-mirror.sh
source "$script_dir/skill-mirror.sh"

usage() {
  printf 'usage: install-user-skills.sh [--check | --force] SOURCE_DIR HOME\n' >&2
}

mode=apply
force=0
while [[ $# -gt 0 ]]; do
  case $1 in
    --check)
      mode=check
      shift
      ;;
    --force)
      force=1
      shift
      ;;
    --)
      shift
      break
      ;;
    -*)
      usage
      exit 2
      ;;
    *)
      break
      ;;
  esac
done

if [[ $mode == check && $force -eq 1 ]]; then
  printf 'install-user-skills: --check and --force cannot be combined\n' >&2
  exit 2
fi

if [[ $# -ne 2 ]]; then
  usage
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

if [[ $mode == apply && $force -eq 0 ]]; then
  conflict=0
  for dest in "${destinations[@]}"; do
    skill_user_conflicts "$source_dir" "$dest" || conflict=1
  done
  if [[ $conflict -eq 1 ]]; then
    printf 'install-user-skills: refusing to overwrite drifted files; re-run with --force\n' >&2
    exit 1
  fi
fi

for dest in "${destinations[@]}"; do
  skill_sync_user_root "$source_dir" "$dest" "$mode"
done
