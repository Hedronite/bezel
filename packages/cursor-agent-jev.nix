{ writeShellApplication, jev-router, jq }:

# cursor-agent-jev "<intent>" [--step-digest TEXT] -- <cursor-agent args>
# cursor-agent-jev --catalog
# cursor-agent-jev --schema TOOL
# JEV_ROUTER / JEV_MODE / JEV_BYPASS / JEV_STEP_DIGEST at runtime. Shadow never blocks.
# Success exec is silent: no launch line, and the router's stderr is not
# forwarded onto the child. Failures still explain on stderr.
# JEV_BYPASS is only "1" or "true" — same predicate as isJevBypass in
# packages/jev-router/policy.mjs. Do not add another spelling here.
# Active: stop/escalate do NOT exec; continue / gate=auto execs. Escalate is HITL.
# --catalog / --schema do not exec cursor-agent and do not start a second client.
# On exec, JEV_TOOL_CATALOG is the tiny index (no JSON Schema, no API key).

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
    show_catalog=0
    schema_tool=""
    while [ "$#" -gt 0 ]; do
      if [ "$1" = "--" ]; then
        shift
        break
      fi
      if [ "$1" = "--catalog" ]; then
        show_catalog=1
        shift
        continue
      fi
      if [ "$1" = "--schema" ] || [ "$1" = "--schema-dump" ]; then
        if [ -z "''${2:-}" ] || [ "$2" = "--" ]; then
          echo "cursor-agent-jev: --schema needs a tool name" >&2
          exit 2
        fi
        schema_tool="$2"
        shift 2
        continue
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

    # Query paths. No cursor-agent, no Choice, no allow.
    if [ -n "$schema_tool" ]; then
      exec "$router" --schema "$schema_tool"
    fi
    if [ "$show_catalog" = 1 ]; then
      exec "$router" --catalog
    fi

    if ! command -v cursor-agent >/dev/null 2>&1; then
      echo "cursor-agent-jev: cursor-agent not on PATH (fail closed)." >&2
      exit 127
    fi

    if [ "''${JEV_BYPASS:-0}" = "1" ] || [ "''${JEV_BYPASS:-}" = "true" ]; then
      cat_tmp="$(mktemp)"
      if "$router" --catalog >"$cat_tmp" 2>/dev/null; then
        line="$(jq -c 'select(.kind == "tiny")' <"$cat_tmp" 2>/dev/null || true)"
        if [ -n "$line" ] && [ "$line" != "null" ]; then
          JEV_TOOL_CATALOG="$line"
          export JEV_TOOL_CATALOG
        fi
      fi
      rm -f "$cat_tmp"
      exec cursor-agent "$@"
    fi

    tmp="$(mktemp)"
    router_err="$(mktemp)"
    trap 'rm -f "$tmp" "$router_err"' EXIT

    set +e
    if [ -n "$step_digest" ]; then
      JEV_MODE="$mode" JEV_STEP_DIGEST="$step_digest" "$router" --step-digest "$step_digest" "$intent" >"$tmp" 2>"$router_err"
    else
      JEV_MODE="$mode" "$router" "$intent" >"$tmp" 2>"$router_err"
    fi
    router_rc=$?
    set -e

    choice="$(jq -r '.choice // "unclassified"' <"$tmp" 2>/dev/null || echo unclassified)"
    gate="$(jq -r '.gate // "hold"' <"$tmp" 2>/dev/null || echo hold)"
    blocked="$(jq -r '.blocked // false' <"$tmp" 2>/dev/null || echo false)"
    hitl="$(jq -r '.hitl // false' <"$tmp" 2>/dev/null || echo false)"
    line="$(jq -c '.catalog | select(.kind == "tiny")' <"$tmp" 2>/dev/null || true)"
    if [ -n "$line" ] && [ "$line" != "null" ]; then
      JEV_TOOL_CATALOG="$line"
      export JEV_TOOL_CATALOG
    fi

    if [ "$mode" = "shadow" ]; then
      if [ "$router_rc" -ne 0 ]; then
        echo "cursor-agent-jev: router rc=$router_rc; shadow does not block" >&2
      fi
      rm -f "$tmp" "$router_err"
      trap - EXIT
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

    rm -f "$tmp" "$router_err"
    trap - EXIT
    exec cursor-agent "$@"
  '';
}
