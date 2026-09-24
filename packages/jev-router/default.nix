{
  lib,
  stdenv,
  rustc,
  cargo,
  git,
}:

# Router package: the bezel-bridle binary. No JavaScript grader.
# TYPESAFE_API_KEY is runtime only.

let
  repoRoot = ../..;
in
stdenv.mkDerivation {
  pname = "jev-router";
  version = "0.2.1";
  src = ./.;

  nativeBuildInputs = [
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
    mkdir -p "$work/crates/bezel-bridle" "$work/packages/jev-router" "$work/docs" "$work/skills-stub"
    cp ${repoRoot}/crates/bezel-bridle/Cargo.toml ${repoRoot}/crates/bezel-bridle/Cargo.lock "$work/crates/bezel-bridle/"
    cp -r ${repoRoot}/crates/bezel-bridle/src "$work/crates/bezel-bridle/src"
    cp -r ${repoRoot}/packages/jev-router/testdata "$work/packages/jev-router/testdata"
    cp ${repoRoot}/docs/POLICY-MAP.md "$work/docs/POLICY-MAP.md"
    cp ${repoRoot}/packages/cursor-agent-jev.nix "$work/packages/cursor-agent-jev.nix"
    cp ${repoRoot}/packages/bezel-planes-shim.sh "$work/packages/bezel-planes-shim.sh"
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

    mkdir -p $out/bin
    # Secrets: never --set TYPESAFE_API_KEY / getEnv at build.
    install -m 755 "$BRIDLE_BIN" $out/bin/bezel-bridle
    if head -c 80 "$out/bin/bezel-bridle" | grep -q node; then
      echo "bezel-bridle must be the Rust binary" >&2
      exit 1
    fi
    cat > $out/bin/jev-router << EOF
#!/bin/sh
exec $out/bin/bezel-bridle "\$@"
EOF
    chmod 755 $out/bin/jev-router
    if grep -q jev-router-node $out/bin/jev-router; then
      echo "jev-router must not exec jev-router-node" >&2
      exit 1
    fi
    if [ -e $out/bin/jev-router-node ]; then
      echo "jev-router-node must not be installed" >&2
      exit 1
    fi
    if find $out -name '*.mjs' | grep -q .; then
      echo "jev-router must not install JavaScript" >&2
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
