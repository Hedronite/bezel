{
  symlinkJoin,
  writeShellApplication,
  bezel-jev,
  nodejs,
  lib,
}:

# Grok Build PreToolUse adapter. One router, one JEV_BYPASS predicate.
# The shell does not classify tools and does not bake TYPESAFE_API_KEY.
# JEV_MODE / JEV_PERMISSION_MODE / JEV_TYPED_CALL_MODE stay at the process
# environment. This wrapper does not default any of them to active.
# `grok-build-jev` is the same program, for one transition.

let
  inner = writeShellApplication {
    name = "grok-build-bezel";
    runtimeInputs = [
      bezel-jev
    ];
    text = ''
      if [ -z "''${JEV_ROUTER:-}" ]; then
        export JEV_ROUTER="${lib.getExe bezel-jev}"
      fi
      exec ${lib.getExe nodejs} ${bezel-jev}/lib/jev-router/harness.mjs "$@"
    '';
  };
in
symlinkJoin {
  name = "grok-build-bezel";
  paths = [ inner ];
  postBuild = ''
    ln -s grok-build-bezel $out/bin/grok-build-jev
  '';
  meta = {
    description = "Bezel PreToolUse adapter for Grok Build. Prints defer or deny.";
    mainProgram = "grok-build-bezel";
    license = lib.licenses.mit;
    platforms = lib.platforms.unix;
  };
}
