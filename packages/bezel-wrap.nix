{
  lib,
  stdenvNoCC,
  makeWrapper,
  nodejs,
  ompBinary,
  jevRouter ? null,
}:

# Thin wrap: host omp binary (or fail-closed runtime resolver) → $out/bin/bezel-omp.
# The product is Bezel. This binary is the omp engine path, so it is not named bezel.
# `omapi` is a symlink to bezel-omp for one release. It still execs upstream omp.
#
# Contract:
#   makeWrapper ${ompBinary}/bin/omp $out/bin/bezel-omp
#     --prefix PATH : nodejs (+ optional jev-router)
#     --set-default BEZEL_OVERLAY 1
#     --set-default OMAPI_OVERLAY 1
#     --run planes-shim when BEZEL_PLANES=1 or OMAPI_PLANES=1
#     --run put --config=<omp-overlay.yml> ahead of the caller's args, unless
#           BEZEL_OMP_OVERLAY=0
#
# The omp overlay (packages/omp-overlay.yml) holds exactly one setting,
# tools.outputMaxColumns, so one-line JSON skill replies reach the model whole.
# omp accepts --config more than once, so a caller's own --config comes later
# and wins. BEZEL_OMP_OVERLAY=0 passes no overlay at all.
#
# Forwarded at runtime (NOT build-time): TYPESAFE_API_KEY, JEV_*, CURSOR_*,
# OMP_*, BEZEL_*, OMAPI_*, HOME. Never --set those from flake attrs or builtins.getEnv.

stdenvNoCC.mkDerivation {
  pname = "bezel";
  version = ompBinary.version or "overlay";

  dontUnpack = true;
  dontConfigure = true;
  dontBuild = true;

  nativeBuildInputs = [ makeWrapper ];

  installPhase = ''
    runHook preInstall

    mkdir -p $out/bin $out/libexec
    install -Dm755 ${./bezel-planes-shim.sh} $out/libexec/bezel-planes-shim
    ln -s bezel-planes-shim $out/libexec/omapi-planes-shim
    install -Dm644 ${./omp-overlay.yml} $out/share/bezel/omp-overlay.yml

    makeWrapper ${lib.getExe ompBinary} $out/bin/bezel-omp \
      --prefix PATH : ${
        lib.makeBinPath ([ nodejs ] ++ lib.optional (jevRouter != null) jevRouter)
      } \
      --set-default BEZEL_OVERLAY 1 \
      --set-default OMAPI_OVERLAY 1 \
      --run "if [ \"\''${BEZEL_PLANES:-0}\" = 1 ] || [ \"\''${OMAPI_PLANES:-0}\" = 1 ]; then \"$out/libexec/bezel-planes-shim\"; fi" \
      --run "if [ \"\''${BEZEL_OMP_OVERLAY:-1}\" != 0 ]; then set -- \"--config=$out/share/bezel/omp-overlay.yml\" \"\$@\"; fi"
    ln -s bezel-omp $out/bin/omapi

    runHook postInstall
  '';

  meta = {
    description = "Bezel omp engine path. Runs upstream omp with the Bezel toolkit on PATH";
    homepage = "https://github.com/Hedronite/bezel";
    mainProgram = "bezel-omp";
    license = lib.licenses.mit;
    platforms = ompBinary.meta.platforms or lib.platforms.unix;
  };
}
