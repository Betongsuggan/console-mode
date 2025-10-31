# Console Mode Configuration Guide

This guide shows how to configure console-mode in your NixOS/home-manager setup.

## Quick Start

### 1. Add the flake input

In your `flake.nix`:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    home-manager.url = "github:nix-community/home-manager";

    # Add console-mode
    console-mode = {
      url = "path:/home/betongsuggan/development/console-mode";
      # Or use git: url = "github:yourusername/console-mode";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { nixpkgs, home-manager, console-mode, ... }: {
    homeConfigurations.yourusername = home-manager.lib.homeManagerConfiguration {
      modules = [
        console-mode.homeManagerModules.default
        ./home.nix
      ];
    };
  };
}
```

### 2. Enable in your home-manager configuration

Minimal configuration (`home.nix`):

```nix
{
  programs.console-mode = {
    enable = true;
    autoStart = true;  # Auto-start on TTY1
  };
}
```

## Configuration Options

### Basic Options

```nix
{
  programs.console-mode = {
    # Enable the module
    enable = true;

    # Auto-start on TTY login
    autoStart = true;
    autoStartVT = 1;  # TTY1 (default)

    # Use a custom package (optional)
    # package = pkgs.console-mode;
  };
}
```

### Display Configuration

```nix
{
  programs.console-mode = {
    enable = true;

    # Force specific display (skip interactive selection)
    display = "card1-HDMI-A-1";

    # Override resolution
    resolution = "2560x1440";

    # Override refresh rate
    refreshRate = 144;
  };
}
```

### Feature Control

```nix
{
  programs.console-mode = {
    enable = true;

    # Force enable features even if not detected
    forceVrr = true;
    forceHdr = true;

    # Or disable features even if detected
    # noVrr = true;
    # noHdr = true;

    # Use safe mode (conservative settings)
    # safeMode = true;
  };
}
```

### Custom Binary Paths

```nix
{
  programs.console-mode = {
    enable = true;

    # Use unstable gamescope
    gamescopeBin = "${pkgs.unstable.gamescope}/bin/gamescope";

    # Use flatpak steam
    steamBin = "/var/lib/flatpak/exports/bin/com.valvesoftware.Steam";
  };
}
```

### Extra Arguments

```nix
{
  programs.console-mode = {
    enable = true;

    # Pass additional arguments to gamescope
    extraArgs = [
      "--prefer-vk-device"
      "1002:73ff"  # Prefer AMD GPU
      "--fsr-sharpness"
      "5"
    ];
  };
}
```

### Environment Variables

```nix
{
  programs.console-mode = {
    enable = true;

    # Set environment variables for the session
    environmentVariables = {
      # AMD GPU optimizations
      RADV_PERFTEST = "gpl";
      MESA_VK_WSI_PRESENT_MODE = "mailbox";

      # Controller settings
      SDL_JOYSTICK_HIDAPI = "0";

      # Steam settings
      STEAM_USE_DYNAMIC_VRS = "0";
    };
  };
}
```

### Desktop Entry Customization

```nix
{
  programs.console-mode = {
    enable = true;

    # Create desktop entry (default: true)
    createDesktopEntry = true;

    desktopEntry = {
      name = "Gaming Mode";
      genericName = "Gamescope Session";
      comment = "Launch into gaming session";
      icon = "steam";
      categories = [ "Game" "Application" ];
    };
  };
}
```

## Complete Example Configuration

Here's a complete example configuration for a gaming setup:

```nix
{ config, pkgs, inputs, ... }:

{
  imports = [
    inputs.console-mode.homeManagerModules.default
  ];

  programs.console-mode = {
    enable = true;

    # Auto-start on TTY1
    autoStart = true;
    autoStartVT = 1;

    # Display settings
    # display = "card1-HDMI-A-1";  # Uncomment to skip display selection
    resolution = "2560x1440";
    refreshRate = 144;

    # Enable gaming features
    forceVrr = true;
    # forceHdr = true;  # Uncomment if you have HDR display

    # Use unstable gamescope for latest features
    gamescopeBin = "${pkgs.unstable.gamescope}/bin/gamescope";

    # Gamescope optimizations
    extraArgs = [
      "--prefer-vk-device"
      "1002:73ff"  # AMD Radeon RX 6700 XT
    ];

    # Gaming optimizations
    environmentVariables = {
      # AMD GPU performance
      RADV_PERFTEST = "gpl";
      MESA_VK_WSI_PRESENT_MODE = "mailbox";

      # Controller compatibility
      SDL_JOYSTICK_HIDAPI = "0";
      STEAM_USE_DYNAMIC_VRS = "0";

      # FSR for compatible games
      # WINE_FULLSCREEN_FSR = "1";
    };

    # Desktop entry
    createDesktopEntry = true;
    desktopEntry = {
      name = "Console Gaming Mode";
      genericName = "Steam Big Picture";
      comment = "Launch optimized gaming session";
      icon = "steam";
    };
  };

  # Additional gaming packages
  home.packages = with pkgs; [
    steam
    gamescope
    mangohud
  ];
}
```

## Migrating from Bash Script

If you're migrating from the old `start-gamescope-session` bash script:

### Old Configuration

```nix
# Old way - inline bash script
home.packages = [
  (writeShellScriptBin "start-gamescope-session" ''
    export STEAM_FORCE_DESKTOPUI_SCALING=1
    export XDG_SESSION_TYPE=wayland
    # ... 300 lines of bash ...
    gamescope ... -- steam -bigpicture
  '')
];

programs.bash.profileExtra = ''
  if [[ -z "$DISPLAY" && "$XDG_VTNR" = "1" ]]; then
    exec start-gamescope-session
  fi
'';
```

### New Configuration

```nix
# New way - declarative configuration
programs.console-mode = {
  enable = true;
  autoStart = true;
  resolution = "2560x1440";
  refreshRate = 144;
  forceVrr = true;

  environmentVariables = {
    RADV_PERFTEST = "gpl";
    MESA_VK_WSI_PRESENT_MODE = "mailbox";
  };
};
```

Benefits:
- Type-checked configuration
- No inline bash scripts
- Easier to maintain and modify
- Better error handling
- Automatic EDID detection still works
- Fallback modes built-in

## Shell-Specific Configuration

The module automatically configures the appropriate shell profile:

### Bash
Auto-configured via `programs.bash.profileExtra`

### Zsh
Auto-configured via `programs.zsh.profileExtra`

### Fish
Auto-configured via `programs.fish.loginShellInit`

## Troubleshooting

### Enable safe mode

If console-mode fails to start:

```nix
programs.console-mode.safeMode = true;
```

### Disable advanced features

```nix
programs.console-mode = {
  noVrr = true;
  noHdr = true;
  refreshRate = 60;
};
```

### Use specific display

Skip interactive selection:

```nix
programs.console-mode.display = "card1-HDMI-A-1";
```

### Check available displays

List your displays:

```bash
ls -l /sys/class/drm/card*/card*-*/status
```

### Test without auto-start

```nix
programs.console-mode = {
  enable = true;
  autoStart = false;  # Don't auto-start
};
```

Then manually run:

```bash
console-mode --help
console-mode --safe-mode
```

## Advanced Usage

### Multiple User Configurations

In your system configuration:

```nix
# For user "gamer" - auto-start gaming mode
home-manager.users.gamer = {
  programs.console-mode = {
    enable = true;
    autoStart = true;
    forceVrr = true;
  };
};

# For user "regular" - available but don't auto-start
home-manager.users.regular = {
  programs.console-mode = {
    enable = true;
    autoStart = false;
  };
};
```

### Conditional Configuration

Enable only on specific hosts:

```nix
programs.console-mode = {
  enable = config.networking.hostName == "gaming-pc";
  autoStart = true;
};
```

### Using with NixOS Module

System-wide installation:

```nix
# configuration.nix
{
  imports = [ inputs.console-mode.nixosModules.default ];

  programs.console-mode.enable = true;
}
```

Then configure per-user in home-manager.

## See Also

- [README.md](README.md) - Project overview and building instructions
- [src/main.rs](src/main.rs) - Source code and implementation details
