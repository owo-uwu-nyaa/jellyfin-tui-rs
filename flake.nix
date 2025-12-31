{
  description = "jellyfin terminal ui in nice";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    systems.url = "github:nix-systems/default";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils = {
      url = "github:numtide/flake-utils";
      inputs.systems.follows = "systems";
    };
    nix-rust-build = {
      url = "github:RobinMarchart/nix-rust-build";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.systems.follows = "systems";
      inputs.flake-utils.follows = "flake-utils";
    };
  };

  outputs =
    {
      nixpkgs,
      rust-overlay,
      flake-utils,
      nix-rust-build,
      ...
    }:
    (
      flake-utils.lib.eachDefaultSystem (
        system:
        let
          overlays = [
            rust-overlay.overlays.default
            nix-rust-build.overlays.default
          ];
          pkgs = import nixpkgs {
            inherit system overlays;
          };
          jellyhaj = pkgs.callPackage ./jellyhaj.nix { };
          jellyhaj-incremental = pkgs.callPackage ./jellyhaj-incremental.nix { };
        in
        {
          formatter = pkgs.nixfmt-tree;
          packages = {
            default = jellyhaj;
            inherit jellyhaj jellyhaj-incremental;
          };
          checks = {
            format =
              pkgs.runCommand "nixfmt-check-jellyhaj"
                {
                  nativeBuildInputs = [
                    pkgs.cargo
                    pkgs.rustfmt
                  ];
                }
                ''
                  cd ${./.}
                  cargo fmt --check
                  touch $out
                '';
          };
          apps = {
            default = {
              type = "app";
              program = "${jellyhaj}/bin/jellyhaj";
            };
          };
          devShells = {
            default = pkgs.mkShell {
              nativeBuildInputs = [
                pkgs.cargo-nextest
                pkgs.cargo-audit
                pkgs.cargo-expand
                pkgs.rust-bin.nightly.latest.rust-analyzer
                (pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml)
                pkgs.sqlx-cli
                pkgs.pkg-config
                pkgs.sqlite-interactive
                pkgs.rustPlatform.bindgenHook
              ];
              buildInputs = [
                pkgs.mpv-unwrapped
                pkgs.sqlite
              ];
              DATABASE_URL = "sqlite://db.sqlite";
            };
          };
        }
      )
      // (
        let
          jellyhaj = final: prev: {
            jellyhaj = final.callPackage ./jellyhaj.nix { };
          };
        in
        {
          overlays = {
            inherit jellyhaj;
            default = jellyhaj;
          };
          hmModules = {
            default = import ./hm-module.nix nix-rust-build;
          };
        }
      )
    );
}
