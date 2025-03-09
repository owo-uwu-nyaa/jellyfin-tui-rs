{
  lib,
  rustPlatform,
  pkg-config,
  openssl,
  mpv,
  sqlite,
  use_bindgen ? true,
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
    (fileset_src ./jellyfin-rs)
    (fileset_src ./libmpv-rs)
    ./libmpv-rs/test-data
    (fileset_src ./libmpv-rs/libmpv-sys)
  ];

  src = lib.fileset.toSource {
    root = ./.;
    inherit fileset;
  };
in
rustPlatform.buildRustPackage {
  inherit src;
  pname = "jellyfin-tui";
  version = "0.1.0";
  cargoLock.lockFile = ./Cargo.lock;
  cargoTestFlags = [
    # run in wokspace
    "--workspace"
    # skip tests failing in sandbox
    "--"
    "--skip"
    "tests::events"
    "--skip"
    "tests::node_map"
    "--skip"
    "tests::properties"
  ];
  buildNoDefaultFeatures = true;
  nativeBuildInputs = [
    pkg-config
  ] ++ (lib.optionals use_bindgen [ rustPlatform.bindgenHook ]);
  buildInputs = [
    openssl
    mpv
  ] ++ (lib.optionals (!bundle_sqlite) [ sqlite ]);
  buildFeatures =
    (lib.optionals use_bindgen [ "use-bindgen" ])
    ++ (if bundle_sqlite then [ "sqlite-bundled" ] else [ "sqlite-unbundled" ]);
  SQLX_OFFLINE = "true";
}
