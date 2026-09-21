{ writeShellApplication, jev-router, jq }:

# cursor-agent-jev "<intent>" [--step-digest TEXT] -- <cursor-agent args>
# JEV_ROUTER / JEV_MODE / JEV_BYPASS / JEV_STEP_DIGEST at runtime. Shadow never blocks.
# JEV_BYPASS is only "1" or "true" — same predicate as isJevBypass in
# packages/jev-router/policy.mjs. Do not add another spelling here.
# Active: stop/escalate do NOT exec; continue / gate=auto execs. Escalate is HITL.

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
    step_digest="''${JEV_STEP_DIGEST:-}"
    while [ "$#" -gt 0 ]; do
      if [ "$1" = "--" ]; then
        shift
        break
      fi
      if [ "$1" = "--step-digest" ] && [ -n "''${2:-}" ]; then
        step_digest="$2"
        shift 2
        continue
      fi
      if [ -z "$intent" ]; then
        intent="$1"
        shift
        continue
      fi
      break
    done

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
    if [ -n "$step_digest" ]; then
      JEV_MODE="$mode" JEV_STEP_DIGEST="$step_digest" "$router" --step-digest "$step_digest" "$intent" >"$tmp"
    else
      JEV_MODE="$mode" "$router" "$intent" >"$tmp"
    fi
    router_rc=$?
    set -e

    if [ -s "$tmp" ]; then
      echo "cursor-agent-jev: verdict $(tr -d '\n' <"$tmp")" >&2
    fi

    choice="$(jq -r '.choice // "unclassified"' <"$tmp" 2>/dev/null || echo unclassified)"
    gate="$(jq -r '.gate // "hold"' <"$tmp" 2>/dev/null || echo hold)"
    blocked="$(jq -r '.blocked // false' <"$tmp" 2>/dev/null || echo false)"
    hitl="$(jq -r '.hitl // false' <"$tmp" 2>/dev/null || echo false)"

    if [ "$mode" = "shadow" ]; then
      if [ "$router_rc" -ne 0 ]; then
        echo "cursor-agent-jev: router rc=$router_rc; shadow does not block" >&2
      fi
      echo "cursor-agent-jev: shadow Choice logged choice=$choice gate=$gate; exec cursor-agent (does not block)" >&2
      exec cursor-agent "$@"
    fi

    if [ "$router_rc" -ne 0 ]; then
      echo "cursor-agent-jev: active router rc=$router_rc choice=$choice gate=$gate — not execing cursor-agent" >&2
      exit 2
    fi

    if [ "$choice" = "stop" ] || [ "$choice" = "escalate" ]; then
      echo "cursor-agent-jev: active choice=$choice gate=$gate hitl=$hitl — stop/escalate do not exec (HITL, no auto-retry)" >&2
      exit 2
    fi

    if [ "$blocked" = "true" ]; then
      echo "cursor-agent-jev: active blocked=true choice=$choice gate=$gate — not execing cursor-agent" >&2
      exit 2
    fi

    if [ "$choice" != "continue" ] && [ "$gate" != "auto" ]; then
      echo "cursor-agent-jev: active choice=$choice gate=$gate — honor continue/auto only" >&2
      exit 2
    fi

    echo "cursor-agent-jev: active choice=$choice gate=$gate — exec cursor-agent" >&2
    exec cursor-agent "$@"
  '';
}
