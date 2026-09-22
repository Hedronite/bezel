#!/bin/sh
# Optional PLANES pre-exec filter. Lives in $out/libexec — not compiled into omp.
# The bezel wrapper runs this when BEZEL_PLANES=1 or OMAPI_PLANES=1.
# Env forwarded at runtime (OMP_*, BEZEL_*, OMAPI_*, JEV_*, CURSOR_*, TYPESAFE_API_KEY).
# This script must not print or persist secrets.

set -eu

planes="${BEZEL_PLANES:-${OMAPI_PLANES:-0}}"
if [ "$planes" != "1" ]; then
  exit 0
fi

# Fail closed on an explicit hold from a caller-supplied planes verdict file.
# The path is caller-owned; we never read home for keys.
verdict="${BEZEL_PLANES_VERDICT:-${OMAPI_PLANES_VERDICT:-}}"
if [ -n "$verdict" ]; then
  if [ ! -f "$verdict" ]; then
    echo "bezel-planes-shim: planes verdict is not a file (fail closed)." >&2
    exit 2
  fi
  gate=$(grep -E '^gate=' "$verdict" | head -n1 | cut -d= -f2- || true)
  if [ "$gate" = "hold" ]; then
    echo "bezel-planes-shim: verdict gate=hold — blocking exec." >&2
    exit 2
  fi
fi

exit 0
