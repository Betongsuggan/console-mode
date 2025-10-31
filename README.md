# Console Mode

A Rust-based gamescope session launcher with automatic display detection and capability detection for Linux gaming systems.

## Features

- **Automatic Display Detection**: Scans `/sys/class/drm` to detect all connected displays
- **Interactive Display Selection**: Prompts for display choice when multiple monitors are connected
- **EDID Capability Detection**: Automatically detects display capabilities including:
  - VRR/Adaptive Sync (FreeSync/G-SYNC)
  - HDR support
  - Maximum refresh rate
  - Color depth (8-bit, 10-bit, 12-bit)
- **Smart Gamescope Configuration**: Builds optimal gamescope arguments based on detected capabilities
- **Fallback Support**: Safe mode and fallback options for problematic displays
- **CLI Overrides**: Full command-line control over all display settings

## Requirements

- Linux system with DRM display subsystem
- `gamescope` installed
- `steam` installed
- `edid-decode` tool (optional, for capability detection)
- Rust toolchain for building (cargo)

## Installation

### Using Nix Flakes (Recommended)

This project provides a Nix flake with home-manager and NixOS modules.

1. Add to your `flake.nix` inputs:

```nix
{
  inputs = {
    console-mode = {
      url = "github:yourusername/console-mode";
      # Or use local path during development:
      # url = "path:/home/betongsuggan/development/console-mode";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
}
```

2. Import the home-manager module:

```nix
{
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

3. Configure in your `home.nix`:

```nix
{
  programs.console-mode = {
    enable = true;
    autoStart = true;  # Auto-start on TTY1
    resolution = "2560x1440";
    refreshRate = 144;
    forceVrr = true;
  };
}
```

See [CONFIGURATION.md](CONFIGURATION.md) for detailed configuration options.

### From Source

```bash
git clone <repository-url>
cd console-mode
cargo build --release
```

The binary will be located at `target/release/console-mode`.

### Using Nix (without flakes)

Build and run directly:

```bash
nix build
./result/bin/console-mode --help
```

## Usage

### Basic Usage

Launch with automatic display detection:

```bash
console-mode
```

### CLI Options

```
Options:
  -d, --display <DISPLAY>
          Override display selection (connector name, e.g., "card1-HDMI-A-1")

  -r, --resolution <RESOLUTION>
          Override resolution (e.g., "1920x1080")

  -f, --refresh-rate <REFRESH_RATE>
          Override refresh rate in Hz

      --force-vrr
          Force enable VRR/Adaptive Sync

      --force-hdr
          Force enable HDR

      --no-vrr
          Disable VRR even if supported

      --no-hdr
          Disable HDR even if supported

      --safe-mode
          Use safe mode (disable advanced features)

      --gamescope-bin <GAMESCOPE_BIN>
          Custom gamescope binary path

      --steam-bin <STEAM_BIN>
          Custom steam binary path

  -h, --help
          Print help

  -V, --version
          Print version
```

### Examples

#### Force specific display and resolution:

```bash
console-mode --display card1-HDMI-A-1 --resolution 2560x1440
```

#### Override refresh rate:

```bash
console-mode --refresh-rate 144
```

#### Force HDR and VRR:

```bash
console-mode --force-hdr --force-vrr
```

#### Safe mode (conservative settings):

```bash
console-mode --safe-mode
```

#### Custom gamescope/steam paths:

```bash
console-mode --gamescope-bin /usr/bin/gamescope --steam-bin /usr/bin/steam
```

#### Pass additional arguments to gamescope:

```bash
console-mode -- --prefer-vk-device 1002:73ff
```

## Integration

### Auto-start on Login (TTY1)

Add to your shell profile (e.g., `~/.bash_profile` or `~/.zprofile`):

```bash
if [[ -z "$DISPLAY" && "$XDG_VTNR" = "1" ]]; then
  exec console-mode
fi
```

### Desktop Entry

A desktop entry is useful for launching from a desktop environment:

```ini
[Desktop Entry]
Name=Console Mode
GenericName=Gaming Session
Comment=Launch Steam Big Picture in Gamescope
Exec=console-mode
Icon=steam
Terminal=false
Type=Application
Categories=Game;Application;
```

## How It Works

1. **Environment Setup**: Sets required environment variables for Wayland/gamescope
2. **Display Detection**: Scans `/sys/class/drm/card*/card*-*/` for connected displays
3. **Display Selection**:
   - Single display: Automatically selected
   - Multiple displays: Interactive prompt
   - CLI override: Use specified display
4. **EDID Analysis**: Reads EDID data and uses `edid-decode` to parse capabilities
5. **Capability Detection**: Detects VRR, HDR, refresh rate, and color depth
6. **Gamescope Launch**: Builds optimized command line and launches gamescope + Steam
7. **Fallback**: On failure, offers to retry with safe settings

## Troubleshooting

### No displays detected

- Check that `/sys/class/drm` is accessible
- Verify displays are actually connected
- Try running with `--safe-mode`

### EDID parsing fails

- Install `edid-decode` tool
- The application will fall back to conservative defaults if EDID parsing fails

### Gamescope fails to start

- The application will prompt to retry with safe settings
- Try `--safe-mode` flag
- Check gamescope logs for specific errors

### Performance issues

- Try disabling HDR: `--no-hdr`
- Try disabling VRR: `--no-vrr`
- Lower refresh rate: `--refresh-rate 60`

## Development

### Building

```bash
cargo build
```

### Running in development

```bash
cargo run -- --help
```

### Running tests

```bash
cargo test
```

### Code structure

- Display detection: `detect_displays()` function
- EDID parsing: `detect_capabilities()` and `parse_edid_capabilities()` functions
- Gamescope launcher: `launch_gamescope()` function
- CLI parsing: Uses `clap` derive macros

## License

MIT OR Apache-2.0

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.
