{
  description = "jellyfin terminal ui in nice";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    systems.url = "github:nix-systems/default-linux";
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
          jellyfin-tui-rs = pkgs.callPackage ./jellyfin-tui.nix { };
        in
        {
          formatter = pkgs.nixfmt-tree;
          packages = {
            default = jellyfin-tui-rs;
            inherit jellyfin-tui-rs;
          };
          apps = {
            default = {
              type = "app";
              program = "${jellyfin-tui-rs}/bin/jellyfin-tui-rs";
            };
          };
          devShells.default =
            pkgs.mkShell.override
              {
                stdenv = pkgs.stdenvAdapters.useMoldLinker pkgs.clangStdenv;
              }
              {
                buildInputs = [
                  pkgs.cargo-nextest
                  pkgs.cargo-audit
                  pkgs.cargo-expand
                  pkgs.rust-bin.nightly.latest.rust-analyzer
                  (pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml)
                  pkgs.sqlx-cli
                  pkgs.pkg-config
                  pkgs.mpv-unwrapped
                  pkgs.sqlite-interactive
                  pkgs.sqlite
                  pkgs.rustPlatform.bindgenHook
                ];
                DATABASE_URL = "sqlite://db.sqlite";
              };
        }
      )
      // (
        let
          jellyfin-tui-rs = final: prev: {
            jellyfin-tui-rs = final.callPackage ./jellyfin-tui.nix { };
          };
        in
        {
          overlays = {
            inherit jellyfin-tui-rs;
            default = jellyfin-tui-rs;
          };
          hmModules = {
            default = import ./hm-module.nix nix-rust-build;
          };
        }
      )
    );
}
