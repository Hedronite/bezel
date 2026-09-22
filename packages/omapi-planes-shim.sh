#!/bin/sh
# Optional PLANES pre-exec filter. Lives in $out/libexec — not compiled into omp.
# Runs only when the omapi wrapper sees OMAPI_PLANES=1.
# Env forwarded at runtime (OMP_*, OMAPI_*, JEV_*, CURSOR_*, TYPESAFE_API_KEY).
# This script must not print or persist secrets.

set -eu

if [ "${OMAPI_PLANES:-0}" != "1" ]; then
  exit 0
fi

# Fail closed on an explicit hold from a caller-supplied planes verdict file.
# The path is caller-owned; we never read home for keys.
if [ -n "${OMAPI_PLANES_VERDICT:-}" ]; then
  if [ ! -f "$OMAPI_PLANES_VERDICT" ]; then
    echo "omapi-planes-shim: OMAPI_PLANES_VERDICT is not a file (fail closed)." >&2
    exit 2
  fi
  gate=$(grep -E '^gate=' "$OMAPI_PLANES_VERDICT" | head -n1 | cut -d= -f2- || true)
  if [ "$gate" = "hold" ]; then
    echo "omapi-planes-shim: verdict gate=hold — blocking exec." >&2
    exit 2
  fi
fi

exit 0
