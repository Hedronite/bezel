{
  lib,
  stdenvNoCC,
  fetchurl,
  nodejs,
  makeWrapper,
}:

# Bezel Jev router: jev-router.mjs + published @typesafe-ai/sdk.
# The program is `bezel-jev`. `jev-router` is the same program, for one transition.
# No Eli config.json, logs, or card material. TYPESAFE_API_KEY is runtime only.

let
  sdk = fetchurl {
    pname = "typesafe-ai-sdk";
    version = "0.6.0";
    url = "https://registry.npmjs.org/@typesafe-ai/sdk/-/sdk-0.6.0.tgz";
    hash = "sha512-IddX+Q0XM+VagOUZFeP7wZjaO4SHMdvnh2zEBdrZZnXedWI3BNK1lKhMx3ayrkFWvVLbVcUHJy6AVZlY+e6Jaw==";
  };
in
stdenvNoCC.mkDerivation {
  pname = "bezel-jev";
  version = "0.1.0";
  src = ./.;

  nativeBuildInputs = [
    makeWrapper
    nodejs
  ];

  dontConfigure = true;
  dontBuild = true;

  installPhase = ''
    runHook preInstall

    mkdir -p $out/lib/jev-router/node_modules/@typesafe-ai/sdk
    cp jev-router.mjs policy.mjs facts.mjs check.mjs loop-stop.mjs permission.mjs catalog.mjs calls.mjs harness.mjs package.json $out/lib/jev-router/
    cp -r testdata $out/lib/jev-router/testdata
    tar -xzf ${sdk} -C $out/lib/jev-router/node_modules/@typesafe-ai/sdk --strip-components=1

    mkdir -p $out/bin
    makeWrapper ${lib.getExe nodejs} $out/bin/bezel-jev \
      --add-flags "$out/lib/jev-router/jev-router.mjs" \
      --prefix NODE_PATH : "$out/lib/jev-router/node_modules"
    ln -s bezel-jev $out/bin/jev-router
    # Secrets: never --set TYPESAFE_API_KEY / getEnv at build.

    runHook postInstall
  '';

  meta = {
    description = "Bezel Jev router: Choice gate, tool catalog, and shadow typed calls (runtime key)";
    mainProgram = "bezel-jev";
    license = lib.licenses.mit;
    platforms = lib.platforms.unix;
  };
}
