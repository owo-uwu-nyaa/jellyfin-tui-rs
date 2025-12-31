{
  lib,
  pkg-config,
  mpv-unwrapped,
  rustPlatform,
  sqlite,
  rust-build,
  runCommand,
  remarshal,
  stdenv,
  attach ? false,
  mpris ? stdenv.hostPlatform.isLinux,
}:
let
  fileset = lib.fileset.unions [
    (lib.fileset.fileFilter (
      file: file.hasExt "rs" || file.name == "Cargo.toml" || file.name == "Cargo.lock"
    ) ./.)
    ./.sqlx
    ./config/config.toml
    ./config/keybinds.toml
    ./migrations
    ./jellyhaj.desktop
    ./libmpv-rs/test-data
  ];

  src = lib.fileset.toSource {
    root = ./.;
    inherit fileset;
  };
  jellyhaj =
    let
      checkKeybinds =
        keybinds:
        runCommand "keybinds.toml"
          {
            nativeBuildInputs = [
              remarshal
              jellyhaj
            ];
            value = builtins.toJSON keybinds;
            passAsFile = [ "value" ];
          }
          ''
            json2toml "$valuePath" "$out"
            jellyhaj check-keybinds "$out"
          '';
    in
    (
      (rust-build.withCrateOverrides {
        libmpv-sys = {
          buildInputs = [ mpv-unwrapped ];
          nativeBuildInputs = [
            pkg-config
            rustPlatform.bindgenHook
          ];
        };
        libsqlite3-sys =
          {
            buildInputs = [ sqlite ];
            nativeBuildInputs = [
              pkg-config
              rustPlatform.bindgenHook
            ];
          };
      }).build
      {
        inherit src;
        pname = "jellyhaj";
        version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.version;
        features = (lib.optional attach "attach") ++ (lib.optional mpris "mpris");
      }
    ).overrideAttrs
      (
        _: prev: {
          passthru = (prev.passthru or { }) // {
            inherit checkKeybinds;
          };
          postBuild = lib.optionalString stdenv.hostPlatform.isLinux ''
          install -Dm644 $src/jellyhaj.desktop $out/share/applications/jellyhaj.desktop       
'';
          meta = (prev.meta or { }) // {
            mainProgramm = "jellyhaj";
          };
        }
      );
in
jellyhaj
