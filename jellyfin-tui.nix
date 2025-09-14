{
  lib,
  pkg-config,
  mpv-unwrapped,
  rustPlatform,
  sqlite,
  rust-build,
  runCommand,
  remarshal,
  use_bindgen ? false,
  bundle_sqlite ? false,
}:
let
  fileset = lib.fileset.unions [
    (lib.fileset.fileFilter (file: file.hasExt "rs"|| file.name == "Cargo.toml" || file.name == "Cargo.lock") ./. )
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
          ]
          ++ (if use_bindgen then [ rustPlatform.bindgenHook ] else [ ]);
        };
        libsqlite3-sys =
          if !bundle_sqlite then
            {
              buildInputs = [ sqlite ];
              nativeBuildInputs = [
                pkg-config
                rustPlatform.bindgenHook
              ];
            }
          else
            { };
      }).build
      {
        inherit src;
        pname = "jellyfin-tui";
        version = "0.1.0";
        noDefaultFeatures = true;
        features =
          (lib.optionals use_bindgen [ "use-bindgen" ])
          ++ (if bundle_sqlite then [ "sqlite-bundled" ] else [ "sqlite-unbundled" ]);
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
