{
  lib,
  stdenv,
  fetchurl,
  nodejs,
  makeWrapper,
  rustc,
  cargo,
  git,
}:

# Router-only package: jev-router.mjs + published @typesafe-ai/sdk.
# No Eli config.json, logs, or card material. TYPESAFE_API_KEY is runtime only.

let
  sdk = fetchurl {
    pname = "typesafe-ai-sdk";
    version = "0.6.0";
    url = "https://registry.npmjs.org/@typesafe-ai/sdk/-/sdk-0.6.0.tgz";
    hash = "sha512-IddX+Q0XM+VagOUZFeP7wZjaO4SHMdvnh2zEBdrZZnXedWI3BNK1lKhMx3ayrkFWvVLbVcUHJy6AVZlY+e6Jaw==";
  };
  repoRoot = ../..;
in
stdenv.mkDerivation {
  pname = "jev-router";
  version = "0.2.1";
  src = ./.;

  nativeBuildInputs = [
    makeWrapper
    nodejs
    rustc
    cargo
    git
  ];

  dontConfigure = true;

  buildPhase = ''
    runHook preBuild
    export BEVEL_JS=$PWD
    export CARGO_HOME=$TMPDIR/cargo-home
    export HOME=$TMPDIR/home
    mkdir -p "$CARGO_HOME" "$HOME"
    work=$TMPDIR/bridle
    mkdir -p "$work/crates/bezel-bridle" "$work/packages/jev-router" "$work/skills-stub"
    cp ${repoRoot}/crates/bezel-bridle/Cargo.toml ${repoRoot}/crates/bezel-bridle/Cargo.lock "$work/crates/bezel-bridle/"
    cp -r ${repoRoot}/crates/bezel-bridle/src "$work/crates/bezel-bridle/src"
    cp -r ${repoRoot}/packages/jev-router/testdata "$work/packages/jev-router/testdata"
    cp -r ${repoRoot}/skills-stub/laws "$work/skills-stub/laws"
    chmod -R u+w "$work"
    echo "bezel-bridle oracle fixtures"
    export CARGO_TARGET_DIR=$work/target
    cargo test --manifest-path "$work/crates/bezel-bridle/Cargo.toml" --offline --release
    cargo build --manifest-path "$work/crates/bezel-bridle/Cargo.toml" --offline --release
    export BRIDLE_BIN=$CARGO_TARGET_DIR/release/bezel-bridle
    test -x "$BRIDLE_BIN"
    runHook postBuild
  '';

  installPhase = ''
    runHook preInstall
    cd "$BEVEL_JS"

    mkdir -p $out/lib/jev-router/node_modules/@typesafe-ai/sdk
    cp jev-router.mjs policy.mjs facts.mjs check.mjs loop-stop.mjs permission.mjs catalog.mjs calls.mjs harness.mjs package.json $out/lib/jev-router/
    cp -r testdata $out/lib/jev-router/testdata
    tar -xzf ${sdk} -C $out/lib/jev-router/node_modules/@typesafe-ai/sdk --strip-components=1

    mkdir -p $out/bin
    # Live commands stay on the JavaScript client. Offline commands run the Rust binary.
    makeWrapper ${lib.getExe nodejs} $out/bin/jev-router-node \
      --add-flags "$out/lib/jev-router/jev-router.mjs" \
      --prefix NODE_PATH : "$out/lib/jev-router/node_modules"
    # Secrets: never --set TYPESAFE_API_KEY / getEnv at build.
    install -m 755 "$BRIDLE_BIN" $out/bin/bezel-bridle
    if head -c 80 "$out/bin/bezel-bridle" | grep -q node; then
      echo "bezel-bridle must be the Rust binary" >&2
      exit 1
    fi
    cat > $out/bin/jev-router << EOF
#!/bin/sh
for arg in "\$@"; do
  case "\$arg" in
    hook|--catalog|--schema|--schema-dump|--check)
      exec $out/bin/bezel-bridle "\$@"
      ;;
  esac
done
exec $out/bin/jev-router-node "\$@"
EOF
    chmod 755 $out/bin/jev-router

    export NODE_PATH=$out/lib/jev-router/node_modules
    export JEV_MODE=shadow
    unset TYPESAFE_API_KEY || true
    fixtures=${repoRoot}/packages/jev-router/testdata
    match_node() {
      set +e
      ${lib.getExe nodejs} $out/lib/jev-router/jev-router.mjs "$@" > node.out
      node_code=$?
      $out/bin/jev-router "$@" > rust.out
      rust_code=$?
      set -e
      if [ "$node_code" != "$rust_code" ] || ! cmp -s node.out rust.out; then
        echo "jev-router $* does not match node ($node_code vs $rust_code)" >&2
        echo "--- node ---" >&2
        cat node.out >&2
        echo "--- rust ---" >&2
        cat rust.out >&2
        exit 1
      fi
    }
    match_node --catalog
    match_node --schema Bash
    match_node --schema
    match_node --check --diff-file "$fixtures/empty.diff"
    match_node --check --diff-file "$fixtures/skip-marker.diff"
    set +e
    ${lib.getExe nodejs} $out/lib/jev-router/jev-router.mjs hook < "$fixtures/grok-pretool-bash.json" > node-hook.out
    node_hook=$?
    $out/bin/jev-router hook < "$fixtures/grok-pretool-bash.json" > rust-hook.out
    rust_hook=$?
    set -e
    if [ "$node_hook" != "$rust_hook" ] || ! cmp -s node-hook.out rust-hook.out; then
      echo "jev-router hook does not match node ($node_hook vs $rust_hook)" >&2
      echo "--- node ---" >&2
      cat node-hook.out >&2
      echo "--- rust ---" >&2
      cat rust-hook.out >&2
      exit 1
    fi
    if grep -q '"decision":"allow"' rust.out rust-hook.out; then
      echo "offline jev-router must not allow" >&2
      exit 1
    fi

    runHook postInstall
  '';

  meta = {
    description = "Router-only Jev Choice gate, tiny tool catalog, and shadow typed calls (runtime key)";
    mainProgram = "jev-router";
    license = lib.licenses.mit;
    platforms = lib.platforms.unix;
  };
}
