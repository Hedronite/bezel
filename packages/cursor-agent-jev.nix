{ writeShellApplication, jev-router, jq }:

# cursor-agent-jev "<intent>" -- <cursor-agent args>
# JEV_ROUTER / JEV_MODE / JEV_BYPASS at runtime. Shadow never blocks.

writeShellApplication {
  name = "cursor-agent-jev";
  runtimeInputs = [
    jev-router
    jq
  ];
  text = ''
    mode="''${JEV_MODE:-shadow}"
    router="''${JEV_ROUTER:-jev-router}"

    intent=""
    if [ "$#" -gt 0 ] && [ "$1" != "--" ]; then
      intent="$1"
      shift
    fi
    if [ "''${1:-}" = "--" ]; then
      shift
    fi

    if ! command -v cursor-agent >/dev/null 2>&1; then
      echo "cursor-agent-jev: cursor-agent not on PATH (fail closed)." >&2
      exit 127
    fi

    if [ "''${JEV_BYPASS:-0}" = "1" ] || [ "''${JEV_BYPASS:-}" = "true" ]; then
      echo "cursor-agent-jev: JEV_BYPASS — skipping router, exec cursor-agent" >&2
      exec cursor-agent "$@"
    fi

    tmp="$(mktemp)"
    trap 'rm -f "$tmp"' EXIT

    set +e
    JEV_MODE="$mode" "$router" "$intent" >"$tmp"
    router_rc=$?
    set -e

    if [ -s "$tmp" ]; then
      echo "cursor-agent-jev: verdict $(tr -d '\n' <"$tmp")" >&2
    fi

    if [ "$router_rc" -ne 0 ]; then
      if [ "$mode" = "shadow" ]; then
        echo "cursor-agent-jev: router rc=$router_rc; shadow does not block" >&2
        exec cursor-agent "$@"
      fi
      echo "cursor-agent-jev: active router rc=$router_rc — not execing cursor-agent" >&2
      exit 2
    fi

    gate="$(jq -r '.gate // "hold"' <"$tmp")"
    blocked="$(jq -r '.blocked // false' <"$tmp")"

    if [ "$mode" = "active" ] && { [ "$gate" != "auto" ] || [ "$blocked" = "true" ]; }; then
      echo "cursor-agent-jev: active gate=$gate blocked=$blocked — honor gate:auto only" >&2
      exit 2
    fi

    if [ "$mode" = "shadow" ]; then
      echo "cursor-agent-jev: shadow Choice logged; exec cursor-agent (does not block)" >&2
    fi

    exec cursor-agent "$@"
  '';
}
