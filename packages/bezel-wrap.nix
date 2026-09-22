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

    makeWrapper ${lib.getExe ompBinary} $out/bin/bezel-omp \
      --prefix PATH : ${
        lib.makeBinPath ([ nodejs ] ++ lib.optional (jevRouter != null) jevRouter)
      } \
      --set-default BEZEL_OVERLAY 1 \
      --set-default OMAPI_OVERLAY 1 \
      --run "if [ \"\''${BEZEL_PLANES:-0}\" = 1 ] || [ \"\''${OMAPI_PLANES:-0}\" = 1 ]; then \"$out/libexec/bezel-planes-shim\"; fi"
    ln -s bezel-omp $out/bin/omapi

    runHook postInstall
  '';

  meta = {
    description = "Bezel omp engine path. Runs upstream omp with the Bezel toolkit on PATH";
    homepage = "https://github.com/VirtualMachinist/bezel";
    mainProgram = "bezel-omp";
    license = lib.licenses.mit;
    platforms = ompBinary.meta.platforms or lib.platforms.unix;
  };
}
