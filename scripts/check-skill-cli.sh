#!/usr/bin/env bash
# Exercise the Lapis, Facet, and Geode skill bodies against fake CLIs.
# The fakes record argv. Refused commands must not reach them.
set -euo pipefail

script_dir=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
root=$(CDPATH= cd -- "$script_dir/.." && pwd)
skills=${1:-$root/.agents/skills}
nu=${NU:-nu}

fail() {
  printf 'check-skill-cli: %s\n' "$*" >&2
  exit 1
}

if [[ -f "$root/flake.nix" && -d "$root/skills" ]]; then
  fail "repo-root skills/ is a second product tree; ship skills through .agents/skills/"
fi

command -v "$nu" >/dev/null || fail "nushell (nu) is not on PATH"
command -v python3 >/dev/null || fail "python3 is not on PATH"

for f in \
  lapis/SKILL.md lapis/lapis.nu \
  facet/SKILL.md facet/facet.nu \
  geode/SKILL.md geode/geode.nu
do
  [[ -f "$skills/$f" ]] || fail "missing $skills/$f"
done

for skill in lapis facet geode; do
  grep -q '{ ok, command, exit, data, error }' "$skills/$skill/SKILL.md" \
    || fail "$skill SKILL.md does not document the shared envelope"
  grep -q "nu $skill.nu" "$skills/$skill/SKILL.md" \
    || fail "$skill SKILL.md does not call $skill.nu"
done

if grep -nE '/(Users|home)/|localhost|@[A-Za-z0-9._-]+\.(com|dev|net)' \
  "$skills/lapis/SKILL.md" "$skills/lapis/lapis.nu" \
  "$skills/facet/SKILL.md" "$skills/facet/facet.nu" \
  "$skills/geode/SKILL.md" "$skills/geode/geode.nu"
then
  fail "skill files contain a private-looking path, host, or address"
fi

for word in \
  '"list"' '"verify"' '"vault recipients"' '"snapshot ls"' '"policy show"' \
  '"policy check"' '"audit verify"' '"custody status"' '"keyring list"'
do
  grep -q "$word" "$skills/geode/geode.nu" || fail "geode allowlist missing $word"
done

[[ "$(grep -c '\^lapis' "$skills/lapis/lapis.nu")" -eq 1 ]] || fail "lapis.nu should call lapis once"
[[ "$(grep -c '\^facet' "$skills/facet/facet.nu")" -eq 1 ]] || fail "facet.nu should call facet once"
[[ "$(grep -c '\^geode' "$skills/geode/geode.nu")" -eq 1 ]] || fail "geode.nu should call geode once"
grep -q -- '--bodies' "$skills/facet/facet.nu" || fail "facet.nu does not mention --bodies"
grep -q 'request run' "$skills/facet/facet.nu" || fail "facet.nu does not mention request run"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
export HOME="$work/home"
export XDG_CONFIG_HOME="$work/xdg"
mkdir -p "$HOME" "$XDG_CONFIG_HOME" "$work/bin"
export FAKE_LOG="$work/log"
: >"$FAKE_LOG"

cat >"$work/bin/fake-tool.py" <<'PY'
#!/usr/bin/env python3
import os
import sys

name = os.environ.get("FAKE_TOOL") or os.path.basename(sys.argv[0])
args = sys.argv[1:]
log = os.environ.get("FAKE_LOG", "")
if log:
    with open(log, "a", encoding="utf-8") as fh:
        fh.write(name + "\t" + "\t".join(args) + "\n")

if name == "lapis":
    body = (
        '{"data":{"hits":[{"path":"notes/a.md","title":"Alpha","body":"do-not-leak-body"}]}}'
    )
    if "list" in args:
        body = '{"data":[{"path":"projects/a.md","title":"A"},{"path":"notes/b.md","title":"B"}]}'
    elif "analytics" in args:
        # Lapis 0.4.2: domain and priority sit before mtime_iso. A positional
        # read would report the domain as mtime.
        body = (
            '{"data":{"rows":['
            '{"path":"notes/a.md","title":"Alpha","domain":"notes","priority":1,"mtime":111,"mtime_iso":"2024-01-02T00:00:00Z"},'
            '{"path":"notes/b.md","title":"Beta","domain":"notes","priority":2,"mtime":222,"mtime_iso":"2024-01-01T00:00:00Z"}'
            ']}}'
        )
    elif "read" in args:
        body = '{"data":{"hal":{"title":"Alpha"},"body":"hello"}}'
    elif "vault" in args:
        body = '{"data":{"root":"vault"}}'
    elif "doctor" in args:
        body = '{"data":{"index":"up"}}'
elif name == "facet":
    if args[:2] == ["env", "list"]:
        body = '{"variables":[{"name":"TOKEN","value":"super-secret-value","note":"kept"}],"API_TOKEN":"super-secret-value"}'
    elif args[:1] == ["diff"]:
        body = '{"left":"aaa","right":"bbb"}'
    else:
        body = '{"entries":[{"id":"1","status":200}]}'
elif name == "geode":
    body = '{"entries":[{"name":"alpha","size":1}]}'
else:
    body = "{}"

if "FAKE_STDOUT" in os.environ:
    body = os.environ["FAKE_STDOUT"]
sys.stdout.write(body)
if body and not body.endswith("\n"):
    sys.stdout.write("\n")
sys.stderr.write(os.environ.get("FAKE_STDERR", ""))
raise SystemExit(int(os.environ.get("FAKE_EXIT", "0")))
PY
chmod +x "$work/bin/fake-tool.py"
# /bin/sh wrappers so the fakes run in a Nix sandbox without /usr/bin/env.
for tool in lapis facet geode; do
  cat >"$work/bin/$tool" <<EOF
#!/bin/sh
export FAKE_TOOL=$tool
exec python3 "$work/bin/fake-tool.py" "\$@"
EOF
  chmod +x "$work/bin/$tool"
done
export PATH="$work/bin:${PATH}"

clear_fake() {
  unset FAKE_EXIT FAKE_STDOUT FAKE_STDERR || true
  : >"$FAKE_LOG"
}

run() {
  local out=$1
  shift
  : >"$FAKE_LOG"
  set +e
  "$nu" "$@" >"$out" 2>"$out.err"
  local rc=$?
  set -e
  if [[ $rc -ne 0 ]]; then
    printf 'command failed (%s):' "$rc" >&2
    printf ' %q' "$@" >&2
    printf '\n' >&2
    cat "$out.err" >&2 || true
    cat "$out" >&2 || true
    exit 1
  fi
  if [[ -s "$out.err" ]]; then
    echo "stderr not empty for: $*" >&2
    cat "$out.err" >&2
    exit 1
  fi
  python3 - "$out" <<'PY'
import json, sys
text = open(sys.argv[1], encoding="utf-8").read().strip()
data = json.loads(text)
keys = sorted(data)
if keys != ["command", "data", "error", "exit", "ok"]:
    raise SystemExit(f"envelope keys {keys}: {text}")
if not isinstance(data["ok"], bool):
    raise SystemExit("ok is not a bool")
if not isinstance(data["command"], str):
    raise SystemExit("command is not a string")
if isinstance(data["exit"], bool) or not isinstance(data["exit"], int):
    raise SystemExit("exit is not an int")
if data["ok"]:
    if data["error"] is not None:
        raise SystemExit("ok row has error")
else:
    if data["data"] is not None:
        raise SystemExit("error row has data")
    if not isinstance(data["error"], str) or not data["error"]:
        raise SystemExit("error row missing error")
PY
}

expect() {
  python3 - "$1" "$2" "$3" "$4" <<'PY'
import json, sys
data = json.loads(open(sys.argv[1], encoding="utf-8").read())
ok = sys.argv[2] == "true"
command, code = sys.argv[3], int(sys.argv[4])
if data["ok"] != ok or data["command"] != command or data["exit"] != code:
    raise SystemExit(
        f"want ok={ok} command={command} exit={code} got {data}"
    )
PY
}

log_has() {
  grep -F -- "$1" "$FAKE_LOG" >/dev/null || fail "log missing [$1]: $(cat "$FAKE_LOG")"
}

log_lacks() {
  if grep -F -- "$1" "$FAKE_LOG" >/dev/null; then
    fail "log should not contain [$1]: $(cat "$FAKE_LOG")"
  fi
}

log_empty() {
  if [[ -s "$FAKE_LOG" ]]; then
    fail "expected no CLI call, log was: $(cat "$FAKE_LOG")"
  fi
}

unset LAPIS_VAULT || true
unset GEODE_VAULT || true
clear_fake

# Lapis: read-only envelope, search drops bodies, vault flag order.
run "$work/out" "$skills/lapis/lapis.nu" search "phrase" --limit 5 --domain notes
expect "$work/out" true search 0
python3 - "$work/out" <<'PY'
import json, sys
data = json.loads(open(sys.argv[1], encoding="utf-8").read())
hits = data["data"]
if hits != [{"path": "notes/a.md", "title": "Alpha"}]:
    raise SystemExit(hits)
PY
log_has $'lapis\t--json\tsearch\tphrase\t-n\t5\t--domain\tnotes'
log_lacks "--vault"
log_lacks "do-not-leak-body"
if grep -q 'do-not-leak-body' "$work/out"; then
  fail "search leaked a hit body"
fi

export LAPIS_VAULT="$work/from-env"
run "$work/out" "$skills/lapis/lapis.nu" status --vault "$work/from-flag"
expect "$work/out" true status 0
log_has $'--vault\t'"$work/from-flag"
log_lacks "$work/from-env"
unset LAPIS_VAULT

run "$work/out" "$skills/lapis/lapis.nu" list --prefix projects/
expect "$work/out" true list 0
python3 - "$work/out" <<'PY'
import json, sys
data = json.loads(open(sys.argv[1], encoding="utf-8").read())
paths = [row["path"] for row in data["data"]]
if paths != ["projects/a.md"]:
    raise SystemExit(paths)
PY

run "$work/out" "$skills/lapis/lapis.nu" recent --limit 1
expect "$work/out" true recent 0
python3 - "$work/out" <<'PY'
import json, sys
data = json.loads(open(sys.argv[1], encoding="utf-8").read())
want = [{"path": "notes/a.md", "title": "Alpha", "mtime": "2024-01-02T00:00:00Z"}]
if data["data"] != want:
    raise SystemExit(data["data"])
PY

export FAKE_STDOUT='{"data":{"rows":[{"path":"notes/a.md","title":"Alpha","mtime":1},{"path":"notes/b.md","title":"Beta","mtime":2}]}}'
run "$work/out" "$skills/lapis/lapis.nu" recent --limit 1
expect "$work/out" true recent 0
python3 - "$work/out" <<'PY'
import json, sys
data = json.loads(open(sys.argv[1], encoding="utf-8").read())
if data["data"] != [{"path": "notes/a.md", "title": "Alpha", "mtime": 1}]:
    raise SystemExit(data["data"])
PY
clear_fake

export FAKE_EXIT=2 FAKE_STDERR="index down" FAKE_STDOUT=""
run "$work/out" "$skills/lapis/lapis.nu" doctor
expect "$work/out" false doctor 2
python3 - "$work/out" <<'PY'
import json, sys
err = json.loads(open(sys.argv[1], encoding="utf-8").read())["error"]
if "index down" not in err:
    raise SystemExit(err)
PY
clear_fake

run "$work/out" "$skills/lapis/lapis.nu" create "note"
expect "$work/out" false create 2
log_empty

run "$work/out" "$skills/lapis/lapis.nu"
expect "$work/out" false "" 2
log_empty

# Facet: bodies, blob, and request run never reach the CLI. Env drops values.
run "$work/out" "$skills/facet/facet.nu" history coll --limit 2 --status 200
expect "$work/out" true history 0
log_has $'facet\thistory\tcoll\t--limit\t2\t--status\t200\t--json'
log_lacks "--bodies"
log_lacks $'\t--id'

run "$work/out" "$skills/facet/facet.nu" history --bodies
expect "$work/out" false history 2
log_empty

run "$work/out" "$skills/facet/facet.nu" history --id 9
expect "$work/out" false history 2
log_empty

run "$work/out" "$skills/facet/facet.nu" blob 9
expect "$work/out" false blob 2
log_empty

run "$work/out" "$skills/facet/facet.nu" request run --dry-run
expect "$work/out" false request 2
log_empty

run "$work/out" "$skills/facet/facet.nu" replay 9
expect "$work/out" false replay 2
log_empty

export FAKE_EXIT=1 FAKE_STDOUT='{"left":"aaa","right":"bbb"}'
run "$work/out" "$skills/facet/facet.nu" diff idA idB
expect "$work/out" true diff 1
python3 - "$work/out" <<'PY'
import json, sys
data = json.loads(open(sys.argv[1], encoding="utf-8").read())["data"]
if data.get("same") is not False or data.get("left") != "aaa":
    raise SystemExit(data)
PY
log_has $'facet\tdiff\tidA\tidB\t--json'
log_lacks "--bodies"
clear_fake

run "$work/out" "$skills/facet/facet.nu" diff idA idB --bodies
expect "$work/out" false diff 2
log_empty

run "$work/out" "$skills/facet/facet.nu" env coll --environment dev
expect "$work/out" true env 0
python3 - "$work/out" <<'PY'
import json, sys
raw = open(sys.argv[1], encoding="utf-8").read()
if "super-secret-value" in raw:
    raise SystemExit(raw)
data = json.loads(raw)["data"]
if data["variables"] != [{"name": "TOKEN", "note": "kept"}]:
    raise SystemExit(data["variables"])
if data.get("API_TOKEN") is not None:
    raise SystemExit(data["API_TOKEN"])
if "API_TOKEN" not in data:
    raise SystemExit(data)
PY
log_has $'env\tlist\tcoll\t--environment\tdev\t--json'

run "$work/out" "$skills/facet/facet.nu" env set TOKEN
expect "$work/out" false env 2
log_empty

run "$work/out" "$skills/facet/facet.nu" session start
expect "$work/out" false session 2
log_empty

run "$work/out" "$skills/facet/facet.nu" sessions --open
expect "$work/out" true sessions 0
log_has $'session\tlist\t--open\t--json'

# Geode: metadata allowlist. Missing vault and every other command stay local.
run "$work/out" "$skills/geode/geode.nu" list
expect "$work/out" false list 2
log_empty

export GEODE_VAULT="$work/vault"
run "$work/out" "$skills/geode/geode.nu" list
expect "$work/out" true list 0
log_has $'geode\t--output\tjson\tlist\t'"$work/vault"

run "$work/out" "$skills/geode/geode.nu" verify
expect "$work/out" true verify 0
log_has $'verify\t'"$work/vault"$'\t--cheap'
log_lacks "--sample"

run "$work/out" "$skills/geode/geode.nu" verify --sample 0.05
expect "$work/out" true verify 0
log_has $'--sample\t0.05'
log_lacks "--cheap"

run "$work/out" "$skills/geode/geode.nu" verify --sample -1
expect "$work/out" false verify 2
log_empty

export FAKE_EXIT=3 FAKE_STDOUT='{"allow":false}'
run "$work/out" "$skills/geode/geode.nu" can alice read notes/a
expect "$work/out" true can 3
python3 - "$work/out" <<'PY'
import json, sys
data = json.loads(open(sys.argv[1], encoding="utf-8").read())
if data["data"] != {"allow": False}:
    raise SystemExit(data)
PY
log_has $'policy\tcheck\t'"$work/vault"$'\talice\tread\tnotes/a'
clear_fake
export GEODE_VAULT="$work/vault"

for args in \
  "cat secret" \
  "open secret" \
  "mount secret" \
  "run secret" \
  "agent secret" \
  "keygen" \
  "vault init" \
  "policy set" \
  "custody unlock" \
  "keyring add" \
  "git unlock"
do
  # shellcheck disable=SC2086
  run "$work/out" "$skills/geode/geode.nu" $args
  python3 - "$work/out" <<'PY'
import json, sys
data = json.loads(open(sys.argv[1], encoding="utf-8").read())
if data["ok"] or data["exit"] != 2 or not str(data["error"]).startswith("refused:"):
    raise SystemExit(data)
PY
  log_empty
done

# A missing binary is still the envelope, not a stack trace.
# Drop the fake CLI directory and keep nu, python3, and coreutils.
export PATH="${PATH#"$work/bin:"}"
unset LAPIS_VAULT || true
run "$work/out" "$skills/lapis/lapis.nu" status
expect "$work/out" false status 127
python3 - "$work/out" <<'PY'
import json, sys
err = json.loads(open(sys.argv[1], encoding="utf-8").read())["error"]
if "lapis:" not in err:
    raise SystemExit(err)
if "/home/" in err or "/Users/" in err:
    raise SystemExit(err)
PY

echo "check-skill-cli: ok"
