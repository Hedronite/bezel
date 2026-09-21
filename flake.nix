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

            got="$(OMP_BIN="$fake" "$wrap" --version)"
            echo "$got" | grep -q 'overlay=1'
            echo "$got" | grep -q -- '--version'

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
        }
      );
    };
}
