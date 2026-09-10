{
  lib,
  stdenv,
  runCommand,
  zig_0_16,
  zigDepsHash ? "sha256-HWfTcAUFL4vZJDg242y5R3zQY6fDshKk6VMn+qgSNi4=",
}:

let
  zig = zig_0_16;
in
stdenv.mkDerivation (finalAttrs: {
  pname = "htmlua";
  version = "1.0.0-alpha";

  src = lib.fileset.toSource {
    root = ./.;
    fileset = lib.fileset.unions [
      ./build.zig
      ./build.zig.zon
      ./src
      ./examples
      ./LICENSE
      ./README.md
    ];
  };

  nativeBuildInputs = [ zig ];

  zigDeps =
    runCommand "${finalAttrs.pname}-${finalAttrs.version}-zig-deps"
      {
        inherit (finalAttrs) src;
        nativeBuildInputs = [ zig ];
        outputHashMode = "recursive";
        outputHashAlgo = "sha256";
        outputHash = zigDepsHash;
      }
      ''
        export ZIG_GLOBAL_CACHE_DIR="$TMPDIR/zig-cache"
        mkdir -p "$ZIG_GLOBAL_CACHE_DIR/tmp"
        runHook unpackPhase
        cd "$sourceRoot"

        for attempt in 1 2 3; do
          if zig build --fetch; then
            mv "$ZIG_GLOBAL_CACHE_DIR/p" "$out"
            exit 0
          fi
          echo "Zig dependency fetch attempt $attempt failed" >&2
        done
        exit 1
      '';

  postConfigure = ''
    ln -s ${finalAttrs.zigDeps} "$ZIG_GLOBAL_CACHE_DIR/p"
  '';

  doCheck = true;

  meta = {
    description = "An HTML templating engine with (not yet reimplemented) Lua scripting abilities";
    license = lib.licenses.mit;
    mainProgram = "htmlua";
    platforms = lib.platforms.unix;
  };
})
