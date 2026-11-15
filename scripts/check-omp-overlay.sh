#!/usr/bin/env bash
# Check that bezel-omp passes the omp config overlay by default and passes
# none under BEZEL_OMP_OVERLAY=0. A fake omp records its argv; no real omp runs.
# Usage: check-omp-overlay.sh [path/to/bezel-omp]. With no argument it builds
# .#bezel with nix and uses the bezel-omp in the result.
set -euo pipefail

script_dir=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
root=$(CDPATH= cd -- "$script_dir/.." && pwd)

fail() {
  printf 'check-omp-overlay: %s\n' "$*" >&2
  exit 1
}

if [[ $# -gt 1 ]]; then
  printf 'usage: check-omp-overlay.sh [path/to/bezel-omp]\n' >&2
  exit 2
fi

if [[ $# -eq 1 ]]; then
  wrap=$1
else
  command -v nix >/dev/null || fail "nix is not on PATH; pass the path to a built bezel-omp"
  out=$(nix build "$root#bezel" --no-link --print-out-paths)
  wrap=$out/bin/bezel-omp
fi
[[ -x $wrap ]] || fail "not executable: $wrap"

source_overlay=$root/packages/omp-overlay.yml
[[ -f $source_overlay ]] || fail "missing packages/omp-overlay.yml"

# The overlay sets one key and nothing else.
body=$(grep -Ev '^[[:space:]]*(#|$)' "$source_overlay")
want=$(printf 'tools:\n  outputMaxColumns: 8192')
[[ $body == "$want" ]] || fail "packages/omp-overlay.yml must set tools.outputMaxColumns: 8192 and nothing else"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
export HOME=$work/home
mkdir -p "$HOME"

fake=$work/omp
cat >"$fake" <<'SH'
#!/bin/sh
: >"$FAKE_ARGV"
for arg in "$@"; do
  printf '%s\n' "$arg" >>"$FAKE_ARGV"
done
SH
chmod +x "$fake"
export FAKE_ARGV=$work/argv

# The fake's argv, one element per line, into args. Bash 3.2 has no mapfile.
read_args() {
  args=()
  while IFS= read -r line; do
    args+=("$line")
  done <"$FAKE_ARGV"
}

# Run bezel-omp under a clean environment: only the fake and the variables
# the case names reach it.
run() {
  env -i HOME="$HOME" PATH="$PATH" OMP_BIN="$fake" FAKE_ARGV="$FAKE_ARGV" "$@" "$wrap" --no-lsp -p hello
}

# Default: one --config=<overlay> first, then the caller's args in order.
run
read_args
[[ ${#args[@]} -eq 4 ]] || fail "default run: expected 4 args, got ${#args[@]}: ${args[*]}"
case ${args[0]} in
  --config=*/omp-overlay.yml) ;;
  *) fail "default run: first arg is not --config=<overlay>: ${args[0]}" ;;
esac
installed=${args[0]#--config=}
[[ -f $installed ]] || fail "default run: the overlay path does not exist: $installed"
cmp -s "$installed" "$source_overlay" || fail "default run: the installed overlay differs from packages/omp-overlay.yml"
[[ ${args[1]} == --no-lsp && ${args[2]} == -p && ${args[3]} == hello ]] \
  || fail "default run: the caller's args were changed: ${args[*]}"

# BEZEL_OMP_OVERLAY=1 is the default spelled out.
run BEZEL_OMP_OVERLAY=1
read_args
[[ ${args[0]} == --config=*/omp-overlay.yml && ${#args[@]} -eq 4 ]] \
  || fail "BEZEL_OMP_OVERLAY=1: expected the overlay first: ${args[*]}"

# Opt out: no --config at all, the caller's args untouched.
run BEZEL_OMP_OVERLAY=0
read_args
[[ ${#args[@]} -eq 3 ]] || fail "opt-out: expected 3 args, got ${#args[@]}: ${args[*]}"
for arg in "${args[@]}"; do
  [[ $arg != --config* ]] || fail "opt-out: a --config reached omp: $arg"
done
[[ ${args[0]} == --no-lsp && ${args[1]} == -p && ${args[2]} == hello ]] \
  || fail "opt-out: the caller's args were changed: ${args[*]}"

# A caller's own --config comes after the overlay, so it wins.
env -i HOME="$HOME" PATH="$PATH" OMP_BIN="$fake" FAKE_ARGV="$FAKE_ARGV" \
  "$wrap" --config=/caller/own.yml -p hello
read_args
[[ ${args[0]} == --config=*/omp-overlay.yml && ${args[1]} == --config=/caller/own.yml ]] \
  || fail "caller --config: expected the overlay first, then the caller's: ${args[*]}"

# No secret is baked into the wrapper.
if grep -Ei 'API_KEY|TOKEN|SECRET' "$wrap" | grep -E -- '--set(-default)?[ =]' ; then
  fail "the wrapper sets a secret-looking variable"
fi

printf 'check-omp-overlay: ok\n'
