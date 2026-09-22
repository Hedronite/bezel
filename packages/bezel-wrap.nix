{
  lib,
  stdenvNoCC,
  makeWrapper,
  nodejs,
  ompBinary,
  jevRouter ? null,
}:

# Thin wrap: host omp binary (or fail-closed runtime resolver) → $out/bin/bezel.
# Real makeWrapper — not a comment-only stub.
# `omapi` is a compatibility name for the same program. It still execs upstream omp.
#
# Contract:
#   makeWrapper ${ompBinary}/bin/omp $out/bin/bezel
#     --prefix PATH : nodejs (+ optional bezel-jev)
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

    makeWrapper ${lib.getExe ompBinary} $out/bin/bezel \
      --prefix PATH : ${
        lib.makeBinPath ([ nodejs ] ++ lib.optional (jevRouter != null) jevRouter)
      } \
      --set-default BEZEL_OVERLAY 1 \
      --set-default OMAPI_OVERLAY 1 \
      --run "if [ \"\''${BEZEL_PLANES:-0}\" = 1 ] || [ \"\''${OMAPI_PLANES:-0}\" = 1 ]; then \"$out/libexec/bezel-planes-shim\"; fi"
    ln -s bezel $out/bin/omapi

    runHook postInstall
  '';

  meta = {
    description = "Bezel command that runs upstream omp with the Bezel toolkit on PATH";
    mainProgram = "bezel";
    license = lib.licenses.mit;
    platforms = ompBinary.meta.platforms or lib.platforms.unix;
  };
}
