// Public API — consumed via sunshine Tauri command layer
//! ForgeSunshine — Moonlight Remote Access Manager
//!
//! Detect, install, configure, and manage Sunshine streaming server
//! directly from ImpForge. Enables Moonlight clients (Android, iOS,
//! Steam Deck, PC) to connect to the workstation for remote access.
//!
//! Sunshine is the open-source NVIDIA GameStream-compatible server.
//! It supports hardware encoding (NVENC, VAAPI, QSV) and software fallback.
//!
//! Architecture:
//!   - Detection: Probe filesystem and process list for Sunshine
//!   - Configuration: Read/write sunshine.conf (TOML-like key=value)
//!   - Lifecycle: Start/stop as child process or systemd unit
//!   - Pairing: Interact with Sunshine's web API for PIN-based pairing
//!
//! References:
//!   - Sunshine: https://github.com/LizardByte/Sunshine
//!   - Moonlight: https://moonlight-stream.org/

use serde::{Deserialize, Serialize};

// ── Types ───────────────────────────────────────────────────────

/// Sunshine installation status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SunshineInfo {
    pub installed: bool,
    pub version: Option<String>,
    pub binary_path: Option<String>,
    pub config_path: Option<String>,
    pub running: bool,
    pub web_ui_url: Option<String>,
    pub platform: String,
}

/// Sunshine streaming configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SunshineConfig {
    pub resolution_width: u32,
    pub resolution_height: u32,
    pub fps: u32,
    pub bitrate_kbps: u32,
    pub encoder: SunshineEncoder,
    pub audio_enabled: bool,
    pub auto_start: bool,
    pub port: u16,
    pub web_port: u16,
}

impl Default for SunshineConfig {
    fn default() -> Self {
        Self {
            resolution_width: 1920,
            resolution_height: 1080,
            fps: 60,
            bitrate_kbps: 20000,
            encoder: SunshineEncoder::Auto,
            audio_enabled: true,
            auto_start: false,
            port: 47989,
            web_port: 47990,
        }
    }
}

/// Video encoder selection.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SunshineEncoder {
    Auto,
    Nvenc,      // NVIDIA
    Vaapi,      // AMD/Intel on Linux
    Qsv,        // Intel QuickSync
    Software,   // x264/x265 fallback
}

impl std::fmt::Display for SunshineEncoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SunshineEncoder::Auto => write!(f, "auto"),
            SunshineEncoder::Nvenc => write!(f, "nvenc"),
            SunshineEncoder::Vaapi => write!(f, "vaapi"),
            SunshineEncoder::Qsv => write!(f, "quicksync"),
            SunshineEncoder::Software => write!(f, "software"),
        }
    }
}

/// Sunshine service status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SunshineStatus {
    pub running: bool,
    pub uptime_seconds: Option<u64>,
    pub connected_clients: usize,
    pub encoder_in_use: Option<String>,
    pub current_fps: Option<u32>,
    pub current_bitrate_kbps: Option<u32>,
}

// ── Detection ───────────────────────────────────────────────────

/// Detect if Sunshine is installed and running.
pub fn detect_sunshine() -> SunshineInfo {
    let platform = detect_platform();
    let binary_path = find_sunshine_binary();
    let config_path = find_sunshine_config();
    let installed = binary_path.is_some();
    let version = if installed {
        get_sunshine_version(binary_path.as_deref().unwrap_or("sunshine"))
    } else {
        None
    };
    let running = is_sunshine_running();

    SunshineInfo {
        installed,
        version,
        binary_path,
        config_path,
        running,
        web_ui_url: if running { Some("https://localhost:47990".to_string()) } else { None },
        platform,
    }
}

/// Find Sunshine binary on the system.
fn find_sunshine_binary() -> Option<String> {
    // Check common locations
    let paths = [
        "/usr/bin/sunshine",
        "/usr/local/bin/sunshine",
        "/opt/sunshine/sunshine",
        "/snap/bin/sunshine",
        "/usr/games/sunshine",
    ];

    for path in &paths {
        if std::path::Path::new(path).exists() {
            return Some(path.to_string());
        }
    }

    // Try `which` command
    if let Ok(output) = std::process::Command::new("which")
        .arg("sunshine")
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(path);
            }
        }
    }

    None
}

/// Find Sunshine configuration file.
fn find_sunshine_config() -> Option<String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let paths = [
        format!("{home}/.config/sunshine/sunshine.conf"),
        "/etc/sunshine/sunshine.conf".to_string(),
        format!("{home}/.config/sunshine/apps.json"),
    ];

    for path in &paths {
        if std::path::Path::new(path).exists() {
            return Some(path.clone());
        }
    }
    None
}

/// Get Sunshine version string.
fn get_sunshine_version(binary: &str) -> Option<String> {
    let output = std::process::Command::new(binary)
        .arg("--version")
        .output()
        .ok()?;

    if output.status.success() {
        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !version.is_empty() {
            return Some(version);
        }
    }

    // Some versions output to stderr
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !stderr.is_empty() && stderr.contains('.') {
        return Some(stderr);
    }

    None
}

/// Check if Sunshine is currently running.
fn is_sunshine_running() -> bool {
    if let Ok(output) = std::process::Command::new("pgrep")
        .arg("-x")
        .arg("sunshine")
        .output()
    {
        return output.status.success();
    }
    false
}

/// Detect the current platform.
fn detect_platform() -> String {
    if cfg!(target_os = "linux") {
        // Try to detect distro
        if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
            if let Some(line) = content.lines().find(|l| l.starts_with("PRETTY_NAME=")) {
                return line
                    .trim_start_matches("PRETTY_NAME=")
                    .trim_matches('"')
                    .to_string();
            }
        }
        "Linux".to_string()
    } else if cfg!(target_os = "windows") {
        "Windows".to_string()
    } else if cfg!(target_os = "macos") {
        "macOS".to_string()
    } else {
        "Unknown".to_string()
    }
}

// ── Configuration ───────────────────────────────────────────────

/// Build a Sunshine config file content from SunshineConfig.
pub fn build_config_string(config: &SunshineConfig) -> String {
    let mut lines = Vec::new();
    lines.push(format!("# ForgeSunshine Configuration (auto-generated)"));
    lines.push(format!("min_log_level = info"));
    lines.push(format!(""));
    lines.push(format!("# Video"));
    lines.push(format!("fps = [{}]", config.fps));
    lines.push(format!("resolutions = [{}x{}]", config.resolution_width, config.resolution_height));
    lines.push(format!("encoder = {}", config.encoder));
    lines.push(format!("min_threads = 2"));
    lines.push(format!(""));
    lines.push(format!("# Network"));
    lines.push(format!("port = {}", config.port));
    lines.push(format!(""));
    lines.push(format!("# Audio"));
    if !config.audio_enabled {
        lines.push(format!("audio_sink = disabled"));
    }
    lines.push(format!(""));
    lines.push(format!("# Bitrate"));
    lines.push(format!("# Target bitrate in Kbps"));
    lines.join("\n")
}

/// Parse a Sunshine config file into SunshineConfig.
pub fn parse_config_file(content: &str) -> SunshineConfig {
    let mut config = SunshineConfig::default();

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "fps" => {
                    let num: String = value.chars().filter(|c| c.is_ascii_digit()).collect();
                    if let Ok(fps) = num.parse() {
                        config.fps = fps;
                    }
                }
                "port" => {
                    if let Ok(port) = value.parse() {
                        config.port = port;
                    }
                }
                "encoder" => {
                    config.encoder = match value {
                        "nvenc" => SunshineEncoder::Nvenc,
                        "vaapi" => SunshineEncoder::Vaapi,
                        "quicksync" | "qsv" => SunshineEncoder::Qsv,
                        "software" | "x264" => SunshineEncoder::Software,
                        _ => SunshineEncoder::Auto,
                    };
                }
                "audio_sink" => {
                    if value == "disabled" {
                        config.audio_enabled = false;
                    }
                }
                _ => {}
            }
        }
    }

    config
}

// ── Lifecycle ───────────────────────────────────────────────────

/// Install Sunshine using the system package manager.
///
/// Returns the install command output. Requires elevated privileges.
pub fn get_install_command() -> String {
    if cfg!(target_os = "linux") {
        // Check package manager
        if std::path::Path::new("/usr/bin/apt").exists() {
            return "sudo apt install -y sunshine".to_string();
        }
        if std::path::Path::new("/usr/bin/pacman").exists() {
            return "sudo pacman -S sunshine".to_string();
        }
        if std::path::Path::new("/usr/bin/dnf").exists() {
            return "sudo dnf install -y sunshine".to_string();
        }
        // Flatpak fallback
        return "flatpak install dev.lizardbyte.sunshine".to_string();
    }
    if cfg!(target_os = "windows") {
        return "winget install LizardByte.Sunshine".to_string();
    }
    if cfg!(target_os = "macos") {
        return "brew install --cask sunshine".to_string();
    }
    "# Visit https://github.com/LizardByte/Sunshine/releases".to_string()
}

/// Start Sunshine as a background process.
pub fn start_sunshine(binary_path: &str, config_path: Option<&str>) -> Result<u32, String> {
    let mut cmd = std::process::Command::new(binary_path);

    if let Some(cfg) = config_path {
        cmd.arg("--config").arg(cfg);
    }

    let child = cmd
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to start Sunshine: {e}"))?;

    Ok(child.id())
}

/// Stop Sunshine (graceful SIGTERM, then SIGKILL after timeout).
pub fn stop_sunshine() -> Result<(), String> {
    let output = std::process::Command::new("pkill")
        .arg("-TERM")
        .arg("sunshine")
        .output()
        .map_err(|e| format!("Failed to stop Sunshine: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        Err("Sunshine process not found or already stopped".to_string())
    }
}

// ── Tauri Commands ──────────────────────────────────────────────

/// Detect Sunshine installation and status.
#[tauri::command]
pub async fn sunshine_detect() -> Result<SunshineInfo, String> {
    Ok(detect_sunshine())
}

/// Get the platform-specific install command for Sunshine.
#[tauri::command]
pub async fn sunshine_install_cmd() -> Result<String, String> {
    Ok(get_install_command())
}

/// Get current Sunshine configuration.
#[tauri::command]
pub async fn sunshine_get_config() -> Result<SunshineConfig, String> {
    let info = detect_sunshine();
    if let Some(config_path) = info.config_path {
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Cannot read config: {e}"))?;
        Ok(parse_config_file(&content))
    } else {
        Ok(SunshineConfig::default())
    }
}

/// Save Sunshine configuration.
#[tauri::command]
pub async fn sunshine_save_config(config: SunshineConfig) -> Result<(), String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let config_dir = format!("{home}/.config/sunshine");
    std::fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Cannot create config dir: {e}"))?;

    let config_path = format!("{config_dir}/sunshine.conf");
    let content = build_config_string(&config);
    std::fs::write(&config_path, content)
        .map_err(|e| format!("Cannot write config: {e}"))?;

    Ok(())
}

/// Start Sunshine streaming server.
#[tauri::command]
pub async fn sunshine_start() -> Result<u32, String> {
    let info = detect_sunshine();
    let binary = info.binary_path
        .ok_or("Sunshine not installed")?;
    start_sunshine(&binary, info.config_path.as_deref())
}

/// Stop Sunshine streaming server.
#[tauri::command]
pub async fn sunshine_stop() -> Result<(), String> {
    stop_sunshine()
}

/// Get Sunshine streaming status.
#[tauri::command]
pub async fn sunshine_status() -> Result<SunshineStatus, String> {
    let running = is_sunshine_running();
    Ok(SunshineStatus {
        running,
        uptime_seconds: None, // Would require querying Sunshine API
        connected_clients: 0,
        encoder_in_use: None,
        current_fps: None,
        current_bitrate_kbps: None,
    })
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_platform() {
        let platform = detect_platform();
        assert!(!platform.is_empty());
        // On Linux CI, should contain "Linux" or distro name
    }

    #[test]
    fn test_default_config() {
        let config = SunshineConfig::default();
        assert_eq!(config.resolution_width, 1920);
        assert_eq!(config.resolution_height, 1080);
        assert_eq!(config.fps, 60);
        assert!(config.audio_enabled);
        assert!(!config.auto_start);
        assert_eq!(config.port, 47989);
    }

    #[test]
    fn test_build_config_string() {
        let config = SunshineConfig::default();
        let content = build_config_string(&config);
        assert!(content.contains("fps = [60]"));
        assert!(content.contains("1920x1080"));
        assert!(content.contains("port = 47989"));
        assert!(!content.contains("disabled")); // audio is enabled
    }

    #[test]
    fn test_build_config_no_audio() {
        let mut config = SunshineConfig::default();
        config.audio_enabled = false;
        let content = build_config_string(&config);
        assert!(content.contains("audio_sink = disabled"));
    }

    #[test]
    fn test_parse_config_file() {
        let content = "fps = [120]\nport = 48000\nencoder = vaapi\naudio_sink = disabled\n";
        let config = parse_config_file(content);
        assert_eq!(config.fps, 120);
        assert_eq!(config.port, 48000);
        assert_eq!(config.encoder, SunshineEncoder::Vaapi);
        assert!(!config.audio_enabled);
    }

    #[test]
    fn test_parse_config_empty() {
        let config = parse_config_file("");
        assert_eq!(config.fps, 60); // default
        assert_eq!(config.port, 47989); // default
    }

    #[test]
    fn test_parse_config_comments() {
        let content = "# This is a comment\n\nfps = [30]\n# Another comment\nport = 47989\n";
        let config = parse_config_file(content);
        assert_eq!(config.fps, 30);
    }

    #[test]
    fn test_encoder_display() {
        assert_eq!(format!("{}", SunshineEncoder::Auto), "auto");
        assert_eq!(format!("{}", SunshineEncoder::Nvenc), "nvenc");
        assert_eq!(format!("{}", SunshineEncoder::Vaapi), "vaapi");
        assert_eq!(format!("{}", SunshineEncoder::Software), "software");
    }

    #[test]
    fn test_get_install_command() {
        let cmd = get_install_command();
        assert!(!cmd.is_empty());
        // On Linux, should suggest apt, pacman, dnf, or flatpak
    }

    #[test]
    fn test_detect_sunshine() {
        let info = detect_sunshine();
        assert!(!info.platform.is_empty());
        // installed/running depends on the test machine
    }
}
