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
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
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
          platform_dev = pkgs.makeRustPlatform {
            rustc = toolchain_dev;
            cargo = toolchain_dev;
          };
          jellyfin-tui = pkgs.callPackage ./jellyfin-tui.nix { };
        in
        {
          packages = {
            default = jellyfin-tui;
            inherit jellyfin-tui;
          };
          devShells.default =
            let
              crate = jellyfin-tui.override {
                rustPlatform = platform_dev;
                use_bindgen = true;
              };
            in
            pkgs.mkShell.override
              {
                stdenv = pkgs.stdenvAdapters.useMoldLinker pkgs.clangStdenv;
              }
              (
                {
                  inputsFrom = [
                    crate
                  ];
                  buildInputs = [
                    pkgs.cargo-nextest
                    pkgs.cargo-audit
                    pkgs.rust-bin.nightly.latest.rust-analyzer
                    pkgs.sqlx-cli
                  ];
                }
                // crate.env
              );
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
        }
      )
    );
}
