use anyhow::{Context, Result};
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use evdev::{Device, InputEventKind, Key};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use regex::Regex;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Log debug messages to a file (since TUI takes over the terminal)
fn debug_log(msg: &str) {
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/console-mode-debug.log")
    {
        let _ = writeln!(file, "[{}] {}", chrono::Local::now().format("%H:%M:%S%.3f"), msg);
    }
}

/// Console Mode - A gamescope session launcher with automatic display detection
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Override display selection (connector name, e.g., "card1-HDMI-A-1")
    #[arg(short, long)]
    display: Option<String>,

    /// Override resolution (e.g., "1920x1080")
    #[arg(short, long)]
    resolution: Option<String>,

    /// Override refresh rate in Hz
    #[arg(short = 'f', long)]
    refresh_rate: Option<u32>,

    /// Force enable VRR/Adaptive Sync
    #[arg(long)]
    force_vrr: bool,

    /// Force enable HDR
    #[arg(long)]
    force_hdr: bool,

    /// Disable VRR even if supported
    #[arg(long)]
    no_vrr: bool,

    /// Disable HDR even if supported
    #[arg(long)]
    no_hdr: bool,

    /// Use safe mode (disable advanced features)
    #[arg(long)]
    safe_mode: bool,

    /// Custom gamescope binary path
    #[arg(long)]
    gamescope_bin: Option<PathBuf>,

    /// Custom steam binary path
    #[arg(long)]
    steam_bin: Option<PathBuf>,

    /// Additional steam arguments
    #[arg(long, value_delimiter = ' ', num_args = 1..)]
    steam_args: Vec<String>,

    /// Launcher command for display selection (e.g., "dmenu", "rofi -dmenu", "wofi --dmenu")
    #[arg(long)]
    launcher: Option<String>,

    /// Launch TUI monitor selector with controller support
    #[arg(long)]
    tui_launcher: bool,

    /// Additional gamescope arguments
    #[arg(last = true)]
    extra_args: Vec<String>,
}

#[derive(Debug, Clone)]
struct DisplayInfo {
    connector_name: String,
    connector_path: PathBuf,
    resolution: String,
    width: u32,
    height: u32,
}

#[derive(Debug, Default)]
struct DisplayCapabilities {
    vrr: bool,
    hdr: bool,
    max_refresh_rate: u32,
    max_bpc: u32,
}

fn main() -> Result<()> {
    let mut args = Args::parse();

    // Check for Sunshine client environment variables as fallback
    apply_sunshine_env_fallbacks(&mut args);

    // Set up environment variables
    setup_environment()?;

    // If TUI launcher mode is requested, run the TUI
    if args.tui_launcher {
        return run_tui_launcher(args);
    }

    // Check if we're running nested inside another compositor
    let is_nested = is_running_nested();

    if is_nested {
        println!("Detected nested environment (running inside another compositor)");
        println!("Launching in nested Wayland mode...");
        println!("\nNote: You may see some warnings from gamescope/Mesa:");
        println!("  - 'No CAP_SYS_NICE' - normal, doesn't affect gaming performance");
        println!("  - 'libdecor warnings' - expected in nested mode");
        println!("  - 'RADV not conformant' - safe to ignore, RADV works great for gaming");
        println!("  - 'vk_khr_present_wait overridden' - informational only\n");
        thread::sleep(Duration::from_secs(2));
        return launch_gamescope_nested(&args);
    }

    // Detect connected displays
    let displays = detect_displays()?;

    if displays.is_empty() {
        eprintln!("⚠ No connected displays detected, using fallback: 1920x1080");
        thread::sleep(Duration::from_secs(1));
        return launch_gamescope_fallback(&args);
    }

    // Select display
    let selected_display = if let Some(ref display_name) = args.display {
        displays
            .iter()
            .find(|d| d.connector_name == *display_name)
            .context(format!("Display '{}' not found", display_name))?
            .clone()
    } else if displays.len() > 1 {
        if let Some(ref launcher_cmd) = args.launcher {
            select_display_launcher(&displays, launcher_cmd)?
        } else {
            select_display_interactive(&displays)?
        }
    } else {
        println!("Detected display: {} at {}", displays[0].connector_name, displays[0].resolution);
        thread::sleep(Duration::from_secs(1));
        displays[0].clone()
    };

    // Override resolution if specified
    let display = if let Some(ref res) = args.resolution {
        let (width, height) = parse_resolution(res)?;
        DisplayInfo {
            resolution: res.clone(),
            width,
            height,
            ..selected_display
        }
    } else {
        selected_display
    };

    // Detect display capabilities
    println!("\n=== Detecting Display Capabilities ===\n");
    let capabilities = detect_capabilities(&display, &args)?;
    println!();
    thread::sleep(Duration::from_secs(2));

    // Launch gamescope
    launch_gamescope(&display, &capabilities, &args)
}

fn setup_environment() -> Result<()> {
    std::env::set_var("STEAM_FORCE_DESKTOPUI_SCALING", "1");
    std::env::set_var("XDG_SESSION_TYPE", "wayland");
    std::env::set_var("LIBSEAT_BACKEND", "logind");

    // Ensure XDG_RUNTIME_DIR is set
    if std::env::var("XDG_RUNTIME_DIR").is_err() {
        let uid = unsafe { libc::getuid() };
        std::env::set_var("XDG_RUNTIME_DIR", format!("/run/user/{}", uid));
    }

    Ok(())
}

/// Apply Sunshine client environment variables as fallback for CLI args
/// These are set by Sunshine when launching applications:
/// - SUNSHINE_CLIENT_WIDTH: Client's horizontal resolution
/// - SUNSHINE_CLIENT_HEIGHT: Client's vertical resolution
/// - SUNSHINE_CLIENT_FPS: Client's framerate setting
fn apply_sunshine_env_fallbacks(args: &mut Args) {
    // Only apply fallbacks if the corresponding CLI args weren't provided
    if args.resolution.is_none() {
        if let (Ok(width), Ok(height)) = (
            std::env::var("SUNSHINE_CLIENT_WIDTH"),
            std::env::var("SUNSHINE_CLIENT_HEIGHT"),
        ) {
            let resolution = format!("{}x{}", width, height);
            eprintln!("Using Sunshine client resolution: {}", resolution);
            args.resolution = Some(resolution);
        }
    }

    if args.refresh_rate.is_none() {
        if let Ok(fps) = std::env::var("SUNSHINE_CLIENT_FPS") {
            if let Ok(rate) = fps.parse::<u32>() {
                eprintln!("Using Sunshine client FPS as refresh rate: {}Hz", rate);
                args.refresh_rate = Some(rate);
            }
        }
    }
}

fn detect_displays() -> Result<Vec<DisplayInfo>> {
    let mut displays = Vec::new();
    let drm_path = Path::new("/sys/class/drm");

    for entry in fs::read_dir(drm_path)? {
        let entry = entry?;
        let path = entry.path();

        // Look for card*-* directories (e.g., card1-HDMI-A-1)
        let dir_name = entry.file_name();
        let dir_name_str = dir_name.to_string_lossy();

        if !dir_name_str.starts_with("card") || !dir_name_str.contains('-') {
            continue;
        }

        let status_file = path.join("status");
        if !status_file.exists() {
            continue;
        }

        let status = fs::read_to_string(&status_file)
            .context("Failed to read status file")?
            .trim()
            .to_string();

        if status == "connected" {
            let modes_file = path.join("modes");
            if modes_file.exists() {
                let modes = fs::read_to_string(&modes_file)?;
                if let Some(resolution) = modes.lines().next() {
                    let (width, height) = parse_resolution(resolution)?;

                    displays.push(DisplayInfo {
                        connector_name: dir_name_str.to_string(),
                        connector_path: path.clone(),
                        resolution: resolution.to_string(),
                        width,
                        height,
                    });
                }
            }
        }
    }

    Ok(displays)
}

fn parse_resolution(res: &str) -> Result<(u32, u32)> {
    let parts: Vec<&str> = res.trim().split('x').collect();
    if parts.len() != 2 {
        anyhow::bail!("Invalid resolution format: {}", res);
    }

    let width = parts[0].parse::<u32>()
        .context("Invalid width in resolution")?;
    let height = parts[1].parse::<u32>()
        .context("Invalid height in resolution")?;

    Ok((width, height))
}

fn select_display_interactive(displays: &[DisplayInfo]) -> Result<DisplayInfo> {
    println!("\n=== Gaming Display Selection ===\n");

    for (i, display) in displays.iter().enumerate() {
        println!("  [{}] {} - {}", i + 1, display.connector_name, display.resolution);
    }

    print!("\nSelect display (1-{}): ", displays.len());
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let choice: usize = input.trim().parse()
        .context("Invalid input")?;

    if choice < 1 || choice > displays.len() {
        println!("Invalid choice, using first display: {} at {}",
                 displays[0].connector_name, displays[0].resolution);
        thread::sleep(Duration::from_secs(1));
        Ok(displays[0].clone())
    } else {
        let selected = &displays[choice - 1];
        println!("Using {} at {}", selected.connector_name, selected.resolution);
        println!();
        thread::sleep(Duration::from_secs(2));
        Ok(selected.clone())
    }
}

fn select_display_launcher(displays: &[DisplayInfo], launcher_cmd: &str) -> Result<DisplayInfo> {
    // Create list of display options
    let options: Vec<String> = displays
        .iter()
        .map(|d| format!("{} - {}", d.connector_name, d.resolution))
        .collect();
    let options_text = options.join("\n");

    // Parse launcher command into program and arguments
    let parts: Vec<&str> = launcher_cmd.split_whitespace().collect();
    if parts.is_empty() {
        anyhow::bail!("Launcher command is empty");
    }

    let (program, args) = (parts[0], &parts[1..]);

    // Spawn the launcher process with piped stdin/stdout
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context(format!("Failed to spawn launcher: {}", launcher_cmd))?;

    // Write options to launcher's stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(options_text.as_bytes())
            .context("Failed to write to launcher stdin")?;
    }

    // Read selection from launcher's stdout
    let output = child.wait_with_output()
        .context("Failed to wait for launcher")?;

    if !output.status.success() {
        anyhow::bail!("Launcher exited with non-zero status (user may have cancelled)");
    }

    let selection = String::from_utf8(output.stdout)
        .context("Launcher output is not valid UTF-8")?
        .trim()
        .to_string();

    if selection.is_empty() {
        anyhow::bail!("No display selected");
    }

    // Find the matching display by parsing the selection
    // Format is "connector_name - resolution"
    let connector_name = selection
        .split(" - ")
        .next()
        .context("Invalid selection format")?;

    displays
        .iter()
        .find(|d| d.connector_name == connector_name)
        .cloned()
        .context(format!("Selected display '{}' not found", connector_name))
}

fn detect_capabilities(display: &DisplayInfo, args: &Args) -> Result<DisplayCapabilities> {
    if args.safe_mode {
        println!("⚠ Safe mode enabled - using conservative defaults");
        return Ok(DisplayCapabilities {
            vrr: false,
            hdr: false,
            max_refresh_rate: 60,
            max_bpc: 8,
        });
    }

    let edid_file = display.connector_path.join("edid");

    if !edid_file.exists() || !edid_file.is_file() {
        println!("⚠ EDID file not accessible, using defaults");
        return Ok(default_capabilities(display));
    }

    // Read EDID binary data
    let edid_data = fs::read(&edid_file)
        .context("Failed to read EDID file")?;

    if edid_data.is_empty() {
        println!("⚠ EDID file is empty, using defaults");
        return Ok(default_capabilities(display));
    }

    // Use edid-decode to parse EDID
    let edid_decode_output = Command::new("edid-decode")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .and_then(|mut child| {
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(&edid_data)?;
            }
            child.wait_with_output()
        });

    let capabilities = if let Ok(output) = edid_decode_output {
        let edid_text = String::from_utf8_lossy(&output.stdout);
        parse_edid_capabilities(&edid_text, display)
    } else {
        println!("⚠ Could not run edid-decode, using defaults");
        default_capabilities(display)
    };

    // Apply user overrides
    let mut caps = capabilities;

    if args.force_vrr {
        caps.vrr = true;
    } else if args.no_vrr {
        caps.vrr = false;
    }

    if args.force_hdr {
        caps.hdr = true;
    } else if args.no_hdr {
        caps.hdr = false;
    }

    if let Some(rate) = args.refresh_rate {
        caps.max_refresh_rate = rate;
    }

    // Print detected capabilities
    print_capabilities(&caps);

    Ok(caps)
}

fn parse_edid_capabilities(edid_text: &str, display: &DisplayInfo) -> DisplayCapabilities {
    let mut caps = DisplayCapabilities {
        vrr: false,
        hdr: false,
        max_refresh_rate: 60,
        max_bpc: 8,
    };

    // Check for VRR/FreeSync/G-SYNC
    let vrr_patterns = [
        "Variable Refresh Rate",
        "FreeSync",
        "G-SYNC Compatible",
        "VESA VRR",
        "Vendor-Specific Data Block (AMD)",
    ];

    for pattern in &vrr_patterns {
        if edid_text.contains(pattern) {
            caps.vrr = true;
            break;
        }
    }

    // Check for HDR
    let hdr_patterns = [
        "HDR Static Metadata",
        "HDR10",
        "SMPTE ST 2084",
    ];

    for pattern in &hdr_patterns {
        if edid_text.contains(pattern) {
            caps.hdr = true;
            break;
        }
    }

    // Check for color depth
    if edid_text.contains("12 bits per") || edid_text.contains("Bits per primary color channel: 12") {
        caps.max_bpc = 12;
    } else if edid_text.contains("10 bits per") || edid_text.contains("Bits per primary color channel: 10") {
        caps.max_bpc = 10;
    }

    // Extract maximum refresh rate
    let refresh_regex = Regex::new(r"(\d+)\.?\d*\s*Hz").ok();
    if let Some(re) = refresh_regex {
        let mut max_rate = 60;
        for cap in re.captures_iter(edid_text) {
            if let Ok(rate) = cap[1].parse::<u32>() {
                if rate > max_rate && rate <= 500 {  // Sanity check
                    max_rate = rate;
                }
            }
        }
        caps.max_refresh_rate = max_rate;
    }

    // Fallback: assume based on resolution if we didn't get a good refresh rate
    if caps.max_refresh_rate < 60 {
        caps.max_refresh_rate = if display.width >= 2560 { 144 } else { 60 };
    }

    caps
}

fn default_capabilities(display: &DisplayInfo) -> DisplayCapabilities {
    DisplayCapabilities {
        vrr: false,
        hdr: false,
        max_refresh_rate: if display.width >= 2560 { 144 } else { 60 },
        max_bpc: 8,
    }
}

fn print_capabilities(caps: &DisplayCapabilities) {
    if caps.vrr {
        println!("✓ VRR/Adaptive Sync supported");
    } else {
        println!("✗ VRR/Adaptive Sync not detected");
    }

    if caps.hdr {
        println!("✓ HDR supported");
    } else {
        println!("✗ HDR not detected");
    }

    match caps.max_bpc {
        12 => println!("✓ 12-bit color depth supported"),
        10 => println!("✓ 10-bit color depth supported"),
        _ => println!("✓ 8-bit color depth (standard)"),
    }

    println!("✓ Maximum refresh rate: {}Hz", caps.max_refresh_rate);
}

fn build_gamescope_args(display: &DisplayInfo, caps: &DisplayCapabilities, args: &Args) -> Vec<String> {
    let mut gs_args = vec![
        "-W".to_string(), display.width.to_string(),
        "-H".to_string(), display.height.to_string(),
        "-r".to_string(), caps.max_refresh_rate.to_string(),
    ];

    // Specify which output to use (strip "cardX-" prefix if present)
    let output_name = if let Some(stripped) = display.connector_name.split_once('-') {
        stripped.1.to_string()
    } else {
        display.connector_name.clone()
    };
    gs_args.extend(["--prefer-output".to_string(), output_name]);

    if caps.vrr {
        gs_args.push("--adaptive-sync".to_string());
    }

    if caps.hdr {
        gs_args.extend(["--hdr-enabled".to_string(), "--hdr-itm-enable".to_string()]);
    }

    // Add MangoHud
    gs_args.push("--mangoapp".to_string());

    // Fullscreen and expose Wayland
    gs_args.extend(["-f".to_string(), "-e".to_string()]);

    // Add any extra user-provided args
    gs_args.extend(args.extra_args.clone());

    gs_args
}

fn launch_gamescope(display: &DisplayInfo, caps: &DisplayCapabilities, args: &Args) -> Result<()> {
    let gs_args = build_gamescope_args(display, caps, args);

    println!("Launching gamescope with: {}", gs_args.join(" "));
    println!();
    thread::sleep(Duration::from_secs(1));

    let gamescope_bin = args.gamescope_bin.as_deref()
        .unwrap_or(Path::new("gamescope"));
    let steam_bin = args.steam_bin.as_deref()
        .unwrap_or(Path::new("steam"));

    let mut cmd = Command::new(gamescope_bin);
    cmd.args(&gs_args)
        .arg("--")
        .arg(steam_bin)
        .arg("-bigpicture")
        .args(&args.steam_args);

    let status = cmd.status()
        .context("Failed to launch gamescope")?;

    if !status.success() {
        eprintln!("\n======================================");
        eprintln!("Gamescope failed to start!");
        eprintln!("======================================\n");

        // Offer to retry with safe options
        print!("Press Enter to retry with safe options, or Ctrl+C to exit: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        println!("\nRetrying with safe options...");
        thread::sleep(Duration::from_secs(2));

        let width_str = display.width.to_string();
        let height_str = display.height.to_string();
        let safe_args = vec![
            "-W", width_str.as_str(),
            "-H", height_str.as_str(),
            "-r", "120",
            "-f", "-e",
        ];

        let mut safe_cmd = Command::new(gamescope_bin);
        safe_cmd.args(safe_args)
            .arg("--")
            .arg(steam_bin)
            .arg("-bigpicture")
            .args(&args.steam_args);

        safe_cmd.status()
            .context("Failed to launch gamescope in safe mode")?;
    }

    Ok(())
}

fn launch_gamescope_fallback(args: &Args) -> Result<()> {
    let gamescope_bin = args.gamescope_bin.as_deref()
        .unwrap_or(Path::new("gamescope"));
    let steam_bin = args.steam_bin.as_deref()
        .unwrap_or(Path::new("steam"));

    let mut cmd = Command::new(gamescope_bin);
    cmd.args(["-W", "1920", "-H", "1080", "-r", "60", "-f", "-e", "--"])
        .arg(steam_bin)
        .arg("-bigpicture")
        .args(&args.steam_args);

    cmd.status()
        .context("Failed to launch gamescope in fallback mode")?;

    Ok(())
}

fn is_running_nested() -> bool {
    // Check if we're running inside another compositor
    // WAYLAND_DISPLAY indicates we're in a Wayland session
    // DISPLAY indicates we're in an X11 session
    std::env::var("WAYLAND_DISPLAY").is_ok() || std::env::var("DISPLAY").is_ok()
}

fn launch_gamescope_nested(args: &Args) -> Result<()> {
    let gamescope_bin = args.gamescope_bin.as_deref()
        .unwrap_or(Path::new("gamescope"));
    let steam_bin = args.steam_bin.as_deref()
        .unwrap_or(Path::new("steam"));

    // Determine resolution from args or use defaults
    let (width, height) = if let Some(ref res) = args.resolution {
        parse_resolution(res)?
    } else {
        (1920, 1080)
    };

    let refresh_rate = args.refresh_rate.unwrap_or(60);

    let mut gs_args = vec![
        "-W".to_string(), width.to_string(),
        "-H".to_string(), height.to_string(),
        "-r".to_string(), refresh_rate.to_string(),
        "--nested-width".to_string(), width.to_string(),
        "--nested-height".to_string(), height.to_string(),
        "--nested-refresh".to_string(), refresh_rate.to_string(),
        "-e".to_string(),  // Expose Wayland socket
    ];

    // Add MangoHud if desired
    gs_args.push("--mangoapp".to_string());

    // Add any extra user-provided args
    gs_args.extend(args.extra_args.clone());

    println!("Launching gamescope in nested mode with: {}", gs_args.join(" "));
    println!();
    thread::sleep(Duration::from_secs(1));

    let mut cmd = Command::new(gamescope_bin);
    cmd.args(&gs_args)
        .arg("--")
        .arg(steam_bin)
        .arg("-bigpicture")
        .args(&args.steam_args);

    let status = cmd.status()
        .context("Failed to launch gamescope in nested mode")?;

    if !status.success() {
        anyhow::bail!("Gamescope exited with non-zero status");
    }

    Ok(())
}

// ============================================================================
// TUI Launcher Implementation
// ============================================================================

/// Input event from either keyboard or controller
enum InputEvent {
    Up,
    Down,
    Select,
    Quit,
}

/// TUI application state
struct TuiApp {
    displays: Vec<DisplayInfo>,
    list_state: ListState,
    should_quit: bool,
    selected_display: Option<DisplayInfo>,
}

impl TuiApp {
    fn new(displays: Vec<DisplayInfo>) -> Self {
        let mut list_state = ListState::default();
        if !displays.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            displays,
            list_state,
            should_quit: false,
            selected_display: None,
        }
    }

    fn next(&mut self) {
        if self.displays.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.displays.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn previous(&mut self) {
        if self.displays.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.displays.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn select(&mut self) {
        if let Some(i) = self.list_state.selected() {
            if i < self.displays.len() {
                self.selected_display = Some(self.displays[i].clone());
                self.should_quit = true;
            }
        }
    }
}

/// Find gamepad devices in /dev/input
fn find_gamepad_devices() -> Vec<PathBuf> {
    let mut devices = Vec::new();
    let input_path = Path::new("/dev/input");

    debug_log("Scanning for gamepad devices in /dev/input...");

    if let Ok(entries) = fs::read_dir(input_path) {
        let mut entries_vec: Vec<_> = entries.flatten().collect();
        // Sort entries to process in order
        entries_vec.sort_by_key(|e| e.path());

        for entry in entries_vec {
            let path = entry.path();
            if let Some(name) = path.file_name() {
                let name_str = name.to_string_lossy();
                if name_str.starts_with("event") {
                    // Check if we can open it and if it's a gamepad
                    match Device::open(&path) {
                        Ok(device) => {
                            let dev_name = device.name().unwrap_or("unknown");
                            debug_log(&format!("Opened {}: '{}'", path.display(), dev_name));

                            // Check for gamepad-like keys (BTN_SOUTH is common on gamepads)
                            if let Some(keys) = device.supported_keys() {
                                let has_south = keys.contains(Key::BTN_SOUTH);
                                let has_east = keys.contains(Key::BTN_EAST);
                                debug_log(&format!("  Keys: BTN_SOUTH={}, BTN_EAST={}", has_south, has_east));

                                if has_south || has_east {
                                    debug_log(&format!("  -> GAMEPAD DETECTED: {}", dev_name));
                                    devices.push(path);
                                }
                            } else {
                                debug_log("  No supported_keys()");
                            }
                        }
                        Err(e) => {
                            debug_log(&format!("Cannot open {}: {}", path.display(), e));
                        }
                    }
                }
            }
        }
    } else {
        debug_log("Failed to read /dev/input directory");
    }

    debug_log(&format!("Total gamepads found: {}", devices.len()));
    devices
}

/// Spawn a thread to read controller input
fn spawn_controller_reader(tx: mpsc::Sender<InputEvent>) {
    thread::spawn(move || {
        debug_log("Controller reader thread started");

        let device_paths = find_gamepad_devices();

        if device_paths.is_empty() {
            debug_log("No gamepads found, controller reader exiting");
            return;
        }

        // Open the first gamepad found
        let device_path = &device_paths[0];
        debug_log(&format!("Opening gamepad at: {}", device_path.display()));

        let mut device = match Device::open(device_path) {
            Ok(d) => {
                debug_log(&format!("Successfully opened: {}", d.name().unwrap_or("unknown")));
                d
            }
            Err(e) => {
                debug_log(&format!("Failed to open device: {}", e));
                return;
            }
        };

        debug_log("Starting event loop...");
        let mut event_count = 0;

        loop {
            match device.fetch_events() {
                Ok(events) => {
                    for ev in events {
                        event_count += 1;

                        // Log every event for debugging
                        if event_count <= 50 {
                            debug_log(&format!("Event #{}: type={:?}, code={:?}, value={}",
                                event_count, ev.kind(), ev.code(), ev.value()));
                        }

                        if let InputEventKind::Key(key) = ev.kind() {
                            debug_log(&format!("Key event: {:?}, value={}", key, ev.value()));

                            // Only process key press events (value == 1)
                            if ev.value() == 1 {
                                let input = match key {
                                    // D-pad
                                    Key::BTN_DPAD_UP => {
                                        debug_log("D-pad UP pressed");
                                        Some(InputEvent::Up)
                                    }
                                    Key::BTN_DPAD_DOWN => {
                                        debug_log("D-pad DOWN pressed");
                                        Some(InputEvent::Down)
                                    }
                                    // Face buttons (BTN_SOUTH = A/Cross, BTN_WEST = X/Square, BTN_EAST = B/Circle)
                                    Key::BTN_SOUTH => {
                                        debug_log("BTN_SOUTH (Cross/A) pressed -> Select");
                                        Some(InputEvent::Select)
                                    }
                                    Key::BTN_WEST => {
                                        debug_log("BTN_WEST (Square/X) pressed -> Select");
                                        Some(InputEvent::Select)
                                    }
                                    Key::BTN_EAST => {
                                        debug_log("BTN_EAST (Circle/B) pressed -> Quit");
                                        Some(InputEvent::Quit)
                                    }
                                    _ => None,
                                };

                                if let Some(input) = input {
                                    debug_log("Sending input event to TUI...");
                                    if tx.send(input).is_err() {
                                        debug_log("Channel closed, exiting controller reader");
                                        return;
                                    }
                                    debug_log("Input event sent successfully");
                                }
                            }
                        }

                        // Handle D-pad as absolute axis (HAT)
                        if let InputEventKind::AbsAxis(axis) = ev.kind() {
                            use evdev::AbsoluteAxisType;
                            match axis {
                                AbsoluteAxisType::ABS_HAT0Y => {
                                    debug_log(&format!("ABS_HAT0Y: value={}", ev.value()));
                                    let input = if ev.value() < 0 {
                                        debug_log("HAT UP -> Navigation Up");
                                        Some(InputEvent::Up)
                                    } else if ev.value() > 0 {
                                        debug_log("HAT DOWN -> Navigation Down");
                                        Some(InputEvent::Down)
                                    } else {
                                        None
                                    };
                                    if let Some(input) = input {
                                        let _ = tx.send(input);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Err(e) => {
                    debug_log(&format!("Error fetching events: {}", e));
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }
    });
}

/// Render the TUI
fn render_tui(frame: &mut Frame, app: &mut TuiApp) {
    let area = frame.area();

    // Create a centered box
    let popup_area = centered_rect(60, 60, area);

    // Create the list items
    let items: Vec<ListItem> = app
        .displays
        .iter()
        .map(|d| {
            let content = format!("{} ({})", d.connector_name, d.resolution);
            ListItem::new(Line::from(content))
        })
        .collect();

    // Create the list widget
    let list = List::new(items)
        .block(
            Block::default()
                .title(" Console Mode - Select Monitor ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, popup_area, &mut app.list_state);

    // Render help text at the bottom
    let help_area = Rect {
        x: popup_area.x,
        y: popup_area.y + popup_area.height,
        width: popup_area.width,
        height: 2,
    };

    if help_area.y + help_area.height <= area.height {
        let help_text = Paragraph::new(Line::from(vec![
            Span::styled("[↑/↓] ", Style::default().fg(Color::Yellow)),
            Span::raw("Navigate  "),
            Span::styled("[Enter/A] ", Style::default().fg(Color::Green)),
            Span::raw("Select  "),
            Span::styled("[Esc/B] ", Style::default().fg(Color::Red)),
            Span::raw("Quit"),
        ]));
        frame.render_widget(help_text, help_area);
    }

    // Show message if no displays found
    if app.displays.is_empty() {
        let msg = Paragraph::new("No connected displays found. Press any key to exit.")
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(msg, popup_area);
    }
}

/// Helper to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}

/// Run the TUI launcher
fn run_tui_launcher(args: Args) -> Result<()> {
    // Detect displays first
    let displays = detect_displays()?;

    // If only one display, skip the TUI and just launch
    if displays.len() == 1 {
        println!("Single display detected: {} at {}", displays[0].connector_name, displays[0].resolution);
        thread::sleep(Duration::from_secs(1));

        let mut new_args = args;
        new_args.display = Some(displays[0].connector_name.clone());
        new_args.tui_launcher = false;

        // Re-run without TUI
        return launch_with_display(&displays[0], new_args);
    }

    // Set up terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = TuiApp::new(displays);

    // Set up input channel for controller
    let (tx, rx) = mpsc::channel::<InputEvent>();
    spawn_controller_reader(tx);

    // Main loop
    loop {
        // Draw
        terminal.draw(|f| render_tui(f, &mut app))?;

        // Handle input
        // Check for controller input (non-blocking)
        if let Ok(input) = rx.try_recv() {
            match input {
                InputEvent::Up => app.previous(),
                InputEvent::Down => app.next(),
                InputEvent::Select => app.select(),
                InputEvent::Quit => app.should_quit = true,
            }
        }

        // Check for keyboard input
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Up | KeyCode::Char('k') => app.previous(),
                        KeyCode::Down | KeyCode::Char('j') => app.next(),
                        KeyCode::Enter | KeyCode::Char(' ') => app.select(),
                        KeyCode::Esc | KeyCode::Char('q') => app.should_quit = true,
                        _ => {}
                    }
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;

    // If a display was selected, launch with it
    if let Some(display) = app.selected_display {
        println!("\nLaunching with display: {} at {}", display.connector_name, display.resolution);
        thread::sleep(Duration::from_secs(1));

        let mut new_args = args;
        new_args.display = Some(display.connector_name.clone());
        new_args.tui_launcher = false;

        launch_with_display(&display, new_args)?;
    }

    Ok(())
}

/// Launch gamescope with a specific display
fn launch_with_display(display: &DisplayInfo, args: Args) -> Result<()> {
    // Detect capabilities for this display
    println!("\n=== Detecting Display Capabilities ===\n");
    let capabilities = detect_capabilities(display, &args)?;
    println!();
    thread::sleep(Duration::from_secs(2));

    // Launch gamescope
    launch_gamescope(display, &capabilities, &args)
}
