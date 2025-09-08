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
    uniqueStrings
    ;
  cfg = config.jellyfin-tui;
  mapping =

    lib.types.mkOptionType rec {
      name = "single keybind mapping";
      description = "either a string or nested map";
      descriptionClass = "conjunction";
      check = val: builtins.isString val || builtins.isAttrs val;
      merge = {
        __functor =
          self: loc: defs:
          (self.v2 { inherit loc defs; }).value;
        v2 =

          { loc, defs }:
          let
            hasStr = builtins.any ({ value, ... }: builtins.isString value);
            pushPosition = map (
              { file, value }:
              builtins.mapAttrs (_: v: {
                inherit file;
                value = v;
              }) value
            );
            checkDefsForError =
              check: loc: defs:
              let
                invalidDefs = builtins.filter (def: !check def.value) defs;
              in
              if invalidDefs != [ ] then
                { message = "Definition values: ${lib.options.showDefs invalidDefs}"; }
              else
                null;

          in
          {
            headError = checkDefsForError check loc defs;

          };

      };
    };

  map_type = types.submodule {
    name = types.str;
    template = mkOption {
      type = types.listOf types.str;
      default = [ ];
      description = "templates that should be inherited from";
    };
    binds = mkOption {
      type = types.attrsOf (types.either types.str map_type);
      default = { };
      description = "keybind bindings";
    };
  };
  bind_type = types.submodule {
    template = mkOption {
      type = types.listOf types.str;
      default = [ ];
      description = "templates that should be inherited from";
    };
    binds = mkOption {
      type = types.attrsOf (types.either types.str map_type);
      default = { };
      description = "keybind bindings";
    };
  };
  mergeMap =
    let
      mapBind = _: val: if builtins.isString val then val else mergeMap val;
    in
    {
      name,
      template,
      binds,
    }:
    let
      b = builtins.mapAttrs mapBind binds;
    in
    b // { inherit name template; };
  mergeBinds =
    let
      mapBind = _: val: if builtins.isString val then val else mergeMap val;
    in
    { template, binds }:
    let
      b = builtins.mapAttrs mapBind binds;
    in
    b // { inherit template; };
  mapKeybinds =
    {
      help_prefixes,
      bindings,
      template,
    }:
    let
      m = builtins.mapAttrs (_: val: mergeBinds val);
      b = m bindings;
      t = m template;
    in
    b
    // {
      inherit help_prefixes;
      template = t;
    };
  keybinds = mapKeybinds cfg.keybinds;
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
    defaultKeybinds = mkEnableOption "enable default keybinds";
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
    keybinds = {
      help_prefixes = mkOption {
        type = types.listOf types.str;
        default = [ ];
        description = "prefixes for keybind help";
      };
      bindings = mkOption {
        type = types.attrsOf bind_type;
        default = { };
        description = "main bindings";
      };
      template = mkOption {
        type = types.attrsOf bind_type;
        default = { };
        description = "template definitions";
      };
    };
  };
  config = mkMerge [
    {
      jellyfin-tui = {
        enable = mkDefault true;
        defaultKeybinds = mkDefault true;
      };
    }
    (mkIf cfg.enable {
      home.packages = [ cfg.package ];
      xdg.configFile = {
        "jellyfin-tui-rs/config.toml".source = pkgs.writers.writeTOML "config.toml" cfg.config;
        "jellyfin-tui-rs/keybinds.toml".source = jellyfin-tui.checkKeybinds keybinds;
      };
    })
    (mkIf cfg.defaultKeybinds {
      jellyfin-tui.keybinds = mkDefault (
        let
          quit = {
            binds.q = "quit";
          };
        in
        {
          help_prefixes = [
            "?"
            "esc"
          ];
          bindings = {
            fetch = quit;
            play_mpv = quit;
            user_view = {
              template = [
                "ud"
                "q"
                "o"
              ];
              binds = {
                r = "reload";
                left = "prev";
                back-tab = "prev";
                right = "next";
                tab = "next";
                enter = "play";
              };
            };
            home_screen = {
              template = [
                "m"
                "o"
              ];
              binds = {
                r = "reload";
                enter = "play-open";
              };
            };
            item_list_details = {
              template = [
                "m"
                "o"
              ];
              binds.enter = "play";
            };
            login_info.binds = {
              backspace = "delete";
              enter = "submit";
              up = "next";
              tab = "next";
              down = "prev";
              back-tab = "prev";
              q = "quit";
            };
            error = {
              template = [ "m" ];
              binds.k = "kill";
            };
            item_details = {
              template = [
                "ud"
                "q"
              ];
              binds = {
                p = "play";
                enter = "play";
              };
            };
          };
          template = {
            q = quit;
            ud.binds = {
              up = "up";
              down = "down";
            };
            lr.binds = {
              left = "left";
              right = "right";
            };
            m.template = [
              "q"
              "ud"
              "lr"
            ];
            o.binds = {
              o = "open";
              p = "play";
              O = {
                name = "open-";
                binds = {
                  e = "open-episode";
                  S = "open-season";
                  s = "open-series";
                };
              };
            };
          };
        }
      );
    })
  ];
}
