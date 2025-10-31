# Integration Example for nix-home

This document shows how to integrate console-mode into your existing nix-home configuration.

## For Your Setup

Based on your current configuration at `/home/betongsuggan/nix-home/hosts/private-desktop/user-gamer.nix`, here's how to integrate console-mode:

### Step 1: Update your flake.nix

In `/home/betongsuggan/nix-home/flake.nix`, add console-mode as an input:

```nix
{
  inputs = {
    # ... your existing inputs ...

    console-mode = {
      url = "path:/home/betongsuggan/development/console-mode";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, home-manager, console-mode, ... }@inputs: {
    # Pass console-mode to your home-manager configuration
    homeConfigurations = {
      gamer = home-manager.lib.homeManagerConfiguration {
        modules = [
          console-mode.homeManagerModules.default
          ./hosts/private-desktop/user-gamer.nix
          # ... your other modules ...
        ];
        extraSpecialArgs = { inherit inputs; };
      };
    };
  };
}
```

### Step 2: Update user-gamer.nix

Replace the inline bash script with console-mode configuration:

**Before** (current configuration):
```nix
{ pkgs, inputs, ... }:

{
  imports = [ ../../modules/users inputs.stylix.homeModules.stylix ];

  # ... other config ...

  home.packages = with pkgs;
    let gamescopeUnstable = unstable.gamescope;
    in [
      steam
      steam-run
      # ... other packages ...
      gamescopeUnstable

      (writeShellScriptBin "start-gamescope-session" ''
        # 300+ lines of bash script...
      '')
    ];

  programs.bash = {
    enable = true;
    profileExtra = ''
      if [[ -z "$DISPLAY" && "$XDG_VTNR" = "1" ]]; then
        exec start-gamescope-session
      fi
    '';
  };
}
```

**After** (with console-mode):
```nix
{ pkgs, inputs, ... }:

{
  imports = [ ../../modules/users inputs.stylix.homeModules.stylix ];

  home.username = "gamer";
  home.homeDirectory = "/home/gamer";
  home.stateVersion = "25.05";

  # Enable gaming setup with enhanced MangoHud
  games = {
    enable = true;
    mangohud = {
      enable = true;
      detailedMode = true;
      controllerToggle = false;
      position = "top-left";
      fontSize = 22;
    };
  };

  battery-monitor.enable = true;

  launcher = {
    enable = true;
    backend = "walker";
  };

  controller = {
    enable = true;
    type = "ps5";
    mangohudToggle = {
      enable = true;
      buttons = [ "square" "triangle" ];
      autoStart = true;
    };
    rumble.enable = true;
  };

  windowManager.enable = true;
  windowManager.type = "hyprland";

  shell.enable = true;
  shell.defaultShell = "bash";

  theme = {
    enable = true;
    wallpaper = ../../assets/wallpaper/zeal.jpg;
    cursor = {
      package = pkgs.banana-cursor;
      name = "Banana";
    };
  };

  # NEW: Console Mode configuration (replaces the bash script)
  programs.console-mode = {
    enable = true;

    # Auto-start on TTY1
    autoStart = true;
    autoStartVT = 1;

    # Use unstable gamescope (same as before)
    gamescopeBin = "${pkgs.unstable.gamescope}/bin/gamescope";
    steamBin = "${pkgs.steam}/bin/steam";

    # The resolution, refresh rate, VRR, HDR, etc. will be auto-detected
    # from EDID, but you can override if needed:
    # resolution = "2560x1440";
    # refreshRate = 144;
    # forceVrr = true;
    # forceHdr = true;

    # Environment variables (same as before)
    environmentVariables = {
      RADV_PERFTEST = "gpl";
      MESA_VK_WSI_PRESENT_MODE = "mailbox";
      STEAM_USE_DYNAMIC_VRS = "0";
      SDL_JOYSTICK_HIDAPI = "0";
    };

    # Create desktop entry
    createDesktopEntry = true;
    desktopEntry = {
      name = "Gamescope Gaming Session";
      genericName = "Steam Big Picture (Gamescope)";
      comment = "Launch Steam Big Picture in Gamescope session";
      icon = "steam";
      categories = [ "Game" "Application" ];
    };
  };

  # Keep your other packages (remove the writeShellScriptBin)
  home.packages = with pkgs;
    let gamescopeUnstable = unstable.gamescope;
    in [
      steam
      steam-run
      htop
      pulseaudio
      pavucontrol
      xdg-utils
      edid-decode
      gamescopeUnstable
      # No more writeShellScriptBin needed!
    ];

  # Remove the bash profileExtra - console-mode handles this automatically
  programs.bash.enable = true;
  # profileExtra is no longer needed!

  # Keep your session variables
  home.sessionVariables = {
    RADV_PERFTEST = "gpl";
    MESA_VK_WSI_PRESENT_MODE = "mailbox";
    STEAM_USE_DYNAMIC_VRS = "0";
    SDL_JOYSTICK_HIDAPI = "0";
  };

  programs.home-manager.enable = true;
}
```

### Step 3: Rebuild your configuration

```bash
cd ~/nix-home
home-manager switch --flake .#gamer
```

## Benefits of the Migration

1. **Declarative**: All configuration in Nix options, no inline bash
2. **Type-safe**: Nix checks your configuration at build time
3. **Maintainable**: Much easier to understand and modify
4. **Modular**: Can easily enable/disable features
5. **Reusable**: Can share configuration across multiple machines
6. **Better error handling**: Rust provides better error messages
7. **Automatic detection**: EDID parsing still works automatically
8. **Fallback support**: Built-in safe mode and retry logic

## What Stays the Same

- Display detection and EDID parsing still work
- VRR/HDR detection still automatic
- Interactive display selection for multi-monitor
- Gamescope launch with optimized settings
- Steam Big Picture Mode launch
- All environment variables
- Auto-start on TTY1

## What's Better

- No more 300-line bash script in your config
- Type-checked configuration
- Better error messages
- Easier to override settings
- Can test without rebuilding entire system
- Reusable across multiple users/machines

## Testing Before Migration

1. Build console-mode:
```bash
cd ~/development/console-mode
nix build
```

2. Test it manually:
```bash
./result/bin/console-mode --help
./result/bin/console-mode --safe-mode  # Won't actually launch, just test
```

3. Once satisfied, update your flake and rebuild

## Rollback Plan

If something goes wrong, you can always rollback:

```bash
home-manager generations
home-manager switch --rollback
```

Or use the previous generation:

```bash
/nix/var/nix/profiles/per-user/$USER/home-manager-<number>-link/activate
```

## Advanced: Per-Display Configuration

If you have multiple displays and want to always use a specific one:

```nix
programs.console-mode = {
  enable = true;
  display = "card1-HDMI-A-1";  # Skip interactive selection
  resolution = "2560x1440";
  refreshRate = 144;
};
```

Find your display names:
```bash
ls /sys/class/drm/card*/card*-*/status
```

## Questions?

See [CONFIGURATION.md](CONFIGURATION.md) for all available options.
