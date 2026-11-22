#!/usr/bin/env bash
# Register the regular-file ~/.agents/skills tree with Antigravity (agy).
#
# Usage: install-agy-skills-config.sh HOME
#
# Creates HOME/.gemini/config/skills.json when absent, adds
# {"path":"HOME/.agents/skills"} to entries[] when not already registered, and
# preserves every unrelated entry and top-level key. The file is never
# rewritten when the path is already registered (bytes and mtime stay).
# Refuses with exit 1, writing nothing, when HOME/.gemini, its config
# directory, or skills.json is a symlink or not a directory/regular file, or
# when skills.json is malformed JSON, a non-object, or has non-list entries.
# PYTHON overrides the interpreter (Home Manager passes pkgs.python3).
set -euo pipefail
python=${PYTHON:-python3}
if [[ $# -ne 1 ]]; then
  printf 'usage: install-agy-skills-config.sh HOME\n' >&2
  exit 2
fi
exec "$python" -B - "$1" <<'PY'
import json
import os
import stat
import sys
import tempfile

prefix = "install-agy-skills-config"


def refuse(message):
    print(f"{prefix}: {message}", file=sys.stderr)
    raise SystemExit(1)


if len(sys.argv) != 2:
    refuse("expected one argument: HOME")
home = os.path.abspath(os.path.expanduser(sys.argv[1]))
if not os.path.isdir(home) or os.path.islink(home):
    refuse(f"home directory is not a real directory: {home}")

gemini = os.path.join(home, ".gemini")
config_dir = os.path.join(gemini, "config")
target = os.path.join(config_dir, "skills.json")
desired = os.path.normpath(os.path.join(home, ".agents", "skills"))

for path, want_dir in ((gemini, True), (config_dir, True)):
    if os.path.islink(path):
        refuse(f"refusing to follow a symlink: {path}")
    if os.path.exists(path) and not os.path.isdir(path):
        refuse(f"not a directory: {path}")

if os.path.islink(target):
    refuse(f"refusing to follow a symlink: {target}")
if os.path.exists(target) and not os.path.isfile(target):
    refuse(f"not a regular file: {target}")


def atomic_write(payload):
    mode = None
    if os.path.exists(target):
        mode = stat.S_IMODE(os.stat(target).st_mode)
    fd, tmp = tempfile.mkstemp(prefix=".skills.json.", suffix=".tmp", dir=config_dir)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
        os.chmod(tmp, 0o644 if mode is None else mode)
        os.replace(tmp, target)
    except BaseException:
        try:
            os.unlink(tmp)
        except FileNotFoundError:
            pass
        raise


if not os.path.exists(target):
    os.makedirs(config_dir, exist_ok=True)
    atomic_write(json.dumps({"entries": [{"path": desired}]}, indent=2) + "\n")
    raise SystemExit(0)

try:
    with open(target, encoding="utf-8") as handle:
        document = json.load(handle)
except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
    refuse(f"cannot parse {target}: {error}")
if not isinstance(document, dict):
    refuse(f"{target} is not a JSON object")
if "entries" not in document:
    entries = []
else:
    entries = document["entries"]
    if not isinstance(entries, list):
        refuse(f"{target} entries is not a list")


def registered(entry):
    if isinstance(entry, str):
        return os.path.normpath(entry) == desired
    if isinstance(entry, dict):
        return os.path.normpath(str(entry.get("path", ""))) == desired
    return False


if any(registered(entry) for entry in entries):
    raise SystemExit(0)

document["entries"] = entries + [{"path": desired}]
atomic_write(json.dumps(document, indent=2) + "\n")
PY
