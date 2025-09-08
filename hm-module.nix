nix-rust-build:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  inherit (lib)
    mkEnableOption
    mkIf
    mkOption
    types
    mkDefault
    mkMerge
    ;
  cfg = config.jellyfin-tui;
  jellyfin-tui = (pkgs.extend nix-rust-build.overlays.default).callPackage ./jellyfin-tui.nix { };

in
{
  options.jellyfin-tui = {
    enable = mkEnableOption "enable jellyfin tui";
    package = mkOption {
      type = types.package;
      default = jellyfin-tui;
      description = "package with jellyfin-tui";
    };
    config = {
      mpv_profile = mkOption {
        type = types.str;
        default = "default";
        description = "mpv profile to inherit from";
      };
      hwdec = mkOption {
        type = types.str;
        default = "auto-safe";
        description = "hardware decoding";
      };
      mpv_log_level = mkOption {
        type = types.str;
        default = "info";
        description = "mpv log level, separate from general log level";
      };
      login_file = mkOption {
        type = types.path;
        default = "${config.xdg.configHome}/jellyfin-tui-rs/login.toml";
        description = "login file";
      };
    };
    keybinds = mkOption {
      type = types.attrsOf types.anything;
      default = builtins.fromTOML (builtins.readFile ./config/keybinds.toml);
      description = "prefixes for keybind help";
    };
  };
  config = mkMerge [
    {
      jellyfin-tui = {
        enable = mkDefault true;
      };
    }
    (mkIf cfg.enable {
      home.packages = [ cfg.package ];
      xdg.configFile = {
        "jellyfin-tui-rs/config.toml".source = pkgs.writers.writeTOML "config.toml" cfg.config;
        "jellyfin-tui-rs/keybinds.toml".source = jellyfin-tui.checkKeybinds cfg.keybinds;
      };
    })
  ];
}
