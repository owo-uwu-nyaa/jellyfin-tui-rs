{
  pkgs,
  rustc,
  cargo,
  lib,
  pkg-config,
  mpv,
  sqlite,
  clang,
  libclang,
  generatedCargoNix,
  use_bindgen ? true,
  bundle_sqlite ? false,
}:
let
  env =
    {
    }
    // (lib.optionalAttrs (use_bindgen || (!bundle_sqlite)) {
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
    ./migrations
    ./.sqlx
    (fileset_src ./jellyfin-rs)
    (fileset_src ./libmpv-rs)
    ./libmpv-rs/test-data
    (fileset_src ./libmpv-rs/libmpv-sys)
  ];
  c2n_src = generatedCargoNix {
    name = "jellyfin-tui";
    src = lib.fileset.toSource {
      root = ./.;
      fileset = fileset;
    };
  };
  c2n = import c2n_src {
    nixpkgs = "";
    inherit pkgs;
    buildRustCrateForPkgs =
      crate:
      pkgs.buildRustCrate.override {
        rustc = rustc;
        cargo = cargo;
        defaultCrateOverrides = pkgs.defaultCrateOverrides // {
          libmpv-sys =
            attrs:
            {
              nativeBuildInputs = [ pkg-config ];
              buildInputs = [
                mpv
              ] ++ (lib.optionals use_bindgen [ clang ]);
            }
            // (lib.attrsets.optionalAttrs use_bindgen {
              LIBCLANG_PATH = "${libclang.lib}/lib";
            });
          rav1e = attrs: {
            CARGO_ENCODED_RUSTFLAGS = "not set";
          };
          libsqlite3-sys =
            attrs:
            lib.attrsets.optionalAttrs (!bundle_sqlite) {
              LIBCLANG_PATH = "${libclang.lib}/lib";
              nativeBuildInputs = [
                clang
              ];
              buildInputs = [ sqlite ];
            };
          sqlx-macros =
            attrs:
            lib.attrsets.optionalAttrs (!bundle_sqlite) {
              buildInputs = [ sqlite ];
            };
          jellyfin-tui =
            attrs:
            lib.attrsets.optionalAttrs (!bundle_sqlite) {
              passthru = { inherit env; };
              buildInputs = [ sqlite ];
            };
        };
      };
  };
  features =
    (lib.optionals use_bindgen [ "use-bindgen" ])
    ++ (lib.optionals bundle_sqlite [ "sqlite-bundled" ])
    ++ (lib.optionals (!bundle_sqlite) [ "sqlite-unbundled" ]);
in
c2n.workspaceMembers.jellyfin-tui.build.override {
  inherit features;
}
