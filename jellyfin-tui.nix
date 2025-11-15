{
  lib,
  pkg-config,
  mpv-unwrapped,
  rustPlatform,
  sqlite,
  rust-build,
  runCommand,
  remarshal,
  attach ? false,
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
    ./libmpv-rs/test-data
  ];

  src = lib.fileset.toSource {
    root = ./.;
    inherit fileset;
  };
  jellyfin-tui =
    let
      checkKeybinds =
        keybinds:
        runCommand "keybinds.toml"
          {
            nativeBuildInputs = [
              remarshal
              jellyfin-tui
            ];
            value = builtins.toJSON keybinds;
            passAsFile = [ "value" ];
          }
          ''
            json2toml "$valuePath" "$out"
            jellyfin-tui check-keybinds "$out"
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
        pname = "jellyfin-tui";
        version = "0.1.0";
        features = lib.optional attach "attach";
      }
    ).overrideAttrs
      (
        _: prev: {
          passthru = (prev.passthru or { }) // {
            inherit checkKeybinds;
          };
        }
      );
in
jellyfin-tui
