#!/bin/sh
# Run bend on the shipped bridle proof, or on a proof path passed as $1.
# A proof that drops one law exits non-zero. This script does not edit the laws.

set -eu
export BEND_NO_TELEMETRY="${BEND_NO_TELEMETRY:-1}"

root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
proof=${1:-"$root/skills-stub/laws/PROOF.bend"}

if [ ! -f "$proof" ]; then
  printf 'bend-gate: no proof at %s\n' "$proof" >&2
  exit 2
fi

for law in \
  shadow_cannot_set_auto_allow \
  code_deny_never_calls_transport \
  empty_findings_are_not_approval \
  state_rejects_secret_key
do
  if ! grep -q "$law" "$proof"; then
    printf 'bend-gate: proof does not name %s\n' "$law" >&2
    exit 1
  fi
done

dir=$(CDPATH= cd -- "$(dirname "$proof")" && pwd)
name=$(basename "$proof")
cd "$dir"
exec bend "$name"
