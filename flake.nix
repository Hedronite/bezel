{
  description = "omapi-overlay: stock omp pin + thin omapi wrap + skills SoT + Jev gates. Not an omp source fork.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    # nixpkgs 26.11 dropped Intel macOS. Keep x86_64-darwin declared (Jupi r1
    # missed this slice) on the last Darwin-x64-supporting pin — not a skip-as-green.
    nixpkgs-darwin-x64.url = "github:NixOS/nixpkgs/nixpkgs-26.05-darwin";

    # Sole skills SoT. omahedron-skills 404s today — path stub until it exists.
    # Swap to: github:VirtualMachinist/omahedron-skills
    # Do not add a second repo-root skills/ product tree.
    skills = {
      url = "path:./skills-stub";
      flake = false;
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      nixpkgs-darwin-x64,
      skills,
      ...
    }:
    let
      inherit (nixpkgs) lib;

      systems = [
        "aarch64-darwin"
        "x86_64-darwin"
        "aarch64-linux"
        "x86_64-linux"
      ];

      forAllSystems = f: lib.genAttrs systems (system: f system);

      nixpkgsFor = system: if system == "x86_64-darwin" then nixpkgs-darwin-x64 else nixpkgs;

      pkgsFor =
        system:
        import (nixpkgsFor system) {
          inherit system;
          overlays = [ self.overlays.default ];
        };
    in
    {
      overlays.default = final: _prev: {
        omp-runtime = final.callPackage ./packages/omp-runtime.nix { };
        omp-pin = final.callPackage ./packages/omp-pin.nix { };
        jev-router = final.callPackage ./packages/jev-router { };
        omapi = final.callPackage ./packages/omapi-wrap.nix {
          ompBinary = final.omp-runtime;
          jevRouter = final.jev-router;
        };
        omapi-pinned = final.callPackage ./packages/omapi-wrap.nix {
          ompBinary = final.omp-pin;
          jevRouter = final.jev-router;
        };
        cursor-agent-jev = final.callPackage ./packages/cursor-agent-jev.nix {
          jev-router = final.jev-router;
        };
        grok-build-jev = final.callPackage ./packages/grok-build-jev.nix { };
      };

      packages = forAllSystems (
        system:
        let
          pkgs = pkgsFor system;
        in
        {
          inherit (pkgs)
            omapi
            omapi-pinned
            jev-router
            cursor-agent-jev
            grok-build-jev
            omp-runtime
            omp-pin
            ;
          default = pkgs.omapi;
        }
      );

      apps = forAllSystems (system: {
        omapi = {
          type = "app";
          program = lib.getExe self.packages.${system}.omapi;
        };
        jev-router = {
          type = "app";
          program = lib.getExe self.packages.${system}.jev-router;
        };
        cursor-agent-jev = {
          type = "app";
          program = lib.getExe self.packages.${system}.cursor-agent-jev;
        };
        grok-build-jev = {
          type = "app";
          program = lib.getExe self.packages.${system}.grok-build-jev;
        };
      });

      homeManagerModules.default = import ./modules/home-manager.nix { inherit skills; };

      checks = forAllSystems (
        system:
        let
          pkgs = pkgsFor system;
        in
        {
          # Cheap overlay packages. omapi-pinned is NOT a check:
          # it fetchurls the 180–240MB stock omp binary.
          omapi = self.packages.${system}.omapi;
          jev-router = self.packages.${system}.jev-router;
          cursor-agent-jev = self.packages.${system}.cursor-agent-jev;
          grok-build-jev = self.packages.${system}.grok-build-jev;

          omapi-wrap-contract = pkgs.runCommand "omapi-wrap-contract" { } ''
            set -eu
            wrap="${self.packages.${system}.omapi}/bin/omapi"
            test -x "$wrap"
            grep -q OMAPI_OVERLAY "$wrap"
            grep -q 'makeWrapper\|OMAPI_OVERLAY' "$wrap"
            test -x "${self.packages.${system}.omapi}/libexec/omapi-planes-shim"
            if grep -E -- '--set(=| )TYPESAFE_API_KEY' "$wrap"; then
              echo "omapi wrapper must not bake TYPESAFE_API_KEY" >&2
              exit 1
            fi

            fake="$(mktemp)"
            cat >"$fake" <<'EOS'
            #!/bin/sh
            printf 'fake-omp overlay=%s planes=%s args=%s\n' "''${OMAPI_OVERLAY-}" "''${OMAPI_PLANES-}" "$*"
            EOS
            chmod +x "$fake"

            outf="$(mktemp)"
            errf="$(mktemp)"
            OMP_BIN="$fake" "$wrap" --version >"$outf" 2>"$errf"
            got="$(cat "$outf")"
            echo "$got" | grep -q 'overlay=1'
            echo "$got" | grep -q -- '--version'
            # Launch is quiet. The mark banner is deleted, not hidden on a non-TTY.
            if grep -F 'oma on' "$outf" "$errf" "$wrap" || grep -F 'omapi-mark' "$outf" "$errf" "$wrap"; then
              echo "omapi launch printed a splash banner" >&2
              exit 1
            fi

            echo ok >"$out"
          '';

          jev-bypass = pkgs.runCommand "jev-bypass" { } ''
            set -eu
            router="${lib.getExe self.packages.${system}.jev-router}"
            jq="${lib.getExe pkgs.jq}"
            outj="$(JEV_BYPASS=1 "$router" "probe intent")"
            echo "$outj" | "$jq" -e '.bypass == true and .gate == "auto" and .blocked == false'
            outt="$(JEV_BYPASS=true "$router" --tool-name run_terminal_command "probe intent")"
            echo "$outt" | "$jq" -e '.bypass == true and .pretool.class == "shell" and .pretool.policyId == "omapi-loop-stop-policy@1"'
            outn="$(JEV_BYPASS=yes "$router" "probe intent")"
            echo "$outn" | "$jq" -e '.bypass == false and .missingKey == true and .blocked == false'
            echo ok >"$out"
          '';

          # Unit tests: policy, available-gate, check envelope. No live Jev.
          jev-router-unit = pkgs.runCommand "jev-router-unit" { nativeBuildInputs = [ pkgs.nodejs ]; } ''
            set -eu
            cp -r ${./packages/jev-router}/. .
            cp ${./docs/POLICY-MAP.md} ./POLICY-MAP.md
            cp ${./docs/GROK-BUILD.md} ./GROK-BUILD.md
            cp ${./packages/cursor-agent-jev.nix} ./cursor-agent-jev.nix
            JEV_ROUTER_BIN="${lib.getExe self.packages.${system}.jev-router}" node test.mjs
            echo ok >"$out"
          '';

          # Shadow --check without TYPESAFE_API_KEY: continues, never "approved".
          jev-check-shadow = pkgs.runCommand "jev-check-shadow" { nativeBuildInputs = [ pkgs.jq ]; } ''
            set -eu
            router="${lib.getExe self.packages.${system}.jev-router}"
            skip="${./packages/jev-router/testdata/skip-marker.diff}"
            empty="${./packages/jev-router/testdata/empty.diff}"

            outj="$(JEV_MODE=shadow "$router" --check --intent probe --diff-file "$skip")"
            echo "$outj" | jq -e '.approval == false and .emptyFindingsAreNotApproval == true and .missingKey == true'
            echo "$outj" | jq -e '[.findings[].flag] | index("skip_marker_added") != null'
            if echo "$outj" | grep -Ei 'approved'; then
              echo "empty/partial findings must not print approved" >&2
              exit 1
            fi

            out2="$(JEV_MODE=shadow "$router" --check --intent probe --diff-file "$empty")"
            echo "$out2" | jq -e '.status == "no_diff" and .approval == false and .facts.diffPresent == false'

            out3="$(JEV_MODE=shadow "$router" "probe intent")"
            echo "$out3" | jq -e '.missingKey == true and .blocked == false and .workflow.shadow == true'
            echo "$out3" | jq -e '.facts.capabilities != null'
            echo "$out3" | jq -e '.autoRetry == false and .autoPromote == false and .loop.reason == "missing_key"'
            echo "$out3" | jq -e '.loopStopPolicy == "omapi-loop-stop-policy@1"'

            echo ok >"$out"
          '';

          # Shadow/active wrap: fake router + fake cursor-agent. No live Jev.
          # Shadow always execs; active stop/escalate do not; continue/auto execs.
          jev-loop-stop-wrap = pkgs.runCommand "jev-loop-stop-wrap" { nativeBuildInputs = [ pkgs.jq ]; } ''
            set -eu
            wrap="${lib.getExe self.packages.${system}.cursor-agent-jev}"
            work="$(mktemp -d)"
            mkdir -p "$work/bin"

            cat >"$work/bin/cursor-agent" <<'EOS'
            #!/bin/sh
            printf 'ran:%s\n' "$*" >> "$AGENT_LOG"
            EOS
            chmod +x "$work/bin/cursor-agent"

            write_router() {
              name="$1"
              json="$2"
              cat >"$work/bin/$name" <<EOF
            #!/bin/sh
            printf '%s\n' '$json'
            exit 0
            EOF
              chmod +x "$work/bin/$name"
            }

            write_router router-continue '{"ok":true,"choice":"continue","gate":"auto","blocked":false,"exec":true,"hitl":false,"autoRetry":false}'
            write_router router-stop '{"ok":true,"choice":"stop","gate":"hold","blocked":true,"exec":false,"hitl":false,"autoRetry":false}'
            write_router router-escalate '{"ok":true,"choice":"escalate","gate":"hold","blocked":true,"exec":false,"hitl":true,"autoRetry":false,"autoPromote":false}'
            write_router router-auto '{"ok":true,"choice":"unclassified","gate":"auto","blocked":false,"exec":true,"hitl":false}'

            export PATH="$work/bin:$PATH"

            export AGENT_LOG="$work/shadow-stop.log"
            : >"$AGENT_LOG"
            JEV_MODE=shadow JEV_ROUTER="$work/bin/router-stop" "$wrap" "intent" -- --probe
            grep -q '^ran:' "$AGENT_LOG"

            export AGENT_LOG="$work/shadow-escalate.log"
            : >"$AGENT_LOG"
            JEV_MODE=shadow JEV_ROUTER="$work/bin/router-escalate" "$wrap" "intent" -- --probe
            grep -q '^ran:' "$AGENT_LOG"

            export AGENT_LOG="$work/active-stop.log"
            : >"$AGENT_LOG"
            set +e
            JEV_MODE=active JEV_ROUTER="$work/bin/router-stop" "$wrap" "intent" -- --probe
            rc=$?
            set -e
            test "$rc" -ne 0
            if [ -s "$AGENT_LOG" ]; then
              echo "active stop must not exec cursor-agent" >&2
              exit 1
            fi

            export AGENT_LOG="$work/active-escalate.log"
            : >"$AGENT_LOG"
            set +e
            JEV_MODE=active JEV_ROUTER="$work/bin/router-escalate" "$wrap" "intent" -- --probe
            rc=$?
            set -e
            test "$rc" -ne 0
            if [ -s "$AGENT_LOG" ]; then
              echo "active escalate must not exec cursor-agent" >&2
              exit 1
            fi

            export AGENT_LOG="$work/active-continue.log"
            : >"$AGENT_LOG"
            JEV_MODE=active JEV_ROUTER="$work/bin/router-continue" "$wrap" "intent" -- --probe
            grep -q '^ran:' "$AGENT_LOG"

            export AGENT_LOG="$work/active-auto.log"
            : >"$AGENT_LOG"
            JEV_MODE=active JEV_ROUTER="$work/bin/router-auto" "$wrap" "intent" -- --probe
            grep -q '^ran:' "$AGENT_LOG"

            grep -q 'stop/escalate do not exec' "$wrap"
            echo ok >"$out"
          '';

          # Tiny catalog is always on the verdict. Full schema is a separate
          # call and never an allow. No live Jev.
          jev-tool-catalog = pkgs.runCommand "jev-tool-catalog" { nativeBuildInputs = [ pkgs.jq ]; } ''
            set -eu
            router="${lib.getExe self.packages.${system}.jev-router}"
            wrap="${lib.getExe self.packages.${system}.cursor-agent-jev}"
            work="$(mktemp -d)"
            mkdir -p "$work/bin"

            cat >"$work/bin/cursor-agent" <<'EOS'
            #!/bin/sh
            printf 'ran catalog=%s\n' "$JEV_TOOL_CATALOG" >> "$AGENT_LOG"
            EOS
            chmod +x "$work/bin/cursor-agent"

            catj="$("$router" --catalog)"
            echo "$catj" | jq -e '.kind == "tiny" and (.entries | length) == 5'
            echo "$catj" | jq -e '[.entries[].policyId] | unique | sort == ["omapi-check-policy@1","omapi-loop-stop-policy@1","omapi-route-workflow-policy@1"]'
            if echo "$catj" | grep -F '$schema' >/dev/null; then
              echo "tiny catalog must not carry schemas" >&2
              exit 1
            fi
            if echo "$catj" | grep -F '"decision":"allow"' >/dev/null; then
              echo "catalog must not allow" >&2
              exit 1
            fi

            schema="$("$router" --schema Bash)"
            echo "$schema" | jq -e '.call == "schema-dump" and .decision == "defer" and .autoAllow == false and .policyId == "omapi-loop-stop-policy@1"'
            echo "$schema" | jq -e '.schema.properties.command.type == "string"'
            if echo "$schema" | grep -F '"decision":"allow"' >/dev/null; then
              echo "schema dump must not allow" >&2
              exit 1
            fi

            set +e
            denied="$("$router" --schema read_file)"
            rc=$?
            set -e
            test "$rc" -eq 2
            echo "$denied" | jq -e '.decision == "deny" and .schema == null and .autoAllow == false'

            export PATH="$work/bin:$PATH"

            export AGENT_LOG="$work/schema.log"
            : >"$AGENT_LOG"
            "$wrap" --schema Edit >/dev/null
            if [ -s "$AGENT_LOG" ]; then
              echo "schema dump must not exec cursor-agent" >&2
              exit 1
            fi

            export AGENT_LOG="$work/catalog.log"
            : >"$AGENT_LOG"
            "$wrap" --catalog >/dev/null
            if [ -s "$AGENT_LOG" ]; then
              echo "catalog query must not exec cursor-agent" >&2
              exit 1
            fi

            export AGENT_LOG="$work/ran.log"
            : >"$AGENT_LOG"
            JEV_MODE=shadow "$wrap" probe -- --hi
            grep -q 'omapi-loop-stop-policy@1' "$AGENT_LOG"
            if grep -F '$schema' "$AGENT_LOG" >/dev/null; then
              echo "agent catalog must stay tiny" >&2
              exit 1
            fi

            export AGENT_LOG="$work/bypass.log"
            : >"$AGENT_LOG"
            JEV_BYPASS=1 "$wrap" probe -- --hi
            grep -q 'omapi-route-workflow-policy@1' "$AGENT_LOG"
            if grep -F '$schema' "$AGENT_LOG" >/dev/null; then
              echo "bypass catalog must stay tiny" >&2
              exit 1
            fi

            echo ok >"$out"
          '';

          # Packaged harness: class → policy id, tiny catalog, FACET call,
          # permission shadow vs active. No live Jev and no API key.
          grok-build-harness = pkgs.runCommand "grok-build-harness" { nativeBuildInputs = [ pkgs.jq ]; } ''
            set -eu
            bin="${lib.getExe self.packages.${system}.grok-build-jev}"

            smoke="$("$bin" smoke)"
            echo "$smoke" | jq -e '.ok == true'
            echo "$smoke" | jq -e '.pretool.class == "shell" and .pretool.matched == true and .pretool.policyId == "omapi-loop-stop-policy@1"'
            echo "$smoke" | jq -e '[.classes[].policyId] | unique | sort == ["omapi-check-policy@1","omapi-loop-stop-policy@1","omapi-route-workflow-policy@1"]'
            echo "$smoke" | jq -e '.catalog.kind == "tiny" and .catalog.schemaInline == false and .catalog.bytes < 900'
            echo "$smoke" | jq -e '.schemaDump.decision == "defer" and .schemaDump.typedCall == false and .schemaDump.autoAllow == false'
            echo "$smoke" | jq -e '.calls.transport == "facet" and .calls.op == "tool_call" and .calls.initiated == false and .calls.autoAllow == false and .calls.policyId == "omapi-route-workflow-policy@1"'
            echo "$smoke" | jq -e '.permission.shadow.decision == "defer" and .permission.shadow.honor == false and .permission.shadow.blocked == false'
            echo "$smoke" | jq -e '.permission.active.decision == "deny" and .permission.active.honor == true and .permission.active.blocked == true'
            echo "$smoke" | jq -e '.permission.uncertain.decision == "deny"'
            echo "$smoke" | jq -e '.permission.emptyFindings.reason == "empty_findings_not_approval" and .permission.emptyFindings.decision == "deny" and .permission.emptyFindings.approval == false'
            echo "$smoke" | jq -e '.permission.keyAbsent.mode == "shadow" and .permission.keyAbsent.honor == false and .permission.keyAbsent.blocked == false'
            echo "$smoke" | jq -e '.bypass.decision == "defer" and .bypass.reason == "bypass"'
            echo "$smoke" | jq -e '.notBypass.decision == "deny"'
            echo "$smoke" | jq -e '.irreversible.decision == "deny" and .irreversible.code == "F454" and .irreversible.initiated == false and .irreversible.autoPromote == false'
            echo "$smoke" | jq -e '.uncertainActive.decision == "deny" and .uncertainShadow.decision == "defer"'
            echo "$smoke" | jq -e '.readStaysDefer.decision == "defer" and .loopStopWins.decision == "deny"'
            echo "$smoke" | jq -e '.unmapped.decision == "defer" and .unmapped.reason == "unmatched"'
            if echo "$smoke" | grep -F '"decision":"allow"' >/dev/null; then
              echo "smoke must not allow" >&2
              exit 1
            fi

            cfg="$("$bin" config)"
            echo "$cfg" | jq -e '.hooks.PreToolUse | length == 1'
            echo "$cfg" | jq -e '.hooks.PreToolUse[0].hooks[0].command == "grok-build-jev hook"'
            if echo "$cfg" | grep -E 'TYPESAFE_API_KEY|JEV_MODE|JEV_PERMISSION_MODE|JEV_TYPED_CALL_MODE' >/dev/null; then
              echo "config must not set a mode or a key" >&2
              exit 1
            fi

            work="$(mktemp -d)"
            cat >"$work/router" <<'EOS'
            #!/bin/sh
            if [ "''${HARNESS_MARK:-}" = "poison" ]; then
              : > "''${HARNESS_CALLED_FILE:?}"
              exit 99
            fi
            printf '%s\n' "$*" > "''${HARNESS_ARGV_LOG:-/dev/null}"
            printf '%s\n' '{"ok":true,"mode":"active","choice":"stop","gate":"hold","blocked":true,"exec":false,"bypass":false}'
            EOS
            chmod +x "$work/router"
            event='{"hookEventName":"PreToolUse","toolName":"Bash","toolInput":{"command":"npm test"}}'

            export HARNESS_ARGV_LOG="$work/argv"
            : >"$HARNESS_ARGV_LOG"
            set +e
            hook="$(printf '%s' "$event" | JEV_ROUTER="$work/router" JEV_BYPASS= JEV_MODE=active "$bin" hook)"
            rc=$?
            set -e
            test "$rc" -eq 2
            echo "$hook" | jq -e '.decision == "deny" and (.reason | contains("policy=omapi-loop-stop-policy@1"))'
            grep -q -- '--tool-name' "$HARNESS_ARGV_LOG"
            grep -q Bash "$HARNESS_ARGV_LOG"
            grep -q -- '--tool-input' "$HARNESS_ARGV_LOG"
            grep -q 'npm test' "$HARNESS_ARGV_LOG"

            rm -f "$work/called"
            set +e
            byp="$(printf '%s' "$event" | JEV_ROUTER="$work/router" HARNESS_MARK=poison HARNESS_CALLED_FILE="$work/called" JEV_BYPASS=1 "$bin" hook)"
            rc=$?
            set -e
            test "$rc" -eq 0
            echo "$byp" | jq -e '.decision == "defer" and .reason == "bypass"'
            if [ -e "$work/called" ]; then
              echo "bypass must not call the router" >&2
              exit 1
            fi

            cat >"$work/garbage" <<'EOS'
            #!/bin/sh
            printf '%s\n' 'not-json'
            EOS
            chmod +x "$work/garbage"
            set +e
            garbage="$(printf '%s' "$event" | JEV_ROUTER="$work/garbage" JEV_BYPASS= JEV_MODE=active "$bin" hook)"
            rc=$?
            set -e
            test "$rc" -eq 2
            echo "$garbage" | jq -e '.decision == "deny" and .reason == "jev uncertain"'

            set +e
            soft="$(printf '%s' "$event" | JEV_ROUTER="$work/garbage" JEV_BYPASS= JEV_MODE=shadow "$bin" hook)"
            rc=$?
            set -e
            test "$rc" -eq 0
            echo "$soft" | jq -e '.decision == "defer" and .reason == "shadow"'

            # Interactive entry is quiet. No banner on stdout or stderr.
            cat_out="$(mktemp)"
            cat_err="$(mktemp)"
            "$bin" catalog >"$cat_out" 2>"$cat_err"
            test ! -s "$cat_err"
            jq -e '.kind == "tiny"' <"$cat_out" >/dev/null
            if grep -F 'oma on' "$cat_out" "$cat_err" || grep -F 'omapi-mark' "$cat_out" "$cat_err"; then
              echo "grok-build-jev catalog printed a splash" >&2
              exit 1
            fi
            hook_out="$(mktemp)"
            hook_err="$(mktemp)"
            printf '%s' '' | JEV_MODE=shadow JEV_BYPASS= "$bin" hook >"$hook_out" 2>"$hook_err"
            echo "$hook_out" | jq -e '.decision == "defer"'
            if grep -F 'oma on' "$hook_out" "$hook_err" || grep -F 'omapi-mark' "$hook_out" "$hook_err"; then
              echo "grok-build-jev hook printed a splash" >&2
              exit 1
            fi

            echo ok >"$out"
          '';
        }
      );
    };
}
