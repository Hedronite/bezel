{ writeShellApplication, jev-router, lib }:

# Grok Build PreToolUse adapter. One router, one JEV_BYPASS predicate.
# The shell does not classify tools and does not bake TYPESAFE_API_KEY.
# JEV_MODE / JEV_PERMISSION_MODE / JEV_TYPED_CALL_MODE stay at the process
# environment. This wrapper does not default any of them to active.

writeShellApplication {
  name = "grok-build-jev";
  runtimeInputs = [
    jev-router
  ];
  text = ''
    exec ${lib.getExe jev-router} "$@"
  '';
}
