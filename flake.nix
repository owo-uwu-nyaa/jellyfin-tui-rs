{
  description = "jellyfin terminal ui in nice";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    systems.url = "github:nix-systems/default-linux";
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
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
      crane,
      rust-overlay,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        inherit (pkgs) lib;
        craneLib = (crane.mkLib pkgs).overrideToolchain (
          p: p.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml
        );
        src = lib.cleanSourceWith {
          src = ./.;
          filter = craneLib.filterCargoSources;
          name = "jellyfin-tui-src";
        };
        commonArgs = {
          strictDeps = true;
          inherit src;
          nativeBuildInputs = with pkgs; [
            pkg-config
          ];
          buildInputs = with pkgs; [
            openssl
            mpv
          ];
        };
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
        jellyfin-tui = craneLib.buildPackage (commonArgs // { inherit cargoArtifacts; });
      in
      {
        checks = {
          inherit jellyfin-tui;
          clippy = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "-- --deny warnings";
            }
          );
          doc = craneLib.cargoDoc (commonArgs // { inherit cargoArtifacts; });
          fmt = craneLib.cargoFmt { inherit src; };
          nextest = craneLib.cargoNextest (
            commonArgs
            // {
              inherit cargoArtifacts;
              partitions = 1;
              partitionType = "count";
            }
          );
        };
        packages = {
          default = jellyfin-tui;
          inherit jellyfin-tui;
        };
        devShells.default = craneLib.devShell {
          checks = self.checks.${system};
        };
      }
    );
}
