#!/usr/bin/env bash
# Install the skills into a temp home twice and require the second run to
# leave bytes and mtimes alone. A file whose bytes differ, or a symlinked
# file, makes a plain install exit 1 with a drift report and write nothing.
# --force replaces those bytes and keeps the mode the regular file already
# had. --check must fail on a byte change or a symlink without repairing it,
# and the installer must not write through a symlinked destination.

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

file_mode() {
  if stat -c %a "$1" >/dev/null 2>&1; then
    stat -c %a "$1"
  else
    stat -f %OLp "$1"
  fi
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

# A matching file keeps a mode that is not the normalized 755/644.
live=$home/.agents/skills/lapis/lapis.nu
chmod 700 "$live"
live_stamp=$(mtime "$live")
bash "$install" "$canonical" "$home"
[[ $(file_mode "$live") == 700 ]] || fail "install normalized a matching file's mode"
[[ $(mtime "$live") == "$live_stamp" ]] || fail "install rewrote a mode-only difference"
bash "$install" --check "$canonical" "$home"

printf 'extra\n' >"$home/.agents/skills/lapis/extra.txt"
if bash "$install" --check "$canonical" "$home"; then
  fail "--check passed with an extra file"
fi
[[ -f $home/.agents/skills/lapis/extra.txt ]] || fail "--check removed the extra file"
bash "$install" "$canonical" "$home"
[[ ! -e $home/.agents/skills/lapis/extra.txt ]] || fail "install left an extra file"
[[ -f $home/.claude/skills/other/SKILL.md ]] || fail "repair removed an unrelated skill"

# Drift check reports and does not repair. A plain install refuses.
err=$work/err
printf 'drift\n' >>"$home/.grok/skills/lapis/SKILL.md"
chmod 640 "$home/.grok/skills/lapis/SKILL.md"
printf 'extra\n' >"$home/.agents/skills/lapis/extra-during-drift.txt"
clean=$(mtime "$home/.agents/skills/lapis/lapis.nu")
if bash "$install" --check "$canonical" "$home"; then
  fail "--check passed after a byte change"
fi
grep -q drift "$home/.grok/skills/lapis/SKILL.md" || fail "--check rewrote the drifted file"
status=0
bash "$install" "$canonical" "$home" 2>"$err" || status=$?
[[ $status -eq 1 ]] || fail "install exit on byte drift was $status"
grep -F -q "byte drift: $home/.grok/skills/lapis/SKILL.md" "$err" || fail "drift report missed the file"
grep -F -q "re-run with --force" "$err" || fail "drift report did not require --force"
grep -q drift "$home/.grok/skills/lapis/SKILL.md" || fail "install overwrote drifted bytes"
[[ $(file_mode "$home/.grok/skills/lapis/SKILL.md") == 640 ]] || fail "refusing install changed the file mode"
[[ -f $home/.agents/skills/lapis/extra-during-drift.txt ]] || fail "refusing install removed an extra file"
[[ $(mtime "$home/.agents/skills/lapis/lapis.nu") == "$clean" ]] || fail "refusing install wrote another tree"
[[ $(file_mode "$live") == 700 ]] || fail "refusing install changed lapis.nu's mode"

# The four copies are independent files.
cmp -s "$home/.agents/skills/lapis/SKILL.md" "$home/.grok/skills/lapis/SKILL.md" && fail "harness copies were the same file after one drifted"

status=0
bash "$install" --check --force "$canonical" "$home" 2>"$err" || status=$?
[[ $status -eq 2 ]] || fail "--check --force exit was $status"
grep -q drift "$home/.grok/skills/lapis/SKILL.md" || fail "--check --force rewrote the drifted file"

bash "$install" --force "$canonical" "$home"
cmp -s "$canonical/lapis/SKILL.md" "$home/.grok/skills/lapis/SKILL.md" || fail "--force did not repair drift"
[[ $(file_mode "$home/.grok/skills/lapis/SKILL.md") == 640 ]] || fail "--force changed the file mode"
[[ ! -e $home/.agents/skills/lapis/extra-during-drift.txt ]] || fail "--force left an extra file"
[[ $(file_mode "$live") == 700 ]] || fail "--force changed a matching file's mode"
bash "$install" --check "$canonical" "$home"

# castle: ~/.local/bin/lapisq points at the live lapis.nu. Refuse leaves the
# symlink, the bytes, and the mode. --force updates the bytes only.
printf '\n# castle\n' >>"$live"
mkdir -p "$home/.local/bin"
ln -s "$live" "$home/.local/bin/lapisq"
castle=$(cat "$live")
status=0
bash "$install" "$canonical" "$home" 2>"$err" || status=$?
[[ $status -eq 1 ]] || fail "install exit on live lapis.nu was $status"
grep -F -q "byte drift: $live" "$err" || fail "drift report missed lapis.nu"
[[ $(readlink "$home/.local/bin/lapisq") == "$live" ]] || fail "refusing install changed lapisq"
[[ $(cat "$live") == "$castle" ]] || fail "refusing install changed live lapis.nu"
[[ $(file_mode "$live") == 700 ]] || fail "refusing install changed live lapis.nu mode"
[[ -x $home/.local/bin/lapisq ]] || fail "lapisq was not executable after refuse"
bash "$install" --force "$canonical" "$home"
cmp -s "$canonical/lapis/lapis.nu" "$live" || fail "--force did not update live lapis.nu"
cmp -s "$canonical/lapis/lapis.nu" "$home/.local/bin/lapisq" || fail "lapisq does not read the updated file"
[[ -L $home/.local/bin/lapisq ]] || fail "--force replaced the lapisq symlink"
[[ $(readlink "$home/.local/bin/lapisq") == "$live" ]] || fail "--force retargeted lapisq"
[[ $(file_mode "$live") == 700 ]] || fail "--force changed live lapis.nu mode"
[[ -x $home/.local/bin/lapisq ]] || fail "lapisq was not executable after --force"
bash "$install" --check "$canonical" "$home"

# A symlink, even to matching bytes, is drift. Check and a plain install
# leave it. --force replaces it with a regular file.
rm -f "$home/.claude/skills/facet/SKILL.md"
ln -s "$canonical/facet/SKILL.md" "$home/.claude/skills/facet/SKILL.md"
if bash "$install" --check "$canonical" "$home"; then
  fail "--check passed while a skill file was a symlink"
fi
[[ -L $home/.claude/skills/facet/SKILL.md ]] || fail "--check replaced a symlink"
status=0
bash "$install" "$canonical" "$home" 2>"$err" || status=$?
[[ $status -eq 1 ]] || fail "install exit on a symlinked file was $status"
grep -F -q "symlink: $home/.claude/skills/facet/SKILL.md" "$err" || fail "drift report missed the symlink"
[[ -L $home/.claude/skills/facet/SKILL.md ]] || fail "install replaced a symlink without --force"
bash "$install" --force "$canonical" "$home"
[[ ! -L $home/.claude/skills/facet/SKILL.md ]] || fail "--force left the skill file as a symlink"
cmp -s "$canonical/facet/SKILL.md" "$home/.claude/skills/facet/SKILL.md" || fail "--force did not replace the symlink"
bash "$install" --check "$canonical" "$home"

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

# ---------- Antigravity skills.json helper ----------
# Every behavior G3b promises: create, idempotent no-op, preserve unrelated
# entries and top-level keys, honor a string-form registration, and fail
# closed (exit 1, bytes and mtime untouched) on symlinks or bad JSON.
agy=$script_dir/install-agy-skills-config.sh
[[ -x $agy ]] || fail "missing $agy"
ahome=$work/agy/home
mkdir -p "$ahome"
bash "$agy" "$ahome" || fail "agy helper failed to create skills.json"
acfg=$ahome/.gemini/config/skills.json
[[ -f $acfg && ! -L $acfg ]] || fail "agy skills.json is not a regular file"
python3 - "$acfg" "$ahome/.agents/skills" <<'PY'
import json, sys
doc = json.load(open(sys.argv[1], encoding="utf-8"))
if not isinstance(doc, dict) or "entries" not in doc:
    raise SystemExit(f"bad skills.json: {doc}")
paths = [e for e in doc["entries"] if isinstance(e, dict) and e.get("path") == sys.argv[2]]
if len(paths) != 1 or len(doc["entries"]) != 1:
    raise SystemExit(f"skills.json does not register the tree once: {doc}")
PY

# A second run is a byte- and mtime-identical no-op.
cp "$acfg" "$work/agy/first.json"
agy_mtime=$(mtime "$acfg")
bash "$agy" "$ahome" || fail "agy helper failed on a registered file"
[[ $(mtime "$acfg") == "$agy_mtime" ]] || fail "agy helper rewrote a registered file"
cmp -s "$acfg" "$work/agy/first.json" || fail "agy helper changed bytes on a no-op"

# Unrelated entries and other top-level keys survive an update.
printf '{"version":3,"entries":["plain",{"path":"/opt/other-skills","label":"x"}],"extra":{"keep":true}}\n' >"$acfg"
bash "$agy" "$ahome" || fail "agy helper failed to update a valid file"
python3 - "$acfg" "$ahome/.agents/skills" <<'PY'
import json, sys
doc = json.load(open(sys.argv[1], encoding="utf-8"))
want = ["plain", {"path": "/opt/other-skills", "label": "x"},
        {"path": sys.argv[2]}]
if doc.get("entries") != want:
    raise SystemExit(f"entries not preserved + appended: {doc.get('entries')}")
if doc.get("version") != 3 or doc.get("extra") != {"keep": True}:
    raise SystemExit(f"top-level keys lost: {doc}")
PY

# A string-form entry for the same tree also counts as registered.
stringhome=$work/agy/stringhome
mkdir -p "$stringhome/.gemini/config"
printf '{"entries":["%s/.agents/skills"]}\n' "$stringhome" >"$stringhome/.gemini/config/skills.json"
cp "$stringhome/.gemini/config/skills.json" "$work/agy/string-before.json"
bash "$agy" "$stringhome" || fail "agy helper rejected a string-form registration"
cmp -s "$stringhome/.gemini/config/skills.json" "$work/agy/string-before.json" \
  || fail "agy helper rewrote a string-form registration"

# Tilde, $HOME, and ${HOME} forms resolve against the HOME argument, so an
# entry that names the same tree is recognized instead of duplicated.
for form in '~/.agents/skills' '$HOME/.agents/skills' '${HOME}/.agents/skills'; do
  formhome=$work/agy/formhome
  rm -rf "$formhome"
  mkdir -p "$formhome/.gemini/config"
  printf '{"entries":[{"path":"%s"}]}\n' "$form" >"$formhome/.gemini/config/skills.json"
  cp "$formhome/.gemini/config/skills.json" "$work/agy/form-before.json"
  form_mtime=$(mtime "$formhome/.gemini/config/skills.json")
  bash "$agy" "$formhome" || fail "agy helper rejected the [$form] registration"
  cmp -s "$formhome/.gemini/config/skills.json" "$work/agy/form-before.json" \
    || fail "agy helper rewrote the [$form] registration"
  [[ $(mtime "$formhome/.gemini/config/skills.json") == "$form_mtime" ]] \
    || fail "agy helper touched the [$form] registration"
done

# Refuse a symlinked skills.json without writing through it.
holder=$work/agy/holder
mkdir -p "$holder"
real=$holder/real.json
printf '{"entries":[]}\n' >"$real"
cp "$real" "$work/agy/real-before.json"
real_mtime=$(mtime "$real")
rm -f "$acfg"
ln -s "$real" "$acfg"
status=0
bash "$agy" "$ahome" 2>/dev/null || status=$?
[[ $status -eq 1 ]] || fail "agy exit on symlinked skills.json was $status"
[[ -L $acfg ]] || fail "agy replaced a symlinked skills.json"
cmp -s "$real" "$work/agy/real-before.json" || fail "agy wrote through a symlink"
[[ $(mtime "$real") == "$real_mtime" ]] || fail "agy touched a symlink target"

# Refuse a symlinked config directory.
linkhome=$work/agy/linkhome
mkdir -p "$linkhome/.gemini" "$work/agy/config-target"
ln -s "$work/agy/config-target" "$linkhome/.gemini/config"
status=0
bash "$agy" "$linkhome" 2>/dev/null || status=$?
[[ $status -eq 1 ]] || fail "agy exit on symlinked config dir was $status"
[[ -z "$(ls -A "$work/agy/config-target")" ]] || fail "agy wrote into a symlinked config dir"

# Refuse a symlinked ~/.gemini.
gemhome=$work/agy/gemhome
mkdir -p "$gemhome" "$work/agy/gemini-target"
ln -s "$work/agy/gemini-target" "$gemhome/.gemini"
status=0
bash "$agy" "$gemhome" 2>/dev/null || status=$?
[[ $status -eq 1 ]] || fail "agy exit on symlinked .gemini was $status"
[[ -z "$(ls -A "$work/agy/gemini-target")" ]] || fail "agy wrote through a symlinked .gemini"

# Malformed JSON, non-object JSON, and non-list entries: exit 1, untouched.
badhome=$work/agy/badhome
mkdir -p "$badhome/.gemini/config"
badcfg=$badhome/.gemini/config/skills.json
for payload in '{oops' '[]' '{"entries":"x"}' '{"entries":null}'; do
  printf '%s\n' "$payload" >"$badcfg"
  cp "$badcfg" "$work/agy/bad-before.json"
  bad_mtime=$(mtime "$badcfg")
  status=0
  bash "$agy" "$badhome" 2>/dev/null || status=$?
  [[ $status -eq 1 ]] || fail "agy exit on payload [$payload] was $status"
  cmp -s "$badcfg" "$work/agy/bad-before.json" || fail "agy rewrote bad JSON [$payload]"
  [[ $(mtime "$badcfg") == "$bad_mtime" ]] || fail "agy touched bad JSON [$payload]"
done
