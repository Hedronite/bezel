{
  lib,
  stdenvNoCC,
  makeWrapper,
  nodejs,
  ompBinary,
  jevRouter ? null,
}:

# Thin wrap: stock omp (or fail-closed runtime resolver) → $out/bin/omapi.
# Real makeWrapper — not a comment-only stub (Jupi r1 → r2).
#
# Contract:
#   makeWrapper ${ompBinary}/bin/omp $out/bin/omapi
#     --prefix PATH : nodejs (+ optional jev-router)
#     --set-default OMAPI_OVERLAY 1
#     --run planes-shim when OMAPI_PLANES=1
#
# Forwarded at runtime (NOT build-time): TYPESAFE_API_KEY, JEV_*, CURSOR_*,
# OMP_*, OMAPI_*, HOME. Never --set those from flake attrs or builtins.getEnv.

stdenvNoCC.mkDerivation {
  pname = "omapi";
  version = ompBinary.version or "overlay";

  dontUnpack = true;
  dontConfigure = true;
  dontBuild = true;

  nativeBuildInputs = [ makeWrapper ];

  installPhase = ''
    runHook preInstall

    mkdir -p $out/bin $out/libexec
    install -Dm755 ${./omapi-planes-shim.sh} $out/libexec/omapi-planes-shim

    makeWrapper ${lib.getExe ompBinary} $out/bin/omapi \
      --prefix PATH : ${
        lib.makeBinPath ([ nodejs ] ++ lib.optional (jevRouter != null) jevRouter)
      } \
      --set-default OMAPI_OVERLAY 1 \
      --run "if [ \"\''${OMAPI_PLANES:-0}\" = 1 ]; then \"$out/libexec/omapi-planes-shim\"; fi"

    runHook postInstall
  '';

  meta = {
    description = "Thin wrap of a host or pinned omp binary (optional engine path)";
    homepage = "https://github.com/VirtualMachinist/omapi-overlay";
    mainProgram = "omapi";
    license = lib.licenses.mit;
    platforms = ompBinary.meta.platforms or lib.platforms.unix;
  };
}
