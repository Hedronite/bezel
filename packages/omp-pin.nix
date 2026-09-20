{
  lib,
  stdenvNoCC,
  fetchurl,
  autoPatchelfHook,
}:

# Stock omp binary pin (can1357/oh-my-pi release assets).
# This is a fetchurl of upstream prebuilts — not an omp source fork,
# and not a vendor of omp sources into this repo.
#
# Hashes from https://github.com/can1357/oh-my-pi/releases/download/v18.2.6/SHA256SUMS.txt
# converted to SRI. If a future system has no artifact, throw (CI honesty).

let
  pname = "omp-stock";
  version = "18.2.6";

  sources = {
    aarch64-darwin = {
      asset = "omp-darwin-arm64";
      hash = "sha256-1JjaQNV34f+mgcqGMsLqQKn3IqCLiAQSAR033/7pUTo=";
    };
    x86_64-darwin = {
      asset = "omp-darwin-x64";
      hash = "sha256-1ViAD6Mmq8aK48u7PlNjIuxkO9z93HHmqYiFfb4alKM=";
    };
    aarch64-linux = {
      asset = "omp-linux-arm64";
      hash = "sha256-ByRcvgUMOZmrXOqbq/6E5+iBnS9NXkm+9HwKrLa5V+Q=";
    };
    x86_64-linux = {
      asset = "omp-linux-x64";
      hash = "sha256-DzhZjJHoI9jM4H8VHsOZnVHyE/ssw+B9ifGvju+SR6I=";
    };
  };

  inherit (stdenvNoCC.hostPlatform) system;

  source =
    sources.${system} or (throw ''
      omapi-overlay: no stock omp pin for system '${system}'.
      Documented pins: ${lib.concatStringsSep ", " (lib.attrNames sources)}.
      Do not fake a green build — mark skip in CI or set OMP_BIN and use omapi (runtime wrap).
    '');
in
stdenvNoCC.mkDerivation {
  inherit pname version;

  src = fetchurl {
    url = "https://github.com/can1357/oh-my-pi/releases/download/v${version}/${source.asset}";
    inherit (source) hash;
  };

  dontUnpack = true;
  dontStrip = true;
  dontPatchELF = true;

  nativeBuildInputs = lib.optionals stdenvNoCC.hostPlatform.isLinux [ autoPatchelfHook ];

  installPhase = ''
    runHook preInstall
    install -Dm755 $src $out/bin/omp
    runHook postInstall
  '';

  meta = {
    description = "Stock omp prebuilt from can1357/oh-my-pi v${version} (pin only; not a source fork)";
    homepage = "https://github.com/can1357/oh-my-pi";
    downloadPage = "https://github.com/can1357/oh-my-pi/releases/tag/v${version}";
    license = lib.licenses.mit;
    mainProgram = "omp";
    sourceProvenance = [ lib.sourceTypes.binaryNativeCode ];
    platforms = lib.attrNames sources;
  };
}
