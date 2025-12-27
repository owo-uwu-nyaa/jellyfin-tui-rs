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
    mkMerge
    mkOption
    types
    filterAttrs
    ;
  cfg = config.programs.jellyfin-tui;
  jellyfin-tui = (pkgs.extend nix-rust-build.overlays.default).callPackage ./jellyfin-tui.nix { };
in
{
  options.programs.jellyfin-tui = {
    enable = mkEnableOption "enable jellyfin tui";
    package = mkOption {
      type = types.package;
      default = jellyfin-tui;
      description = "package with jellyfin-tui";
    };
    config = {
      mpv_profile = mkOption {
        type = types.enum [
          "fast"
          "high-quality"
          "default"
        ];
        default = "default";
        description = "mpv profile to inherit from";
      };
      hwdec = mkOption {
        type = types.str;
        default = "auto-safe";
        description = "hardware decoding";
      };
      mpv_log_level = mkOption {
        type = types.enum [
          "no"
          "fatal"
          "error"
          "warn"
          "info"
          "v"
          "debug"
          "trace"
        ];
        default = "info";
        description = "mpv log level, separate from general log level";
      };
      login_file = mkOption {
        type = types.path;
        default = "${config.xdg.configHome}/jellyfin-tui-rs/login.toml";
        description = "login file";
      };
      keybinds_file = mkOption {
        type = types.nullOr types.path;
        default = null;
      };
      mpv_config_file = mkOption {
        type = types.nullOr types.path;
        default = null;
      };
    };
    keybinds = mkOption {
      type = types.nullOr (types.attrsOf types.anything);
      default = null;
      description = "keybind configuration";
    };
    login = mkOption {
      type = lib.types.nullOr (
        lib.types.submodule {
          options = {
            server_url = mkOption {
              type = lib.types.str;
            };
            username = mkOption {
              type = lib.types.str;
            };
            password = mkOption {
              type = lib.types.str;
              default = "";
            };
            password_cmd = mkOption {
              type = lib.types.nullOr (lib.types.listOf lib.types.str);
              default = null;
            };
          };
        }
      );
      default = null;
    };
    default = "${config.xdg.configHome}/jellyfin-tui-rs/keybinds.toml";
  };
  config = mkMerge [
    (mkIf cfg.enable {
      home.packages = [ cfg.package ];
      xdg.configFile = {
        "jellyfin-tui-rs/config.toml".source = pkgs.writers.writeTOML "config.toml" (
          filterAttrs (_: v: !isNull v) cfg.config
        );
      };
    })
    (mkIf (! isNull cfg.keybinds) {
      programs.jellyfin-tui.config.keybinds_file = jellyfin-tui.checkKeybinds cfg.keybinds;
    }) 
  ];
}
