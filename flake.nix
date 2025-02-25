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
    crate2nix = {
      url = "github:nix-community/crate2nix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.crate2nix_stable.follows = "crate2nix";
    };
  };

  outputs =
    {
      nixpkgs,
      rust-overlay,
      flake-utils,
      crate2nix,
      ...
    }:
    (
      flake-utils.lib.eachDefaultSystem (
        system:
        let
          overlays = [
            rust-overlay.overlays.default
          ];
          pkgs = import nixpkgs {
            inherit system overlays;
          };
          toolchain_dev = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          jellyfin-tui = pkgs.callPackage ./jellyfin-tui.nix {
            inherit (crate2nix.tools.${system}) generatedCargoNix;
          };
          jellyfin-tui-rust-overlay = jellyfin-tui.override {
            rustc = toolchain_dev;
            cargo = toolchain_dev;
          };
        in
        {
          packages = {
            default = jellyfin-tui;

            inherit jellyfin-tui jellyfin-tui-rust-overlay;
          };
          devShells.default =
            pkgs.mkShell.override
              {
                stdenv = pkgs.stdenvAdapters.useMoldLinker pkgs.clangStdenv;
              }
              (
                {
                  inputsFrom = [
                    jellyfin-tui-rust-overlay
                  ];
                  buildInputs = [
                    pkgs.cargo-nextest
                    pkgs.cargo-audit
                    pkgs.rust-bin.nightly.latest.rust-analyzer
                    pkgs.sqlx-cli
                  ] ++ jellyfin-tui-rust-overlay.shellDeps;
                  DATABASE_URL = "sqlite://db.sqlite";
                }
                // jellyfin-tui-rust-overlay.env
              );
        }
      )
      // (
        let
          jellyfin-tui = final: prev: {
            jellyfin-tui = final.callPackage ./jellyfin-tui.nix {
              inherit (crate2nix.tools.${final.system}) generatedCargoNix;
            };
          };
        in
        {
          overlays = {
            inherit jellyfin-tui;
            default = jellyfin-tui;
          };
        }
      )
    );
}
