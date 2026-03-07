//! Lightweight monitoring for status bar
//! Reads directly from sysfs — no sysinfo dependency, <1ms latency

use serde::{Deserialize, Serialize};

/// Lightweight stats for the status bar (polled every 2s)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickStats {
    pub cpu_percent: f32,
    pub ram_used_gb: f32,
    pub ram_total_gb: f32,
    pub gpu_name: Option<String>,
    pub gpu_temp_c: Option<f32>,
    pub gpu_vram_used_mb: Option<u64>,
    pub gpu_vram_total_mb: Option<u64>,
    pub gpu_usage_percent: Option<f32>,
}

#[tauri::command]
pub fn cmd_get_quick_stats() -> QuickStats {
    let (cpu_percent, ram_used_gb, ram_total_gb) = read_cpu_ram();
    let (gpu_name, gpu_temp_c, gpu_vram_used_mb, gpu_vram_total_mb, gpu_usage_percent) =
        read_amd_gpu();

    QuickStats {
        cpu_percent,
        ram_used_gb,
        ram_total_gb,
        gpu_name,
        gpu_temp_c,
        gpu_vram_used_mb,
        gpu_vram_total_mb,
        gpu_usage_percent,
    }
}

/// Check if a service is reachable (timeout 3s)
#[tauri::command]
pub async fn cmd_check_service_health(service: String) -> Result<bool, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|e| e.to_string())?;

    let url = match service.as_str() {
        "ollama" => "http://localhost:11434/api/tags",
        "n8n" => "http://localhost:5678/healthz",
        "langflow" => "http://localhost:7860/health",
        "openwebui" => "http://localhost:3000/health",
        "grafana" => "http://localhost:4000/api/health",
        "searxng" => "http://localhost:8888/healthz",
        "comfyui" => "http://localhost:8188/system_stats",
        _ => return Err(format!("Unknown service: {}", service)),
    };

    match client.get(url).send().await {
        Ok(resp) => Ok(resp.status().is_success()),
        Err(_) => Ok(false),
    }
}

// --- Pure sysfs/procfs readers (no external crate deps) ---

fn read_cpu_ram() -> (f32, f32, f32) {
    use std::fs;

    // CPU: read /proc/stat for aggregate usage
    let cpu_percent = read_cpu_percent().unwrap_or(0.0);

    // RAM: read /proc/meminfo
    let meminfo = fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let mut total_kb: u64 = 0;
    let mut available_kb: u64 = 0;

    for line in meminfo.lines() {
        if let Some(val) = line.strip_prefix("MemTotal:") {
            total_kb = val.trim().split_whitespace().next()
                .and_then(|v| v.parse().ok()).unwrap_or(0);
        } else if let Some(val) = line.strip_prefix("MemAvailable:") {
            available_kb = val.trim().split_whitespace().next()
                .and_then(|v| v.parse().ok()).unwrap_or(0);
        }
    }

    let total_gb = total_kb as f32 / 1_048_576.0;
    let used_gb = (total_kb - available_kb) as f32 / 1_048_576.0;

    (cpu_percent, used_gb, total_gb)
}

/// Read CPU usage from /proc/stat (compares two snapshots 100ms apart)
fn read_cpu_percent() -> Option<f32> {
    use std::fs;
    use std::thread;
    use std::time::Duration;

    fn parse_cpu_line(content: &str) -> Option<(u64, u64)> {
        let line = content.lines().next()?;
        if !line.starts_with("cpu ") { return None; }
        let nums: Vec<u64> = line.split_whitespace().skip(1)
            .filter_map(|n| n.parse().ok()).collect();
        if nums.len() < 4 { return None; }
        let idle = nums[3];
        let total: u64 = nums.iter().sum();
        Some((idle, total))
    }

    let stat1 = fs::read_to_string("/proc/stat").ok()?;
    let (idle1, total1) = parse_cpu_line(&stat1)?;

    thread::sleep(Duration::from_millis(50));

    let stat2 = fs::read_to_string("/proc/stat").ok()?;
    let (idle2, total2) = parse_cpu_line(&stat2)?;

    let idle_delta = idle2.saturating_sub(idle1) as f32;
    let total_delta = total2.saturating_sub(total1) as f32;

    if total_delta == 0.0 { return Some(0.0); }
    Some(((total_delta - idle_delta) / total_delta) * 100.0)
}

/// Read AMD GPU stats directly from sysfs (<1ms)
fn read_amd_gpu() -> (Option<String>, Option<f32>, Option<u64>, Option<u64>, Option<f32>) {
    use std::fs;

    let cards = ["card1", "card0"];

    for card in &cards {
        let base = format!("/sys/class/drm/{}/device", card);

        // Check if this is an AMD GPU (vendor 0x1002)
        let vendor = fs::read_to_string(format!("{}/vendor", base)).unwrap_or_default();
        if !vendor.trim().starts_with("0x1002") {
            continue;
        }

        let name = fs::read_to_string(format!("{}/product_name", base))
            .ok()
            .map(|s| s.trim().to_string())
            .or_else(|| Some(format!("AMD GPU ({})", card)));

        // Temperature: scan hwmon directories
        let temp = (0..10).find_map(|i| {
            let path = format!("{}/hwmon/hwmon{}/temp1_input", base, i);
            fs::read_to_string(&path)
                .ok()
                .and_then(|s| s.trim().parse::<f32>().ok())
                .map(|t| t / 1000.0) // millidegrees → degrees
        });

        let vram_used = fs::read_to_string(format!("{}/mem_info_vram_used", base))
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .map(|b| b / 1_048_576); // bytes → MB

        let vram_total = fs::read_to_string(format!("{}/mem_info_vram_total", base))
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .map(|b| b / 1_048_576);

        let usage = fs::read_to_string(format!("{}/gpu_busy_percent", base))
            .ok()
            .and_then(|s| s.trim().parse::<f32>().ok());

        return (name, temp, vram_used, vram_total, usage);
    }

    (None, None, None, None, None)
}
