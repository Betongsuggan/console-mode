use anyhow::{Context, Result};
use clap::Parser;
use regex::Regex;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

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
    let args = Args::parse();

    // Set up environment variables
    setup_environment()?;

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
        select_display_interactive(&displays)?
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
        .arg("-bigpicture");

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
            .arg("-bigpicture");

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
        .arg("-bigpicture");

    cmd.status()
        .context("Failed to launch gamescope in fallback mode")?;

    Ok(())
}
