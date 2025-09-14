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
          jellyfin-tui = pkgs.callPackage ./jellyfin-tui.nix { };
        in
        {
          formatter = pkgs.nixfmt-tree;
          packages = {
            default = jellyfin-tui;
            inherit jellyfin-tui;
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
                  pkgs.sqlite
                  pkgs.mpv-unwrapped
                  pkgs.rustPlatform.bindgenHook
                ];
                DATABASE_URL = "sqlite://db.sqlite";
              };
        }
      )
      // (
        let
          jellyfin-tui = final: prev: {
            jellyfin-tui = final.callPackage ./jellyfin-tui.nix { };
          };
        in
        {
          overlays = {
            inherit jellyfin-tui;
            default = jellyfin-tui;
          };
          hmModules = {
            default = import ./hm-module.nix nix-rust-build;
          };
        }
      )
    );
}
