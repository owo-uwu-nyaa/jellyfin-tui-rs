{
  lib,
  pkg-config,
  mpv-unwrapped,
  rustPlatform,
  sqlite,
  rust-build,
  use_bindgen ? false,
  bundle_sqlite ? false,
}:
let
  fileset_src =
    base:
    lib.fileset.unions [
      (lib.fileset.maybeMissing (base + "/Cargo.lock"))
      (base + "/Cargo.toml")
      (base + "/src")
      (lib.fileset.maybeMissing (base + "/build.rs"))
    ];
  fileset = lib.fileset.unions [
    (fileset_src ./.)
    ./.sqlx
    ./migrations
    ./config
    (fileset_src ./jellyfin-rs)
    (fileset_src ./libmpv-rs)
    ./libmpv-rs/test-data
    (fileset_src ./libmpv-rs/libmpv-sys)
    (fileset_src ./keybinds-derive)
    (fileset_src ./keybinds-derive-impl)
    (fileset_src ./keybinds)
  ];

  src = lib.fileset.toSource {
    root = ./.;
    inherit fileset;
  };
in
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
