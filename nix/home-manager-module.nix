{ config, lib, pkgs, ... }:

with lib;

let
  cfg = config.programs.console-mode;

  console-mode-pkg = pkgs.callPackage ./package.nix { };

  # Build the command line arguments based on configuration
  mkArgs = cfg:
    lib.concatStringsSep " "
    (lib.optionals (cfg.display != null) [ "--display" cfg.display ]
      ++ lib.optionals (cfg.resolution != null) [
        "--resolution"
        cfg.resolution
      ] ++ lib.optionals (cfg.refreshRate != null) [
        "--refresh-rate"
        (toString cfg.refreshRate)
      ] ++ lib.optionals cfg.forceVrr [ "--force-vrr" ]
      ++ lib.optionals cfg.forceHdr [ "--force-hdr" ]
      ++ lib.optionals cfg.noVrr [ "--no-vrr" ]
      ++ lib.optionals cfg.noHdr [ "--no-hdr" ]
      ++ lib.optionals cfg.safeMode [ "--safe-mode" ]
      ++ lib.optionals (cfg.gamescopeBin != null) [
        "--gamescope-bin"
        cfg.gamescopeBin
      ] ++ lib.optionals (cfg.steamBin != null) [ "--steam-bin" cfg.steamBin ]
      ++ lib.optionals (cfg.steamArgs != [ ])
      [ "--steam-args='${lib.concatStringsSep " " cfg.steamArgs}'" ]
      ++ cfg.extraArgs);

  consoleModeCommand = pkgs.writeShellScript "console-mode-wrapped" ''
    exec ${console-mode-pkg}/bin/console-mode ${mkArgs cfg}
  '';

in {
  options.programs.console-mode = {
    enable = mkEnableOption "Console Mode - gamescope session launcher";

    package = mkOption {
      type = types.package;
      default = console-mode-pkg;
      defaultText = literalExpression "pkgs.console-mode";
      description = "The console-mode package to use.";
    };

    autoStart = mkOption {
      type = types.bool;
      default = false;
      description = ''
        Automatically start console-mode on TTY1 when logging in.
        This adds the exec command to your shell profile.
      '';
    };

    autoStartVT = mkOption {
      type = types.int;
      default = 1;
      description = ''
        The virtual terminal number to auto-start on (e.g., 1 for TTY1).
        Only used if autoStart is enabled.
      '';
    };

    display = mkOption {
      type = types.nullOr types.str;
      default = null;
      example = "card1-HDMI-A-1";
      description = ''
        Override display selection. If null, console-mode will auto-detect
        and prompt if multiple displays are found.
      '';
    };

    resolution = mkOption {
      type = types.nullOr types.str;
      default = null;
      example = "1920x1080";
      description = ''
        Override resolution. If null, uses the display's native resolution.
      '';
    };

    refreshRate = mkOption {
      type = types.nullOr types.int;
      default = null;
      example = 144;
      description = ''
        Override refresh rate in Hz. If null, uses the maximum detected
        refresh rate from EDID.
      '';
    };

    forceVrr = mkOption {
      type = types.bool;
      default = false;
      description = ''
        Force enable VRR/Adaptive Sync even if not detected in EDID.
      '';
    };

    forceHdr = mkOption {
      type = types.bool;
      default = false;
      description = ''
        Force enable HDR even if not detected in EDID.
      '';
    };

    noVrr = mkOption {
      type = types.bool;
      default = false;
      description = ''
        Disable VRR/Adaptive Sync even if supported by the display.
      '';
    };

    noHdr = mkOption {
      type = types.bool;
      default = false;
      description = ''
        Disable HDR even if supported by the display.
      '';
    };

    safeMode = mkOption {
      type = types.bool;
      default = false;
      description = ''
        Use safe mode (disable advanced features, conservative defaults).
        Useful for troubleshooting or older displays.
      '';
    };

    gamescopeBin = mkOption {
      type = types.nullOr types.str;
      default = null;
      example = "\${pkgs.gamescope}/bin/gamescope";
      description = ''
        Custom path to gamescope binary. If null, uses gamescope from PATH.
      '';
    };

    steamBin = mkOption {
      type = types.nullOr types.str;
      default = null;
      example = "\${pkgs.steam}/bin/steam";
      description = ''
        Custom path to steam binary. If null, uses steam from PATH.
      '';
    };

    steamArgs = mkOption {
      type = types.listOf types.str;
      default = [ ];
      example = [ "-steamos3" ];
      description = ''
        Additional arguments to pass to Steam. Use ["-steamos3"] to enable
        Steam Deck features like Bluetooth device management in Big Picture mode.
      '';
    };

    extraArgs = mkOption {
      type = types.listOf types.str;
      default = [ ];
      example = [ "--prefer-vk-device" "1002:73ff" ];
      description = ''
        Additional arguments to pass to console-mode (and eventually gamescope).
      '';
    };

    environmentVariables = mkOption {
      type = types.attrsOf types.str;
      default = { };
      example = {
        RADV_PERFTEST = "gpl";
        MESA_VK_WSI_PRESENT_MODE = "mailbox";
      };
      description = ''
        Additional environment variables to set when launching console-mode.
      '';
    };

    createDesktopEntry = mkOption {
      type = types.bool;
      default = true;
      description = ''
        Create a desktop entry for launching console-mode from a desktop environment.
      '';
    };

    desktopEntry = {
      name = mkOption {
        type = types.str;
        default = "Console Mode";
        description = "Name for the desktop entry.";
      };

      genericName = mkOption {
        type = types.str;
        default = "Gaming Session";
        description = "Generic name for the desktop entry.";
      };

      comment = mkOption {
        type = types.str;
        default = "Launch Steam Big Picture in Gamescope";
        description = "Comment for the desktop entry.";
      };

      icon = mkOption {
        type = types.str;
        default = "steam";
        description = "Icon for the desktop entry.";
      };

      categories = mkOption {
        type = types.listOf types.str;
        default = [ "Game" "Application" ];
        description = "Categories for the desktop entry.";
      };
    };
  };

  config = mkIf cfg.enable {
    home.packages = [ cfg.package ];

    # Set up auto-start on TTY if requested
    programs.bash.profileExtra = mkIf cfg.autoStart ''
      # Auto-start Console Mode on TTY${toString cfg.autoStartVT}
      # Ensure XDG_VTNR is set
      if [[ -z "$XDG_VTNR" ]]; then
        TTY=$(tty)
        case "$TTY" in
          /dev/tty[0-9]*)
            export XDG_VTNR="''${TTY##*/tty}"
            ;;
        esac
      fi

      if [[ -z "$DISPLAY" && "$XDG_VTNR" = "${
        toString cfg.autoStartVT
      }" ]]; then
        ${
          optionalString (cfg.environmentVariables != { }) ''
            ${concatStringsSep "\n"
            (mapAttrsToList (name: value: ''export ${name}="${value}"'')
              cfg.environmentVariables)}
          ''
        }
        # Brief delay to allow DRM subsystem initialization
        sleep 2
        exec ${consoleModeCommand}
      fi
    '';

    programs.zsh.profileExtra = mkIf cfg.autoStart ''
      # Auto-start Console Mode on TTY${toString cfg.autoStartVT}
      # Ensure XDG_VTNR is set
      if [[ -z "$XDG_VTNR" ]]; then
        TTY=$(tty)
        case "$TTY" in
          /dev/tty[0-9]*)
            export XDG_VTNR="''${TTY##*/tty}"
            ;;
        esac
      fi

      if [[ -z "$DISPLAY" && "$XDG_VTNR" = "${
        toString cfg.autoStartVT
      }" ]]; then
        ${
          optionalString (cfg.environmentVariables != { }) ''
            ${concatStringsSep "\n"
            (mapAttrsToList (name: value: ''export ${name}="${value}"'')
              cfg.environmentVariables)}
          ''
        }
        # Brief delay to allow DRM subsystem initialization
        sleep 2
        exec ${consoleModeCommand}
      fi
    '';

    programs.fish.loginShellInit = mkIf cfg.autoStart ''
      # Auto-start Console Mode on TTY${toString cfg.autoStartVT}
      # Ensure XDG_VTNR is set
      if test -z "$XDG_VTNR"
        set TTY (tty)
        if string match -q '/dev/tty[0-9]*' $TTY
          set -x XDG_VTNR (string replace '/dev/tty' ' ' $TTY | string trim)
        end
      end

      if test -z "$DISPLAY"; and test "$XDG_VTNR" = "${
        toString cfg.autoStartVT
      }"
        ${
          optionalString (cfg.environmentVariables != { }) ''
            ${concatStringsSep "\n"
            (mapAttrsToList (name: value: ''set -x ${name} "${value}"'')
              cfg.environmentVariables)}
          ''
        }
        # Brief delay to allow DRM subsystem initialization
        sleep 2
        exec ${consoleModeCommand}
      end
    '';

    # Create desktop entry if requested
    xdg.desktopEntries.console-mode = mkIf cfg.createDesktopEntry {
      name = cfg.desktopEntry.name;
      genericName = cfg.desktopEntry.genericName;
      comment = cfg.desktopEntry.comment;
      exec = toString consoleModeCommand;
      icon = cfg.desktopEntry.icon;
      terminal = false;
      categories = cfg.desktopEntry.categories;
      type = "Application";
    };

    # Set environment variables in session
    home.sessionVariables = cfg.environmentVariables;
  };
}
