#!/usr/bin/env bash
# Install the skills into a temp home twice and require the second run to
# leave bytes and mtimes alone. --check must fail on a byte change or a symlink
# without repairing it, and the installer must not write through a symlinked
# destination.

set -euo pipefail

script_dir=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
install=$script_dir/install-user-skills.sh

if [[ $# -ne 1 ]]; then
  printf 'usage: check-user-skills.sh CANONICAL\n' >&2
  exit 2
fi

canonical=$1
if [[ ! -d $canonical/lapis || ! -f $canonical/lapis/lapis.nu ]]; then
  printf 'check-user-skills: %s is not the Bezel skills tree\n' "$canonical" >&2
  exit 1
fi

fail() {
  printf 'check-user-skills: %s\n' "$*" >&2
  exit 1
}

mtime() {
  python3 -c 'import os, sys; print(os.stat(sys.argv[1]).st_mtime_ns)' "$1"
}

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
home=$work/home
mkdir -p "$home"

# A skill the installer does not own stays put.
mkdir -p "$home/.claude/skills/other"
printf 'other\n' >"$home/.claude/skills/other/SKILL.md"

bash "$install" "$canonical" "$home"

for dest in .agents/skills .claude/skills .grok/skills .cursor/skills; do
  [[ -d $home/$dest/lapis ]] || fail "missing $dest/lapis"
  [[ ! -L $home/$dest ]] || fail "$dest is a symlink"
  [[ ! -L $home/$dest/lapis ]] || fail "$dest/lapis is a symlink"
  [[ ! -L $home/$dest/lapis/lapis.nu ]] || fail "$dest/lapis/lapis.nu is a symlink"
  cmp -s "$canonical/lapis/lapis.nu" "$home/$dest/lapis/lapis.nu" || fail "$dest lapis.nu bytes differ"
  cmp -s "$canonical/lapis/SKILL.md" "$home/$dest/lapis/SKILL.md" || fail "$dest SKILL.md bytes differ"
done

[[ -f $home/.claude/skills/other/SKILL.md ]] || fail "unrelated skill was removed"
cmp -s <(printf 'other\n') "$home/.claude/skills/other/SKILL.md" || fail "unrelated skill changed"

if find "$home/.agents" "$home/.claude" "$home/.grok" "$home/.cursor" -type l | grep -q .; then
  fail "install left a symlink"
fi

stamp=$home/.cursor/skills/lapis/lapis.nu
touch -t 200001010000 "$stamp"
before=$(mtime "$stamp")
bash "$install" "$canonical" "$home"
after=$(mtime "$stamp")
[[ $before == "$after" ]] || fail "second install rewrote a matching file"
bash "$install" --check "$canonical" "$home"

printf 'extra\n' >"$home/.agents/skills/lapis/extra.txt"
if bash "$install" --check "$canonical" "$home"; then
  fail "--check passed with an extra file"
fi
[[ -f $home/.agents/skills/lapis/extra.txt ]] || fail "--check removed the extra file"
bash "$install" "$canonical" "$home"
[[ ! -e $home/.agents/skills/lapis/extra.txt ]] || fail "install left an extra file"
[[ -f $home/.claude/skills/other/SKILL.md ]] || fail "repair removed an unrelated skill"

# Drift check reports and does not repair.
printf 'drift\n' >>"$home/.grok/skills/lapis/SKILL.md"
if bash "$install" --check "$canonical" "$home"; then
  fail "--check passed after a byte change"
fi
grep -q drift "$home/.grok/skills/lapis/SKILL.md" || fail "--check rewrote the drifted file"

# The four copies are independent files.
cmp -s "$home/.agents/skills/lapis/SKILL.md" "$home/.grok/skills/lapis/SKILL.md" && fail "harness copies were the same file after one drifted"

bash "$install" "$canonical" "$home"
bash "$install" --check "$canonical" "$home"
cmp -s "$canonical/lapis/SKILL.md" "$home/.grok/skills/lapis/SKILL.md" || fail "install did not repair drift"

# A symlink, even to matching bytes, is drift, and check does not replace it.
rm -f "$home/.claude/skills/facet/SKILL.md"
ln -s "$canonical/facet/SKILL.md" "$home/.claude/skills/facet/SKILL.md"
if bash "$install" --check "$canonical" "$home"; then
  fail "--check passed while a skill file was a symlink"
fi
[[ -L $home/.claude/skills/facet/SKILL.md ]] || fail "--check replaced a symlink"

# Replacing a symlinked skills directory must not write through the link.
holder=$work/holder
mkdir -p "$holder"
rm -rf "$home/.cursor/skills"
ln -s "$holder" "$home/.cursor/skills"
bash "$install" "$canonical" "$home"
[[ ! -L $home/.cursor/skills ]] || fail "install left the skills directory as a symlink"
[[ -d $home/.cursor/skills/lapis ]] || fail "install did not create a real cursor skills tree"
[[ ! -e $holder/lapis ]] || fail "install wrote through the skills symlink"
bash "$install" --check "$canonical" "$home"
