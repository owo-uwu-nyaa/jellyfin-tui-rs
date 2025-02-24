{
  lib,
  rustPlatform,
  pkg-config,
  openssl,
  mpv,
  clang,
  libclang,
  sqlite,
  use_bindgen ? false,
}:
let
  env =
    {
      DATABASE_URL = "sqlite://db.sqlite";
    }
    // (lib.optionalAttrs use_bindgen {
      LIBCLANG_PATH = "${libclang.lib}/lib";
    });
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
rustPlatform.buildRustPackage (
  {
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
    nativeBuildInputs = [
      pkg-config
    ] ++ (lib.optionals use_bindgen [ clang ]);
    buildInputs = [
      openssl
      mpv
      sqlite
    ];
    buildFeatures = (lib.optionals use_bindgen [ "use-bindgen" ]);
    passthru = { inherit env; };
    SQLX_OFFLINE = "true";
  }
  // env
)
