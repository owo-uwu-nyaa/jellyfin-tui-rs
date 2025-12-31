{
  lib,
  stdenv,
  fetchFromGitHub,
  rustPlatform,
  pkg-config,
  mpv-unwrapped,
  sqlite,
  versionCheckHook,
  withMpris ? stdenv.isLinux, # enable media player dbus interface
}:
let
  fileset = lib.fileset.unions [
    (lib.fileset.fileFilter (
      file: file.hasExt "rs" || file.name == "Cargo.toml" || file.name == "Cargo.lock"
    ) ./.)
    ./.sqlx
    ./config/config.toml
    ./config/keybinds.toml
    ./migrations
    ./jellyhaj.desktop
    ./libmpv-rs/test-data
  ];
  src = lib.fileset.toSource {
    root = ./.;
    inherit fileset;
  };
in
rustPlatform.buildRustPackage {
  pname = "jellyhaj";
  version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.version;
  inherit src;
  cargoLock = {
    lockFile = ./Cargo.lock;
  };
  nativeBuildInputs = [
    rustPlatform.bindgenHook
    pkg-config
  ];
  buildInputs = [
    sqlite
    mpv-unwrapped
  ];
  postBuild = lib.optionalString stdenv.hostPlatform.isLinux ''
    install -Dm644 $src/jellyhaj.desktop $out/share/applications/jellyhaj.desktop       
  '';
  nativeInstallCheckInputs = [ versionCheckHook ];
  versionCheckProgramArg = "--version";
  doInstallCheck = true;
  checkFlags = [
    #some tests need internet access
    "--skip=tests::properties"
    "--skip=tests::node_map"
    "--skip=tests::events"
  ];
  cargoTestFlags = [ "--workspace" ];
  buildFeatures = lib.optional withMpris "mpris";

  meta = {
    description = "Terminal client for Jellyfin trying to reimplement parts of the web ui";
    license = lib.licenses.mit;
    sourceProvenance = [ lib.sourceTypes.fromSource ];
    mainProgram = "jellyhaj";
    homepage = "https://github.com/owo-uwu-nyaa/jellyhaj";
    maintainers = with lib.maintainers; [ RobinMarchart ];
  };
}
