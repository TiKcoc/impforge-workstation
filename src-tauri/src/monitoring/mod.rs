//! NEXUS System Monitoring
//!
//! Real-time monitoring of:
//! - CPU usage, frequency, temperature
//! - Memory usage
//! - GPU usage, temperature, VRAM (AMD ROCm & NVIDIA CUDA)
//! - Power consumption (Wattage)
//! - Disk I/O
//! - Network I/O
//!
//! Historical tracking:
//! - Resource usage history (ring buffer)
//! - Per-process attribution (CPU, Memory, GPU)
//! - Duration tracking (how long resources stressed)
//!
//! AI Model Recommendations:
//! - Hardware-based model selection
//! - Quantization recommendations
//! - Performance optimization tips

pub mod quick;
pub mod recommendations;

use serde::{Deserialize, Serialize};
use sysinfo::{Components, CpuRefreshKind, Disks, MemoryRefreshKind, Networks, Pid, ProcessRefreshKind, RefreshKind, System, UpdateKind};
use systemstat::{Platform, System as SystemStat};
use std::sync::Mutex;
use std::collections::VecDeque;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use once_cell::sync::Lazy;

/// Global system info instance (cached)
static SYSTEM: Lazy<Mutex<System>> = Lazy::new(|| {
    Mutex::new(System::new_with_specifics(
        RefreshKind::new()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything()),
    ))
});

/// CPU information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    /// CPU name/model
    pub name: String,
    /// Number of physical cores
    pub physical_cores: usize,
    /// Number of logical cores (with HT)
    pub logical_cores: usize,
    /// Current frequency in MHz
    pub frequency_mhz: u64,
    /// Overall CPU usage (0-100%)
    pub usage_percent: f32,
    /// Per-core usage (0-100%)
    pub core_usage: Vec<f32>,
    /// CPU temperature in Celsius (if available)
    pub temperature_celsius: Option<f32>,
    /// CPU power consumption in Watts (if available)
    pub power_watts: Option<f32>,
}

/// Memory information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    /// Total RAM in bytes
    pub total_bytes: u64,
    /// Used RAM in bytes
    pub used_bytes: u64,
    /// Available RAM in bytes
    pub available_bytes: u64,
    /// RAM usage percentage
    pub usage_percent: f32,
    /// Total swap in bytes
    pub swap_total_bytes: u64,
    /// Used swap in bytes
    pub swap_used_bytes: u64,
}

/// GPU information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    /// GPU name/model
    pub name: String,
    /// GPU vendor (AMD, NVIDIA, Intel)
    pub vendor: String,
    /// VRAM total in bytes
    pub vram_total_bytes: u64,
    /// VRAM used in bytes
    pub vram_used_bytes: u64,
    /// GPU usage percentage
    pub usage_percent: Option<f32>,
    /// GPU temperature in Celsius
    pub temperature_celsius: Option<f32>,
    /// GPU power consumption in Watts
    pub power_watts: Option<f32>,
    /// GPU fan speed percentage
    pub fan_speed_percent: Option<f32>,
}

/// Disk information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub name: String,
    pub mount_point: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub usage_percent: f32,
    pub file_system: String,
}

/// Network interface information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub name: String,
    pub received_bytes: u64,
    pub transmitted_bytes: u64,
}

/// Complete system status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatus {
    pub cpu: CpuInfo,
    pub memory: MemoryInfo,
    pub gpus: Vec<GpuInfo>,
    pub disks: Vec<DiskInfo>,
    pub networks: Vec<NetworkInfo>,
    pub uptime_seconds: u64,
}

// ============================================================================
// HISTORICAL TRACKING - Duration & Attribution
// ============================================================================

/// Configuration for history tracking
const HISTORY_MAX_SAMPLES: usize = 3600; // 1 hour at 1 sample/sec
const HIGH_USAGE_THRESHOLD_CPU: f32 = 80.0;
const HIGH_USAGE_THRESHOLD_MEM: f32 = 80.0;
const HIGH_USAGE_THRESHOLD_GPU: f32 = 80.0;

/// Timestamp in milliseconds since UNIX epoch
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// A single sample of resource usage with timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSample {
    /// Timestamp in milliseconds since UNIX epoch
    pub timestamp_ms: u64,
    /// CPU usage percentage (0-100)
    pub cpu_percent: f32,
    /// Memory usage percentage (0-100)
    pub memory_percent: f32,
    /// GPU usage percentage (0-100), None if no GPU
    pub gpu_percent: Option<f32>,
    /// VRAM usage percentage (0-100), None if no GPU
    pub vram_percent: Option<f32>,
    /// CPU temperature in Celsius
    pub cpu_temp: Option<f32>,
    /// GPU temperature in Celsius
    pub gpu_temp: Option<f32>,
    /// CPU power in Watts
    pub cpu_power: Option<f32>,
    /// GPU power in Watts
    pub gpu_power: Option<f32>,
}

/// Per-process resource usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessUsage {
    /// Process ID
    pub pid: u32,
    /// Process name
    pub name: String,
    /// Command line (truncated)
    pub cmd: String,
    /// CPU usage percentage
    pub cpu_percent: f32,
    /// Memory usage in bytes
    pub memory_bytes: u64,
    /// Memory usage percentage
    pub memory_percent: f32,
    /// GPU usage percentage (if available, e.g., from nvidia-smi)
    pub gpu_percent: Option<f32>,
    /// VRAM usage in bytes (if available)
    pub vram_bytes: Option<u64>,
    /// How long this process has been running (seconds)
    pub runtime_seconds: u64,
    /// Accumulated CPU time (user + system) in seconds
    pub cpu_time_seconds: u64,
}

/// Duration statistics for high resource usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageDuration {
    /// Total time CPU was above threshold (seconds)
    pub cpu_high_duration_secs: u64,
    /// Total time Memory was above threshold (seconds)
    pub memory_high_duration_secs: u64,
    /// Total time GPU was above threshold (seconds)
    pub gpu_high_duration_secs: u64,
    /// Total time VRAM was above threshold (seconds)
    pub vram_high_duration_secs: u64,
    /// When tracking started (timestamp ms)
    pub tracking_started_ms: u64,
    /// Total tracking duration (seconds)
    pub tracking_duration_secs: u64,
}

/// Top resource consumers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopConsumers {
    /// Top CPU consumers
    pub cpu: Vec<ProcessUsage>,
    /// Top memory consumers
    pub memory: Vec<ProcessUsage>,
    /// Top GPU consumers (if available)
    pub gpu: Vec<ProcessUsage>,
}

/// Historical usage data with ring buffer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageHistory {
    /// Ring buffer of samples
    pub samples: Vec<ResourceSample>,
    /// Duration statistics
    pub duration: UsageDuration,
    /// Current top consumers
    pub top_consumers: TopConsumers,
}

/// Global history storage
static HISTORY: Lazy<Mutex<HistoryState>> = Lazy::new(|| {
    Mutex::new(HistoryState::new())
});

/// Internal history state
struct HistoryState {
    samples: VecDeque<ResourceSample>,
    tracking_started: Instant,
    cpu_high_duration: Duration,
    memory_high_duration: Duration,
    gpu_high_duration: Duration,
    vram_high_duration: Duration,
    last_sample_time: Option<Instant>,
    last_cpu_high: bool,
    last_mem_high: bool,
    last_gpu_high: bool,
    last_vram_high: bool,
}

impl HistoryState {
    fn new() -> Self {
        Self {
            samples: VecDeque::with_capacity(HISTORY_MAX_SAMPLES),
            tracking_started: Instant::now(),
            cpu_high_duration: Duration::ZERO,
            memory_high_duration: Duration::ZERO,
            gpu_high_duration: Duration::ZERO,
            vram_high_duration: Duration::ZERO,
            last_sample_time: None,
            last_cpu_high: false,
            last_mem_high: false,
            last_gpu_high: false,
            last_vram_high: false,
        }
    }

    fn add_sample(&mut self, sample: ResourceSample) {
        let now = Instant::now();

        // Calculate duration since last sample
        if let Some(last_time) = self.last_sample_time {
            let delta = now.duration_since(last_time);

            // Track high usage durations
            if self.last_cpu_high {
                self.cpu_high_duration += delta;
            }
            if self.last_mem_high {
                self.memory_high_duration += delta;
            }
            if self.last_gpu_high {
                self.gpu_high_duration += delta;
            }
            if self.last_vram_high {
                self.vram_high_duration += delta;
            }
        }

        // Update state for next iteration
        self.last_cpu_high = sample.cpu_percent >= HIGH_USAGE_THRESHOLD_CPU;
        self.last_mem_high = sample.memory_percent >= HIGH_USAGE_THRESHOLD_MEM;
        self.last_gpu_high = sample.gpu_percent.unwrap_or(0.0) >= HIGH_USAGE_THRESHOLD_GPU;
        self.last_vram_high = sample.vram_percent.unwrap_or(0.0) >= HIGH_USAGE_THRESHOLD_MEM;
        self.last_sample_time = Some(now);

        // Add to ring buffer
        if self.samples.len() >= HISTORY_MAX_SAMPLES {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
    }

    fn get_duration(&self) -> UsageDuration {
        let tracking_duration = self.tracking_started.elapsed();
        UsageDuration {
            cpu_high_duration_secs: self.cpu_high_duration.as_secs(),
            memory_high_duration_secs: self.memory_high_duration.as_secs(),
            gpu_high_duration_secs: self.gpu_high_duration.as_secs(),
            vram_high_duration_secs: self.vram_high_duration.as_secs(),
            tracking_started_ms: now_ms() - (tracking_duration.as_millis() as u64),
            tracking_duration_secs: tracking_duration.as_secs(),
        }
    }
}

/// Get CPU information
pub fn get_cpu_info() -> CpuInfo {
    let mut sys = SYSTEM.lock().unwrap();
    sys.refresh_cpu_all();

    let cpus = sys.cpus();
    let global_usage: f32 = cpus.iter().map(|c| c.cpu_usage()).sum::<f32>() / cpus.len() as f32;

    // Get CPU temperature from components
    let components = Components::new_with_refreshed_list();
    let cpu_temp = components.iter()
        .find(|c| {
            let label = c.label().to_lowercase();
            label.contains("cpu") || label.contains("k10temp") || label.contains("coretemp")
        })
        .map(|c| c.temperature());

    // Try to get CPU power from RAPL (Linux)
    let cpu_power = read_cpu_power_rapl();

    CpuInfo {
        name: cpus.first().map(|c| c.brand().to_string()).unwrap_or_default(),
        physical_cores: sys.physical_core_count().unwrap_or(0),
        logical_cores: cpus.len(),
        frequency_mhz: cpus.first().map(|c| c.frequency()).unwrap_or(0),
        usage_percent: global_usage,
        core_usage: cpus.iter().map(|c| c.cpu_usage()).collect(),
        temperature_celsius: cpu_temp,
        power_watts: cpu_power,
    }
}

/// Get memory information
pub fn get_memory_info() -> MemoryInfo {
    let mut sys = SYSTEM.lock().unwrap();
    sys.refresh_memory();

    let total = sys.total_memory();
    let used = sys.used_memory();
    let available = sys.available_memory();

    MemoryInfo {
        total_bytes: total,
        used_bytes: used,
        available_bytes: available,
        usage_percent: (used as f32 / total as f32) * 100.0,
        swap_total_bytes: sys.total_swap(),
        swap_used_bytes: sys.used_swap(),
    }
}

/// Get GPU information (AMD ROCm + NVIDIA + Intel)
pub fn get_gpu_info() -> Vec<GpuInfo> {
    let mut gpus = Vec::new();

    // Try AMD ROCm first (supports multiple cards)
    gpus.extend(get_all_amd_gpus());

    // Try NVIDIA GPUs
    #[cfg(feature = "nvidia")]
    gpus.extend(get_nvidia_gpu_info());

    // Fallback: try nvidia-smi parsing if nvml not available
    #[cfg(not(feature = "nvidia"))]
    gpus.extend(get_nvidia_gpu_via_smi());

    // Try Intel integrated GPU
    if let Some(intel) = get_intel_gpu_info() {
        gpus.push(intel);
    }

    gpus
}

/// Get all AMD GPUs (supports multiple cards)
fn get_all_amd_gpus() -> Vec<GpuInfo> {
    use std::fs;

    let mut gpus = Vec::new();

    // Check card0 through card7
    for i in 0..8 {
        let card_path = format!("/sys/class/drm/card{}/device", i);
        if std::path::Path::new(&card_path).exists() {
            // Check if it's an AMD GPU by vendor
            let vendor = fs::read_to_string(format!("{}/vendor", card_path))
                .ok()
                .and_then(|s| s.trim().strip_prefix("0x").map(|h| u32::from_str_radix(h, 16).ok()))
                .flatten();

            // AMD vendor ID is 0x1002
            if vendor == Some(0x1002) {
                if let Some(gpu) = get_amd_gpu_info_for_card(i) {
                    gpus.push(gpu);
                }
            }
        }
    }

    gpus
}

/// Get AMD GPU info for specific card
fn get_amd_gpu_info_for_card(card_index: usize) -> Option<GpuInfo> {
    use std::fs;
    use std::path::Path;

    let base = format!("/sys/class/drm/card{}/device", card_index);
    let hwmon_base = Path::new(&base).join("hwmon");

    if !hwmon_base.exists() {
        return None;
    }

    // Find hwmon directory
    let hwmon_dir = fs::read_dir(&hwmon_base).ok()?
        .filter_map(|e| e.ok())
        .find(|e| e.file_name().to_string_lossy().starts_with("hwmon"))?
        .path();

    // Read temperature (millidegrees Celsius)
    let temp = fs::read_to_string(hwmon_dir.join("temp1_input"))
        .ok()
        .and_then(|s| s.trim().parse::<f32>().ok())
        .map(|t| t / 1000.0);

    // Read power (microwatts)
    let power = fs::read_to_string(hwmon_dir.join("power1_average"))
        .ok()
        .and_then(|s| s.trim().parse::<f32>().ok())
        .map(|p| p / 1_000_000.0);

    // Read fan speed
    let fan_speed = fs::read_to_string(hwmon_dir.join("fan1_input"))
        .ok()
        .and_then(|s| s.trim().parse::<f32>().ok());
    let fan_max = fs::read_to_string(hwmon_dir.join("fan1_max"))
        .ok()
        .and_then(|s| s.trim().parse::<f32>().ok());
    let fan_percent = match (fan_speed, fan_max) {
        (Some(speed), Some(max)) if max > 0.0 => Some((speed / max) * 100.0),
        _ => None,
    };

    // Read GPU busy percent
    let gpu_busy = fs::read_to_string(format!("{}/gpu_busy_percent", base))
        .ok()
        .and_then(|s| s.trim().parse::<f32>().ok());

    // Read VRAM info
    let vram_total = fs::read_to_string(format!("{}/mem_info_vram_total", base))
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(0);

    let vram_used = fs::read_to_string(format!("{}/mem_info_vram_used", base))
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(0);

    // Read GPU name
    let name = fs::read_to_string(format!("{}/product_name", base))
        .ok()
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| format!("AMD GPU {}", card_index));

    Some(GpuInfo {
        name,
        vendor: "AMD".to_string(),
        vram_total_bytes: vram_total,
        vram_used_bytes: vram_used,
        usage_percent: gpu_busy,
        temperature_celsius: temp,
        power_watts: power,
        fan_speed_percent: fan_percent,
    })
}

/// Get NVIDIA GPU info via nvidia-smi (fallback when nvml not available)
#[cfg(not(feature = "nvidia"))]
fn get_nvidia_gpu_via_smi() -> Vec<GpuInfo> {
    use std::process::Command;

    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total,memory.used,utilization.gpu,temperature.gpu,power.draw,fan.speed",
            "--format=csv,noheader,nounits"
        ])
        .output();

    let Ok(output) = output else {
        return Vec::new();
    };

    if !output.status.success() {
        return Vec::new();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
            if parts.len() < 7 {
                return None;
            }

            let name = parts[0].to_string();
            let vram_total_mb: u64 = parts[1].parse().unwrap_or(0);
            let vram_used_mb: u64 = parts[2].parse().unwrap_or(0);
            let usage_percent: f32 = parts[3].parse().unwrap_or(0.0);
            let temp: f32 = parts[4].parse().unwrap_or(0.0);
            let power: f32 = parts[5].parse().unwrap_or(0.0);
            let fan: f32 = parts[6].parse().unwrap_or(0.0);

            Some(GpuInfo {
                name,
                vendor: "NVIDIA".to_string(),
                vram_total_bytes: vram_total_mb * 1024 * 1024,
                vram_used_bytes: vram_used_mb * 1024 * 1024,
                usage_percent: Some(usage_percent),
                temperature_celsius: Some(temp),
                power_watts: Some(power),
                fan_speed_percent: Some(fan),
            })
        })
        .collect()
}

/// Get NVIDIA GPU info via NVML (when nvidia feature enabled)
#[cfg(feature = "nvidia")]
fn get_nvidia_gpu_info() -> Vec<GpuInfo> {
    use nvml_wrapper::Nvml;

    let Ok(nvml) = Nvml::init() else {
        return Vec::new();
    };

    let Ok(device_count) = nvml.device_count() else {
        return Vec::new();
    };

    (0..device_count)
        .filter_map(|i| {
            let device = nvml.device_by_index(i).ok()?;

            let name = device.name().ok()?;
            let memory = device.memory_info().ok()?;
            let utilization = device.utilization_rates().ok();
            let temp = device.temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu).ok();
            let power = device.power_usage().ok().map(|p| p as f32 / 1000.0);
            let fan = device.fan_speed(0).ok().map(|f| f as f32);

            Some(GpuInfo {
                name,
                vendor: "NVIDIA".to_string(),
                vram_total_bytes: memory.total,
                vram_used_bytes: memory.used,
                usage_percent: utilization.map(|u| u.gpu as f32),
                temperature_celsius: temp.map(|t| t as f32),
                power_watts: power,
                fan_speed_percent: fan,
            })
        })
        .collect()
}

// NOTE: Old get_amd_gpu_info removed - now using get_amd_gpu_info_for_card for multi-GPU support

// ============================================================================
// AI MODEL TRACKING - Local LLM Resource Usage
// ============================================================================

/// AI model resource usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelUsage {
    /// Model name/ID (e.g., "llama3.2:latest", "qwen2.5-coder:7b")
    pub model_name: String,
    /// Provider (Ollama, llama.cpp, vLLM, etc.)
    pub provider: String,
    /// Process ID (if running)
    pub pid: Option<u32>,
    /// VRAM usage in bytes
    pub vram_bytes: u64,
    /// RAM usage in bytes
    pub ram_bytes: u64,
    /// GPU utilization percentage
    pub gpu_percent: Option<f32>,
    /// CPU utilization percentage
    pub cpu_percent: f32,
    /// Running time in seconds
    pub running_seconds: u64,
    /// Tokens processed (if available)
    pub tokens_processed: Option<u64>,
    /// Tokens per second (inference speed)
    pub tokens_per_second: Option<f32>,
    /// Model size in bytes
    pub model_size_bytes: u64,
    /// Quantization type (Q4, Q5, Q8, FP16, etc.)
    pub quantization: Option<String>,
    /// Context window size
    pub context_size: Option<u32>,
}

/// Optimization suggestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationSuggestion {
    /// Suggestion type
    pub category: String,
    /// Title
    pub title: String,
    /// Description
    pub description: String,
    /// Potential savings (percentage or bytes)
    pub potential_savings: String,
    /// Priority (1-5, 5 being highest)
    pub priority: u8,
}

/// Get currently running AI models and their resource usage
pub fn get_model_usage() -> Vec<ModelUsage> {
    let mut models = Vec::new();

    // Check Ollama models
    models.extend(get_ollama_models_usage());

    // Check llama.cpp processes
    models.extend(get_llamacpp_usage());

    // Check vLLM processes
    models.extend(get_vllm_usage());

    // Check HuggingFace Transformers / TGI processes
    models.extend(get_huggingface_usage());

    // Check text-generation-inference (TGI)
    models.extend(get_tgi_usage());

    // Check Candle processes
    models.extend(get_candle_usage());

    // Check ExLlamaV2 processes
    models.extend(get_exllamav2_usage());

    models
}

/// Get HuggingFace Transformers model usage
fn get_huggingface_usage() -> Vec<ModelUsage> {
    let sys = System::new_all();
    let gpu_vram_by_pid = get_gpu_vram_by_pid();
    let mut models = Vec::new();

    for (pid, proc) in sys.processes() {
        let cmd = proc.cmd().join(" ");

        // Detect HuggingFace transformers usage patterns
        let is_hf = cmd.contains("transformers")
            || cmd.contains("from_pretrained")
            || cmd.contains("AutoModelFor")
            || cmd.contains("huggingface")
            || cmd.contains("accelerate launch");

        if is_hf {
            let pid_u32 = pid.as_u32();
            let model_name = extract_hf_model_from_cmd(&cmd)
                .unwrap_or_else(|| "HuggingFace model".to_string());

            models.push(ModelUsage {
                model_name,
                provider: "HuggingFace".to_string(),
                pid: Some(pid_u32),
                vram_bytes: gpu_vram_by_pid.get(&pid_u32).copied().unwrap_or(0),
                ram_bytes: proc.memory(),
                gpu_percent: None,
                cpu_percent: proc.cpu_usage(),
                running_seconds: proc.run_time(),
                tokens_processed: None,
                tokens_per_second: None,
                model_size_bytes: 0,
                quantization: extract_hf_quantization(&cmd),
                context_size: None,
            });
        }
    }

    models
}

/// Get Text Generation Inference (TGI) usage
fn get_tgi_usage() -> Vec<ModelUsage> {
    use std::process::Command;

    let mut models = Vec::new();
    let gpu_vram_by_pid = get_gpu_vram_by_pid();
    let sys = System::new_all();

    // Find TGI processes
    for (pid, proc) in sys.processes() {
        let name = proc.name().to_string_lossy().to_lowercase();
        let cmd = proc.cmd().join(" ");

        if name.contains("text-generation") || cmd.contains("text-generation-launcher") {
            let pid_u32 = pid.as_u32();
            let model_name = extract_model_from_cmd(&cmd)
                .or_else(|| extract_hf_model_from_cmd(&cmd))
                .unwrap_or_else(|| "TGI model".to_string());

            models.push(ModelUsage {
                model_name,
                provider: "TGI".to_string(),
                pid: Some(pid_u32),
                vram_bytes: gpu_vram_by_pid.get(&pid_u32).copied().unwrap_or(0),
                ram_bytes: proc.memory(),
                gpu_percent: None,
                cpu_percent: proc.cpu_usage(),
                running_seconds: proc.run_time(),
                tokens_processed: None,
                tokens_per_second: None,
                model_size_bytes: 0,
                quantization: extract_hf_quantization(&cmd),
                context_size: extract_context_size(&cmd),
            });
        }
    }

    // Also try TGI API for running models
    if let Ok(output) = Command::new("curl")
        .args(["-s", "http://localhost:8080/info"])
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
                if let Some(model_id) = json.get("model_id").and_then(|m| m.as_str()) {
                    // Check if we already have this model from process scan
                    if !models.iter().any(|m| m.model_name == model_id) {
                        models.push(ModelUsage {
                            model_name: model_id.to_string(),
                            provider: "TGI".to_string(),
                            pid: None,
                            vram_bytes: 0,
                            ram_bytes: 0,
                            gpu_percent: None,
                            cpu_percent: 0.0,
                            running_seconds: 0,
                            tokens_processed: json.get("total_tokens").and_then(|t| t.as_u64()),
                            tokens_per_second: None,
                            model_size_bytes: 0,
                            quantization: json.get("quantize").and_then(|q| q.as_str()).map(|s| s.to_string()),
                            context_size: json.get("max_input_length").and_then(|c| c.as_u64()).map(|c| c as u32),
                        });
                    }
                }
            }
        }
    }

    models
}

/// Get Candle model usage
fn get_candle_usage() -> Vec<ModelUsage> {
    let sys = System::new_all();
    let gpu_vram_by_pid = get_gpu_vram_by_pid();
    let mut models = Vec::new();

    for (pid, proc) in sys.processes() {
        let cmd = proc.cmd().join(" ");

        // Detect Candle usage
        if cmd.contains("candle") || cmd.contains("--model-id") && cmd.contains("huggingface") {
            let pid_u32 = pid.as_u32();
            let model_name = extract_hf_model_from_cmd(&cmd)
                .unwrap_or_else(|| "Candle model".to_string());

            models.push(ModelUsage {
                model_name,
                provider: "Candle".to_string(),
                pid: Some(pid_u32),
                vram_bytes: gpu_vram_by_pid.get(&pid_u32).copied().unwrap_or(0),
                ram_bytes: proc.memory(),
                gpu_percent: None,
                cpu_percent: proc.cpu_usage(),
                running_seconds: proc.run_time(),
                tokens_processed: None,
                tokens_per_second: None,
                model_size_bytes: 0,
                quantization: extract_quantization(&cmd),
                context_size: None,
            });
        }
    }

    models
}

/// Get ExLlamaV2 model usage
fn get_exllamav2_usage() -> Vec<ModelUsage> {
    let sys = System::new_all();
    let gpu_vram_by_pid = get_gpu_vram_by_pid();
    let mut models = Vec::new();

    for (pid, proc) in sys.processes() {
        let cmd = proc.cmd().join(" ");

        // Detect ExLlamaV2 usage
        if cmd.contains("exllamav2") || cmd.contains("exl2") || cmd.contains("exllama") {
            let pid_u32 = pid.as_u32();
            let model_name = extract_model_from_cmd(&cmd)
                .unwrap_or_else(|| "ExLlamaV2 model".to_string());

            models.push(ModelUsage {
                model_name,
                provider: "ExLlamaV2".to_string(),
                pid: Some(pid_u32),
                vram_bytes: gpu_vram_by_pid.get(&pid_u32).copied().unwrap_or(0),
                ram_bytes: proc.memory(),
                gpu_percent: None,
                cpu_percent: proc.cpu_usage(),
                running_seconds: proc.run_time(),
                tokens_processed: None,
                tokens_per_second: None,
                model_size_bytes: 0,
                quantization: Some("EXL2".to_string()), // ExLlamaV2 uses EXL2 format
                context_size: extract_context_size(&cmd),
            });
        }
    }

    models
}

/// Extract HuggingFace model name from command
fn extract_hf_model_from_cmd(cmd: &str) -> Option<String> {
    // Patterns: --model-id xxx, --model xxx, model_name="xxx", "meta-llama/..."
    let patterns = [
        "--model-id ", "--model-id=",
        "--model ", "--model=",
        "model_name=\"", "model_name='",
        "model_id=\"", "model_id='",
    ];

    for pattern in patterns {
        if let Some(idx) = cmd.find(pattern) {
            let start = idx + pattern.len();
            let rest = &cmd[start..];
            // Handle quoted strings
            let end = if pattern.ends_with('"') || pattern.ends_with('\'') {
                rest.find(|c| c == '"' || c == '\'').unwrap_or(rest.len())
            } else {
                rest.find(' ').unwrap_or(rest.len())
            };
            return Some(rest[..end].to_string());
        }
    }

    // Try to find HuggingFace model format: org/model
    let words: Vec<&str> = cmd.split_whitespace().collect();
    for word in words {
        if word.contains('/') && !word.starts_with('/') && !word.starts_with("http") {
            let parts: Vec<&str> = word.split('/').collect();
            if parts.len() == 2 && parts[0].len() > 1 && parts[1].len() > 1 {
                return Some(word.to_string());
            }
        }
    }

    None
}

/// Extract HuggingFace quantization from command
fn extract_hf_quantization(cmd: &str) -> Option<String> {
    let cmd_lower = cmd.to_lowercase();

    // Check for BitsAndBytes quantization
    if cmd_lower.contains("load_in_4bit") || cmd_lower.contains("bnb_4bit") {
        return Some("BNB-4bit".to_string());
    }
    if cmd_lower.contains("load_in_8bit") || cmd_lower.contains("bnb_8bit") {
        return Some("BNB-8bit".to_string());
    }

    // Check for GPTQ
    if cmd_lower.contains("gptq") {
        return Some("GPTQ".to_string());
    }

    // Check for AWQ
    if cmd_lower.contains("awq") {
        return Some("AWQ".to_string());
    }

    // Check for EXL2
    if cmd_lower.contains("exl2") {
        return Some("EXL2".to_string());
    }

    // Check for GGUF/GGML
    extract_quantization(cmd)
}

/// Get Ollama running models
fn get_ollama_models_usage() -> Vec<ModelUsage> {
    use std::process::Command;

    // Get running models from Ollama API
    let output = Command::new("curl")
        .args(["-s", "http://localhost:11434/api/ps"])
        .output();

    let Ok(output) = output else {
        return Vec::new();
    };

    if !output.status.success() {
        return Vec::new();
    }

    // Parse JSON response
    let stdout = String::from_utf8_lossy(&output.stdout);
    let Ok(json): Result<serde_json::Value, _> = serde_json::from_str(&stdout) else {
        return Vec::new();
    };

    let Some(models) = json.get("models").and_then(|m| m.as_array()) else {
        return Vec::new();
    };

    models
        .iter()
        .filter_map(|model| {
            let name = model.get("name")?.as_str()?.to_string();
            let size = model.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
            let vram_size = model.get("size_vram").and_then(|s| s.as_u64()).unwrap_or(0);

            // Get process details
            let (pid, cpu_percent, ram_bytes, running_secs) = get_process_by_name("ollama");

            Some(ModelUsage {
                model_name: name,
                provider: "Ollama".to_string(),
                pid,
                vram_bytes: vram_size,
                ram_bytes,
                gpu_percent: None,
                cpu_percent,
                running_seconds: running_secs,
                tokens_processed: None,
                tokens_per_second: None,
                model_size_bytes: size,
                quantization: extract_quantization(&model.get("name").and_then(|n| n.as_str()).unwrap_or("")),
                context_size: model.get("details").and_then(|d| d.get("num_ctx")).and_then(|c| c.as_u64()).map(|c| c as u32),
            })
        })
        .collect()
}

/// Get llama.cpp server usage (CPU-first inference engine)
/// llama.cpp supports both CPU and GPU inference via n_gpu_layers
fn get_llamacpp_usage() -> Vec<ModelUsage> {
    let mut models = Vec::new();

    // Get GPU process VRAM mapping (for hybrid CPU+GPU mode)
    let gpu_vram_by_pid = get_gpu_vram_by_pid();

    // Find llama-server, llama-cli, or main (llama.cpp) processes
    let sys = System::new_all();

    for (pid, proc) in sys.processes() {
        let name = proc.name().to_string_lossy().to_lowercase();
        if name.contains("llama") || name.contains("llama-server") || name.contains("llama-cli")
           || name.contains("main") && proc.cmd().join(" ").contains(".gguf") {
            let cmd = proc.cmd().join(" ");
            let model_name = extract_model_from_cmd(&cmd).unwrap_or_else(|| "llama.cpp model".to_string());
            let pid_u32 = pid.as_u32();

            // Detect GPU layers (-ngl, --n-gpu-layers)
            let gpu_layers = extract_gpu_layers(&cmd);
            let is_cpu_only = gpu_layers == Some(0) || gpu_layers.is_none();

            // Get VRAM (only if using GPU layers)
            let vram = if is_cpu_only {
                0
            } else {
                gpu_vram_by_pid.get(&pid_u32).copied().unwrap_or(0)
            };

            // RAM usage is primary metric for llama.cpp (CPU inference)
            let ram_bytes = proc.memory();

            // Detect thread count for CPU optimization
            let threads = extract_thread_count(&cmd);

            models.push(ModelUsage {
                model_name: format!("{}{}",
                    model_name,
                    if is_cpu_only { " (CPU)" } else { " (CPU+GPU)" }
                ),
                provider: if is_cpu_only { "llama.cpp-CPU".to_string() } else { "llama.cpp-Hybrid".to_string() },
                pid: Some(pid_u32),
                vram_bytes: vram,
                ram_bytes,
                gpu_percent: if is_cpu_only { Some(0.0) } else { None },
                cpu_percent: proc.cpu_usage(),
                running_seconds: proc.run_time(),
                tokens_processed: None,
                tokens_per_second: None,
                model_size_bytes: estimate_model_size_from_ram(ram_bytes, vram),
                quantization: extract_quantization(&cmd),
                context_size: extract_context_size(&cmd),
            });
        }
    }

    models
}

/// Extract GPU layers from llama.cpp command line
fn extract_gpu_layers(cmd: &str) -> Option<i32> {
    let patterns = ["-ngl ", "--n-gpu-layers ", "--n-gpu-layers=", "-ngl="];

    for pattern in patterns {
        if let Some(idx) = cmd.find(pattern) {
            let start = idx + pattern.len();
            let rest = &cmd[start..];
            let end = rest.find(' ').unwrap_or(rest.len());
            if let Ok(layers) = rest[..end].parse::<i32>() {
                return Some(layers);
            }
        }
    }

    None
}

/// Extract thread count from llama.cpp command line
fn extract_thread_count(cmd: &str) -> Option<u32> {
    let patterns = ["-t ", "--threads ", "--threads=", "-t="];

    for pattern in patterns {
        if let Some(idx) = cmd.find(pattern) {
            let start = idx + pattern.len();
            let rest = &cmd[start..];
            let end = rest.find(' ').unwrap_or(rest.len());
            if let Ok(threads) = rest[..end].parse::<u32>() {
                return Some(threads);
            }
        }
    }

    None
}

/// Extract batch size from llama.cpp command line
fn extract_batch_size(cmd: &str) -> Option<u32> {
    let patterns = ["-b ", "--batch-size ", "--batch-size=", "-b="];

    for pattern in patterns {
        if let Some(idx) = cmd.find(pattern) {
            let start = idx + pattern.len();
            let rest = &cmd[start..];
            let end = rest.find(' ').unwrap_or(rest.len());
            if let Ok(batch) = rest[..end].parse::<u32>() {
                return Some(batch);
            }
        }
    }

    None
}

/// Estimate model size from RAM + VRAM usage
fn estimate_model_size_from_ram(ram_bytes: u64, vram_bytes: u64) -> u64 {
    // Model is distributed across RAM and VRAM
    // Subtract ~500MB overhead for runtime
    let overhead = 500 * 1024 * 1024;
    (ram_bytes + vram_bytes).saturating_sub(overhead)
}

/// Get VRAM usage by PID from nvidia-smi and rocm-smi
fn get_gpu_vram_by_pid() -> std::collections::HashMap<u32, u64> {
    use std::collections::HashMap;
    use std::process::Command;

    let mut vram_map = HashMap::new();

    // Try NVIDIA first
    if let Ok(output) = Command::new("nvidia-smi")
        .args(["--query-compute-apps=pid,used_gpu_memory", "--format=csv,noheader,nounits"])
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
                if parts.len() >= 2 {
                    if let (Ok(pid), Ok(vram_mb)) = (parts[0].parse::<u32>(), parts[1].parse::<u64>()) {
                        vram_map.insert(pid, vram_mb * 1024 * 1024);
                    }
                }
            }
        }
    }

    // Try AMD ROCm
    if let Ok(output) = Command::new("rocm-smi").args(["--showpidgpumem"]).output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                // Parse ROCm output format: PID GPU_ID VRAM(MB)
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    if let (Ok(pid), Ok(vram_mb)) = (parts[0].parse::<u32>(), parts[2].parse::<u64>()) {
                        *vram_map.entry(pid).or_insert(0) += vram_mb * 1024 * 1024;
                    }
                }
            }
        }
    }

    vram_map
}

/// Get vLLM usage
fn get_vllm_usage() -> Vec<ModelUsage> {
    let sys = System::new_all();
    let mut models = Vec::new();

    for (pid, proc) in sys.processes() {
        let cmd = proc.cmd().join(" ");
        if cmd.contains("vllm") {
            let model_name = extract_model_from_cmd(&cmd).unwrap_or_else(|| "vLLM model".to_string());

            models.push(ModelUsage {
                model_name,
                provider: "vLLM".to_string(),
                pid: Some(pid.as_u32()),
                vram_bytes: 0,
                ram_bytes: proc.memory(),
                gpu_percent: None,
                cpu_percent: proc.cpu_usage(),
                running_seconds: proc.run_time(),
                tokens_processed: None,
                tokens_per_second: None,
                model_size_bytes: 0,
                quantization: None,
                context_size: None,
            });
        }
    }

    models
}

/// Get process details by name
fn get_process_by_name(name: &str) -> (Option<u32>, f32, u64, u64) {
    let sys = System::new_all();

    for (pid, proc) in sys.processes() {
        if proc.name().to_string_lossy().to_lowercase().contains(name) {
            return (Some(pid.as_u32()), proc.cpu_usage(), proc.memory(), proc.run_time());
        }
    }

    (None, 0.0, 0, 0)
}

/// Extract model name from command line
fn extract_model_from_cmd(cmd: &str) -> Option<String> {
    // Common patterns: -m model.gguf, --model model.gguf
    let patterns = ["-m ", "--model ", "--model="];

    for pattern in patterns {
        if let Some(idx) = cmd.find(pattern) {
            let start = idx + pattern.len();
            let rest = &cmd[start..];
            let end = rest.find(' ').unwrap_or(rest.len());
            let model_path = &rest[..end];
            // Extract filename from path
            return Some(model_path.rsplit('/').next().unwrap_or(model_path).to_string());
        }
    }

    None
}

/// Extract quantization from model name or command
fn extract_quantization(s: &str) -> Option<String> {
    let s = s.to_uppercase();
    let quantizations = ["Q8_0", "Q6_K", "Q5_K_M", "Q5_K_S", "Q5_0", "Q4_K_M", "Q4_K_S", "Q4_0", "Q3_K_M", "Q2_K", "FP16", "F16", "IQ4", "IQ3", "IQ2"];

    for q in quantizations {
        if s.contains(q) {
            return Some(q.to_string());
        }
    }

    None
}

/// Extract context size from command line
fn extract_context_size(cmd: &str) -> Option<u32> {
    let patterns = ["-c ", "--ctx-size ", "--ctx-size=", "-ctx "];

    for pattern in patterns {
        if let Some(idx) = cmd.find(pattern) {
            let start = idx + pattern.len();
            let rest = &cmd[start..];
            let end = rest.find(' ').unwrap_or(rest.len());
            if let Ok(ctx) = rest[..end].parse::<u32>() {
                return Some(ctx);
            }
        }
    }

    None
}

/// Get optimization suggestions based on current usage
pub fn get_optimization_suggestions() -> Vec<OptimizationSuggestion> {
    let mut suggestions = Vec::new();

    let models = get_model_usage();
    let gpus = get_gpu_info();
    let memory = get_memory_info();
    let cpu = get_cpu_info();
    let top_processes = get_top_processes(5);

    // =========================================================================
    // CPU OPTIMIZATIONS
    // =========================================================================

    // Check CPU temperature (thermal throttling risk)
    if let Some(temp) = cpu.temperature_celsius {
        if temp > 85.0 {
            suggestions.push(OptimizationSuggestion {
                category: "CPU".to_string(),
                title: "CPU Thermal Throttling Risk".to_string(),
                description: format!(
                    "CPU temperature is {:.1}°C. Consider improving cooling or reducing workload to prevent thermal throttling.",
                    temp
                ),
                potential_savings: "15-30% performance improvement".to_string(),
                priority: 5,
            });
        } else if temp > 75.0 {
            suggestions.push(OptimizationSuggestion {
                category: "CPU".to_string(),
                title: "High CPU Temperature".to_string(),
                description: format!(
                    "CPU is running hot at {:.1}°C. Monitor closely during heavy workloads.",
                    temp
                ),
                potential_savings: "Prevent throttling".to_string(),
                priority: 3,
            });
        }
    }

    // Check CPU usage imbalance (single-threaded bottleneck)
    if cpu.logical_cores > 1 && !cpu.core_usage.is_empty() {
        let max_core = cpu.core_usage.iter().cloned().fold(0.0f32, f32::max);
        let avg_core: f32 = cpu.core_usage.iter().sum::<f32>() / cpu.core_usage.len() as f32;

        if max_core > 90.0 && avg_core < 40.0 {
            suggestions.push(OptimizationSuggestion {
                category: "CPU".to_string(),
                title: "Single-Threaded Bottleneck Detected".to_string(),
                description: format!(
                    "One core at {:.0}% while average is {:.0}%. Application may be single-threaded. Consider multi-threaded alternatives.",
                    max_core, avg_core
                ),
                potential_savings: format!("{}x potential speedup with parallelization", cpu.logical_cores / 2),
                priority: 4,
            });
        }
    }

    // Check high CPU power draw
    if let Some(power) = cpu.power_watts {
        if power > 100.0 {
            suggestions.push(OptimizationSuggestion {
                category: "CPU".to_string(),
                title: "High CPU Power Consumption".to_string(),
                description: format!(
                    "CPU drawing {:.0}W. Consider limiting CPU frequency or cores for sustained workloads.",
                    power
                ),
                potential_savings: "30-50% power reduction".to_string(),
                priority: 2,
            });
        }
    }

    // =========================================================================
    // PROCESS-SPECIFIC OPTIMIZATIONS
    // =========================================================================

    // Check for runaway processes (high CPU, long runtime)
    for proc in &top_processes.cpu {
        if proc.cpu_percent > 80.0 && proc.runtime_seconds > 3600 {
            suggestions.push(OptimizationSuggestion {
                category: "Process".to_string(),
                title: format!("High CPU process: {}", proc.name),
                description: format!(
                    "'{}' (PID {}) has used {:.0}% CPU for {} hours. Consider if this is expected.",
                    proc.name, proc.pid, proc.cpu_percent, proc.runtime_seconds / 3600
                ),
                potential_savings: format!("{:.0}% CPU recovery", proc.cpu_percent),
                priority: 3,
            });
        }
    }

    // Check for memory hogs
    for proc in &top_processes.memory {
        if proc.memory_percent > 20.0 {
            suggestions.push(OptimizationSuggestion {
                category: "Process".to_string(),
                title: format!("High memory process: {}", proc.name),
                description: format!(
                    "'{}' using {:.1}% of RAM ({:.1}GB). Consider memory limits or alternatives.",
                    proc.name, proc.memory_percent, proc.memory_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
                ),
                potential_savings: format!("{:.1}GB RAM", proc.memory_bytes as f64 / (1024.0 * 1024.0 * 1024.0)),
                priority: if proc.memory_percent > 40.0 { 4 } else { 2 },
            });
        }
    }

    // Check GPU utilization
    for gpu in &gpus {
        if let Some(usage) = gpu.usage_percent {
            if usage < 30.0 && gpu.vram_used_bytes > 2 * 1024 * 1024 * 1024 {
                suggestions.push(OptimizationSuggestion {
                    category: "GPU".to_string(),
                    title: "Low GPU utilization with high VRAM usage".to_string(),
                    description: format!(
                        "{} is using {}GB VRAM but only {:.1}% GPU. Consider using smaller batch sizes or a smaller model.",
                        gpu.name,
                        gpu.vram_used_bytes / (1024 * 1024 * 1024),
                        usage
                    ),
                    potential_savings: format!("{:.0}% GPU efficiency gain", 100.0 - usage),
                    priority: 4,
                });
            }
        }

        // Check VRAM pressure
        if gpu.vram_total_bytes > 0 {
            let vram_percent = (gpu.vram_used_bytes as f32 / gpu.vram_total_bytes as f32) * 100.0;
            if vram_percent > 90.0 {
                suggestions.push(OptimizationSuggestion {
                    category: "VRAM".to_string(),
                    title: "VRAM near capacity".to_string(),
                    description: format!(
                        "{} VRAM is {:.1}% full. Consider using more aggressive quantization (Q4 instead of Q8) or offloading layers to CPU.",
                        gpu.name,
                        vram_percent
                    ),
                    potential_savings: "50% VRAM with Q4 vs Q8".to_string(),
                    priority: 5,
                });
            }
        }
    }

    // Check model-specific optimizations
    for model in &models {
        // Suggest quantization for large models
        if model.model_size_bytes > 10 * 1024 * 1024 * 1024 && model.quantization.is_none() {
            suggestions.push(OptimizationSuggestion {
                category: "Model".to_string(),
                title: format!("Large model without quantization: {}", model.model_name),
                description: "Consider using Q4_K_M or Q5_K_M quantization to reduce VRAM usage while maintaining quality.".to_string(),
                potential_savings: "60-75% VRAM reduction".to_string(),
                priority: 4,
            });
        }

        // Suggest upgrading Q2/Q3 to Q4 for better quality
        if let Some(ref quant) = model.quantization {
            if quant.contains("Q2") || quant.contains("Q3") || quant.contains("IQ2") {
                suggestions.push(OptimizationSuggestion {
                    category: "Model".to_string(),
                    title: format!("Low quantization quality: {}", model.model_name),
                    description: format!(
                        "'{}' uses {} quantization. Q4_K_M offers much better quality with only ~25% more VRAM.",
                        model.model_name, quant
                    ),
                    potential_savings: "Significant quality improvement".to_string(),
                    priority: 3,
                });
            }
        }

        // Check context size optimization
        if let Some(ctx) = model.context_size {
            if ctx > 32768 && model.vram_bytes > 8 * 1024 * 1024 * 1024 {
                suggestions.push(OptimizationSuggestion {
                    category: "Model".to_string(),
                    title: format!("Large context size: {}", model.model_name),
                    description: format!(
                        "'{}' using {}K context. Consider reducing to 8K-16K if not needed - saves significant VRAM.",
                        model.model_name, ctx / 1024
                    ),
                    potential_savings: "2-4GB VRAM per 32K context reduction".to_string(),
                    priority: 3,
                });
            }
        }

        // Check idle models consuming VRAM
        if model.vram_bytes > 1024 * 1024 * 1024 && model.cpu_percent < 1.0 && model.running_seconds > 300 {
            suggestions.push(OptimizationSuggestion {
                category: "Model".to_string(),
                title: format!("Idle model in VRAM: {}", model.model_name),
                description: format!(
                    "'{}' is using {:.1}GB VRAM but has been idle for {} minutes. Consider unloading.",
                    model.model_name,
                    model.vram_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
                    model.running_seconds / 60
                ),
                potential_savings: format!("{:.1}GB VRAM", model.vram_bytes as f64 / (1024.0 * 1024.0 * 1024.0)),
                priority: 4,
            });
        }
    }

    // Check for multiple models using GPU (only once, outside loop)
    let gpu_models: Vec<_> = models.iter().filter(|m| m.vram_bytes > 0).collect();
    if gpu_models.len() > 1 {
        let total_vram: u64 = gpu_models.iter().map(|m| m.vram_bytes).sum();
        suggestions.push(OptimizationSuggestion {
            category: "Model".to_string(),
            title: "Multiple models in VRAM".to_string(),
            description: format!(
                "{} models using {:.1}GB VRAM total. Consider unloading unused models with 'ollama stop <model>'.",
                gpu_models.len(),
                total_vram as f64 / (1024.0 * 1024.0 * 1024.0)
            ),
            potential_savings: "Free up VRAM for larger context".to_string(),
            priority: 3,
        });
    }

    // =========================================================================
    // CPU INFERENCE OPTIMIZATIONS (llama.cpp, etc.)
    // =========================================================================

    // Check for CPU-based models and optimize
    for model in &models {
        if model.provider.contains("CPU") || model.provider == "llama.cpp-CPU" {
            // Check RAM usage for CPU inference
            let ram_gb = model.ram_bytes as f64 / (1024.0 * 1024.0 * 1024.0);

            if ram_gb > 16.0 && memory.usage_percent > 70.0 {
                suggestions.push(OptimizationSuggestion {
                    category: "CPU-Inference".to_string(),
                    title: format!("Large CPU model: {}", model.model_name),
                    description: format!(
                        "'{}' using {:.1}GB RAM. Consider Q4 quantization or enable GPU layers (-ngl) to reduce RAM pressure.",
                        model.model_name, ram_gb
                    ),
                    potential_savings: "40-60% RAM with Q4 vs Q8".to_string(),
                    priority: 4,
                });
            }

            // Check CPU utilization for inference
            if model.cpu_percent < 200.0 && cpu.logical_cores > 8 {
                suggestions.push(OptimizationSuggestion {
                    category: "CPU-Inference".to_string(),
                    title: "Underutilized CPU threads".to_string(),
                    description: format!(
                        "'{}' using only {:.0}% CPU on {} cores. Increase threads with -t {} for faster inference.",
                        model.model_name, model.cpu_percent, cpu.logical_cores, cpu.logical_cores - 2
                    ),
                    potential_savings: format!("{}x potential speedup", cpu.logical_cores / 4),
                    priority: 3,
                });
            }

            // Check if model could benefit from GPU offload
            if model.vram_bytes == 0 {
                for gpu in &gpus {
                    let available_vram = gpu.vram_total_bytes.saturating_sub(gpu.vram_used_bytes);
                    let model_size_estimate = model.ram_bytes / 2; // Rough estimate for partial offload

                    if available_vram > model_size_estimate && available_vram > 2 * 1024 * 1024 * 1024 {
                        suggestions.push(OptimizationSuggestion {
                            category: "CPU-Inference".to_string(),
                            title: "GPU offload available".to_string(),
                            description: format!(
                                "'{}' is CPU-only but {} has {:.1}GB free VRAM. Use -ngl 20-40 for 2-3x speed boost.",
                                model.model_name, gpu.name, available_vram as f64 / (1024.0 * 1024.0 * 1024.0)
                            ),
                            potential_savings: "2-3x inference speed".to_string(),
                            priority: 4,
                        });
                        break; // Only suggest once
                    }
                }
            }
        }

        // Check hybrid CPU+GPU models
        if model.provider == "llama.cpp-Hybrid" {
            let vram_gb = model.vram_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
            let ram_gb = model.ram_bytes as f64 / (1024.0 * 1024.0 * 1024.0);

            // Check if more layers could be offloaded
            for gpu in &gpus {
                let available_vram = gpu.vram_total_bytes.saturating_sub(gpu.vram_used_bytes);
                if available_vram > 2 * 1024 * 1024 * 1024 && ram_gb > 4.0 {
                    suggestions.push(OptimizationSuggestion {
                        category: "CPU-Inference".to_string(),
                        title: "Increase GPU layers".to_string(),
                        description: format!(
                            "'{}' has {:.1}GB on CPU, but {} has {:.1}GB free. Increase -ngl for faster inference.",
                            model.model_name, ram_gb, gpu.name, available_vram as f64 / (1024.0 * 1024.0 * 1024.0)
                        ),
                        potential_savings: "30-50% faster inference".to_string(),
                        priority: 3,
                    });
                    break;
                }
            }
        }
    }

    // =========================================================================
    // RAM OPTIMIZATION FOR CPU INFERENCE
    // =========================================================================

    // Check total RAM used by AI models
    let total_model_ram: u64 = models.iter().map(|m| m.ram_bytes).sum();
    let model_ram_percent = (total_model_ram as f32 / memory.total_bytes as f32) * 100.0;

    if model_ram_percent > 50.0 {
        suggestions.push(OptimizationSuggestion {
            category: "RAM".to_string(),
            title: "AI models using significant RAM".to_string(),
            description: format!(
                "AI models using {:.1}GB ({:.0}% of RAM). Consider:\n• Smaller quantization (Q4_K_M)\n• Reduce context size (-c)\n• Use mmap for disk-backed inference",
                total_model_ram as f64 / (1024.0 * 1024.0 * 1024.0), model_ram_percent
            ),
            potential_savings: "30-50% RAM reduction".to_string(),
            priority: 4,
        });
    }

    // =========================================================================
    // GPU THERMAL & POWER OPTIMIZATIONS
    // =========================================================================

    for gpu in &gpus {
        // Check GPU temperature
        if let Some(temp) = gpu.temperature_celsius {
            if temp > 85.0 {
                suggestions.push(OptimizationSuggestion {
                    category: "GPU".to_string(),
                    title: format!("{} Thermal Throttling Risk", gpu.name),
                    description: format!(
                        "GPU at {:.0}°C - may be thermal throttling. Check fan curve, case airflow, or reduce workload.",
                        temp
                    ),
                    potential_savings: "10-20% performance recovery".to_string(),
                    priority: 5,
                });
            }
        }

        // Check GPU power efficiency
        if let (Some(power), Some(usage)) = (gpu.power_watts, gpu.usage_percent) {
            if power > 200.0 && usage < 50.0 {
                suggestions.push(OptimizationSuggestion {
                    category: "GPU".to_string(),
                    title: "Inefficient GPU power usage".to_string(),
                    description: format!(
                        "{} drawing {:.0}W at only {:.0}% utilization. Power limit may help reduce heat/noise.",
                        gpu.name, power, usage
                    ),
                    potential_savings: "30-50W power reduction".to_string(),
                    priority: 2,
                });
            }
        }

        // Check fan speed (cooling check)
        if let Some(fan) = gpu.fan_speed_percent {
            if fan > 90.0 {
                suggestions.push(OptimizationSuggestion {
                    category: "GPU".to_string(),
                    title: "GPU fans at maximum".to_string(),
                    description: format!(
                        "{} fans at {:.0}%. Consider improving case airflow or undervolting GPU.",
                        gpu.name, fan
                    ),
                    potential_savings: "Reduced noise, improved longevity".to_string(),
                    priority: 2,
                });
            }
        }
    }

    // =========================================================================
    // RAM & SWAP OPTIMIZATIONS
    // =========================================================================

    // Check RAM usage
    if memory.usage_percent > 85.0 {
        suggestions.push(OptimizationSuggestion {
            category: "RAM".to_string(),
            title: "High RAM usage".to_string(),
            description: format!(
                "System RAM is {:.1}% used ({:.1}GB/{:.1}GB). Consider closing unused applications or reducing model context size.",
                memory.usage_percent,
                memory.used_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
                memory.total_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
            ),
            potential_savings: "System stability improvement".to_string(),
            priority: 4,
        });
    }

    // Check swap usage
    if memory.swap_total_bytes > 0 {
        let swap_percent = (memory.swap_used_bytes as f32 / memory.swap_total_bytes as f32) * 100.0;
        if swap_percent > 50.0 {
            suggestions.push(OptimizationSuggestion {
                category: "RAM".to_string(),
                title: "Heavy swap usage".to_string(),
                description: format!(
                    "Swap is {:.0}% used ({:.1}GB). System may be slow. Close memory-intensive apps or add RAM.",
                    swap_percent,
                    memory.swap_used_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
                ),
                potential_savings: "Significant performance improvement".to_string(),
                priority: 5,
            });
        }
    }

    // =========================================================================
    // FINAL SORTING & DEDUPLICATION
    // =========================================================================

    // Remove duplicate suggestions (same title)
    suggestions.dedup_by(|a, b| a.title == b.title);

    // Sort by priority (highest first)
    suggestions.sort_by(|a, b| b.priority.cmp(&a.priority));

    suggestions
}

/// Read CPU power from Intel RAPL (Linux)
fn read_cpu_power_rapl() -> Option<f32> {
    use std::fs;
    use std::time::{Duration, Instant};
    use std::sync::atomic::{AtomicU64, Ordering};

    static LAST_ENERGY: AtomicU64 = AtomicU64::new(0);
    static LAST_TIME_NS: AtomicU64 = AtomicU64::new(0);

    let energy_path = "/sys/class/powercap/intel-rapl:0/energy_uj";
    let energy_str = fs::read_to_string(energy_path).ok()?;
    let energy_uj: u64 = energy_str.trim().parse().ok()?;

    let now = Instant::now();
    let now_ns = now.elapsed().as_nanos() as u64;

    let last_energy = LAST_ENERGY.swap(energy_uj, Ordering::SeqCst);
    let last_time = LAST_TIME_NS.swap(now_ns, Ordering::SeqCst);

    if last_time == 0 {
        return None; // First reading
    }

    let energy_delta = energy_uj.saturating_sub(last_energy);
    let time_delta_ns = now_ns.saturating_sub(last_time);

    if time_delta_ns == 0 {
        return None;
    }

    // Power = Energy / Time (convert µJ/ns to W)
    let power_w = (energy_delta as f64 / time_delta_ns as f64) * 1000.0;
    Some(power_w as f32)
}

/// Get disk information
pub fn get_disk_info() -> Vec<DiskInfo> {
    let disks = Disks::new_with_refreshed_list();

    disks.iter().map(|d| {
        let total = d.total_space();
        let available = d.available_space();
        let used = total - available;

        DiskInfo {
            name: d.name().to_string_lossy().to_string(),
            mount_point: d.mount_point().to_string_lossy().to_string(),
            total_bytes: total,
            available_bytes: available,
            usage_percent: if total > 0 { (used as f32 / total as f32) * 100.0 } else { 0.0 },
            file_system: d.file_system().to_string_lossy().to_string(),
        }
    }).collect()
}

/// Get network information
pub fn get_network_info() -> Vec<NetworkInfo> {
    let networks = Networks::new_with_refreshed_list();

    networks.iter().map(|(name, data)| {
        NetworkInfo {
            name: name.clone(),
            received_bytes: data.total_received(),
            transmitted_bytes: data.total_transmitted(),
        }
    }).collect()
}

/// Get system uptime
pub fn get_uptime() -> u64 {
    System::uptime()
}

// ============================================================================
// PROCESS TRACKING - Per-Process Attribution
// ============================================================================

/// Get top processes by resource usage
pub fn get_top_processes(limit: usize) -> TopConsumers {
    let mut sys = System::new_all();
    sys.refresh_all();

    let total_mem = sys.total_memory();

    // Collect all process info
    let mut processes: Vec<ProcessUsage> = sys
        .processes()
        .iter()
        .map(|(pid, proc)| {
            let mem_bytes = proc.memory();
            ProcessUsage {
                pid: pid.as_u32(),
                name: proc.name().to_string_lossy().to_string(),
                cmd: proc.cmd().join(" ").chars().take(100).collect(),
                cpu_percent: proc.cpu_usage(),
                memory_bytes: mem_bytes,
                memory_percent: (mem_bytes as f32 / total_mem as f32) * 100.0,
                gpu_percent: None, // Will be filled by GPU-specific tracking
                vram_bytes: None,
                runtime_seconds: proc.run_time(),
                cpu_time_seconds: proc.cumulative_process_time() / 1000, // ms to seconds
            }
        })
        .collect();

    // Get GPU process info if available
    let gpu_processes = get_gpu_process_usage();

    // Merge GPU info into processes
    for proc in &mut processes {
        if let Some(gpu_proc) = gpu_processes.iter().find(|g| g.pid == proc.pid) {
            proc.gpu_percent = gpu_proc.gpu_percent;
            proc.vram_bytes = gpu_proc.vram_bytes;
        }
    }

    // Sort by CPU and get top
    let mut cpu_sorted = processes.clone();
    cpu_sorted.sort_by(|a, b| b.cpu_percent.partial_cmp(&a.cpu_percent).unwrap_or(std::cmp::Ordering::Equal));
    let top_cpu: Vec<_> = cpu_sorted.into_iter().take(limit).collect();

    // Sort by memory and get top
    let mut mem_sorted = processes.clone();
    mem_sorted.sort_by(|a, b| b.memory_bytes.cmp(&a.memory_bytes));
    let top_memory: Vec<_> = mem_sorted.into_iter().take(limit).collect();

    // Sort by GPU and get top (only those with GPU usage)
    let mut gpu_sorted: Vec<_> = processes.into_iter().filter(|p| p.gpu_percent.is_some()).collect();
    gpu_sorted.sort_by(|a, b| {
        b.gpu_percent
            .unwrap_or(0.0)
            .partial_cmp(&a.gpu_percent.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let top_gpu: Vec<_> = gpu_sorted.into_iter().take(limit).collect();

    TopConsumers {
        cpu: top_cpu,
        memory: top_memory,
        gpu: top_gpu,
    }
}

/// Get GPU process usage (NVIDIA via nvidia-smi, AMD via rocm-smi)
fn get_gpu_process_usage() -> Vec<ProcessUsage> {
    let mut processes = Vec::new();

    // Try NVIDIA first
    processes.extend(get_nvidia_process_usage());

    // Try AMD ROCm
    processes.extend(get_amd_process_usage());

    processes
}

/// Get NVIDIA GPU process usage via nvidia-smi
fn get_nvidia_process_usage() -> Vec<ProcessUsage> {
    use std::process::Command;

    let output = Command::new("nvidia-smi")
        .args([
            "--query-compute-apps=pid,name,used_gpu_memory",
            "--format=csv,noheader,nounits"
        ])
        .output();

    let Ok(output) = output else {
        return Vec::new();
    };

    if !output.status.success() {
        return Vec::new();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
            if parts.len() < 3 {
                return None;
            }

            let pid: u32 = parts[0].parse().ok()?;
            let name = parts[1].to_string();
            let vram_mb: u64 = parts[2].parse().unwrap_or(0);

            Some(ProcessUsage {
                pid,
                name,
                cmd: String::new(),
                cpu_percent: 0.0,
                memory_bytes: 0,
                memory_percent: 0.0,
                gpu_percent: None, // nvidia-smi query-compute-apps doesn't give this
                vram_bytes: Some(vram_mb * 1024 * 1024),
                runtime_seconds: 0,
                cpu_time_seconds: 0,
            })
        })
        .collect()
}

/// Get AMD GPU process usage via rocm-smi
fn get_amd_process_usage() -> Vec<ProcessUsage> {
    use std::process::Command;

    // Try rocm-smi --showpids
    let output = Command::new("rocm-smi")
        .args(["--showpids"])
        .output();

    let Ok(output) = output else {
        return Vec::new();
    };

    if !output.status.success() {
        return Vec::new();
    }

    // Parse rocm-smi output (format varies by version)
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut processes = Vec::new();

    for line in stdout.lines() {
        // Skip header lines
        if line.contains("PID") || line.starts_with('=') || line.is_empty() {
            continue;
        }

        // Try to parse PID and VRAM
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            if let Ok(pid) = parts[0].parse::<u32>() {
                // Try to find VRAM usage (usually last numeric column)
                let vram = parts.iter().rev().find_map(|&p| {
                    p.trim_end_matches("MiB")
                        .trim_end_matches("MB")
                        .parse::<u64>()
                        .ok()
                        .map(|v| v * 1024 * 1024)
                });

                processes.push(ProcessUsage {
                    pid,
                    name: String::new(), // Will be filled from sysinfo
                    cmd: String::new(),
                    cpu_percent: 0.0,
                    memory_bytes: 0,
                    memory_percent: 0.0,
                    gpu_percent: None,
                    vram_bytes: vram,
                    runtime_seconds: 0,
                    cpu_time_seconds: 0,
                });
            }
        }
    }

    processes
}

// ============================================================================
// HISTORY RECORDING & RETRIEVAL
// ============================================================================

/// Record current system state to history
pub fn record_sample() {
    let cpu = get_cpu_info();
    let memory = get_memory_info();
    let gpus = get_gpu_info();

    let sample = ResourceSample {
        timestamp_ms: now_ms(),
        cpu_percent: cpu.usage_percent,
        memory_percent: memory.usage_percent,
        gpu_percent: gpus.first().and_then(|g| g.usage_percent),
        vram_percent: gpus.first().map(|g| {
            if g.vram_total_bytes > 0 {
                (g.vram_used_bytes as f32 / g.vram_total_bytes as f32) * 100.0
            } else {
                0.0
            }
        }),
        cpu_temp: cpu.temperature_celsius,
        gpu_temp: gpus.first().and_then(|g| g.temperature_celsius),
        cpu_power: cpu.power_watts,
        gpu_power: gpus.first().and_then(|g| g.power_watts),
    };

    let mut history = HISTORY.lock().unwrap();
    history.add_sample(sample);
}

/// Get usage history
pub fn get_usage_history(max_samples: Option<usize>) -> UsageHistory {
    let history = HISTORY.lock().unwrap();
    let limit = max_samples.unwrap_or(HISTORY_MAX_SAMPLES);

    // Get last N samples
    let samples: Vec<ResourceSample> = history
        .samples
        .iter()
        .rev()
        .take(limit)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    UsageHistory {
        samples,
        duration: history.get_duration(),
        top_consumers: get_top_processes(10),
    }
}

/// Get usage duration statistics
pub fn get_usage_duration() -> UsageDuration {
    let history = HISTORY.lock().unwrap();
    history.get_duration()
}

/// Clear history and reset tracking
pub fn reset_history() {
    let mut history = HISTORY.lock().unwrap();
    *history = HistoryState::new();
}

/// Get complete system status
pub fn get_system_status() -> SystemStatus {
    SystemStatus {
        cpu: get_cpu_info(),
        memory: get_memory_info(),
        gpus: get_gpu_info(),
        disks: get_disk_info(),
        networks: get_network_info(),
        uptime_seconds: get_uptime(),
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Tauri command: Get CPU info
#[tauri::command]
pub fn cmd_get_cpu_info() -> CpuInfo {
    get_cpu_info()
}

/// Tauri command: Get memory info
#[tauri::command]
pub fn cmd_get_memory_info() -> MemoryInfo {
    get_memory_info()
}

/// Tauri command: Get GPU info
#[tauri::command]
pub fn cmd_get_gpu_info() -> Vec<GpuInfo> {
    get_gpu_info()
}

/// Tauri command: Get full system status
#[tauri::command]
pub fn cmd_get_system_status() -> SystemStatus {
    get_system_status()
}

/// Tauri command: Get disk info
#[tauri::command]
pub fn cmd_get_disk_info() -> Vec<DiskInfo> {
    get_disk_info()
}

/// Tauri command: Get network info
#[tauri::command]
pub fn cmd_get_network_info() -> Vec<NetworkInfo> {
    get_network_info()
}

// ============================================================================
// Tauri Commands - Historical Tracking
// ============================================================================

/// Tauri command: Record a sample to history
#[tauri::command]
pub fn cmd_record_sample() {
    record_sample();
}

/// Tauri command: Get usage history
/// Returns time-series data for visualization
#[tauri::command]
pub fn cmd_get_usage_history(max_samples: Option<usize>) -> UsageHistory {
    get_usage_history(max_samples)
}

/// Tauri command: Get usage duration statistics
/// Shows how long CPU/GPU/RAM have been stressed
#[tauri::command]
pub fn cmd_get_usage_duration() -> UsageDuration {
    get_usage_duration()
}

/// Tauri command: Get top resource consumers
/// Shows which processes are using most CPU/RAM/GPU
#[tauri::command]
pub fn cmd_get_top_processes(limit: Option<usize>) -> TopConsumers {
    get_top_processes(limit.unwrap_or(10))
}

/// Tauri command: Reset history tracking
#[tauri::command]
pub fn cmd_reset_history() {
    reset_history();
}

/// Tauri command: Start background monitoring
/// Records samples at regular intervals
#[tauri::command]
pub async fn cmd_start_monitoring(interval_ms: u64) {
    use tokio::time::{interval, Duration};

    let mut timer = interval(Duration::from_millis(interval_ms.max(100)));

    // This runs in background - the frontend should call stop_monitoring to end it
    tokio::spawn(async move {
        loop {
            timer.tick().await;
            record_sample();
        }
    });
}

// ============================================================================
// Tauri Commands - AI Model Tracking
// ============================================================================

/// Tauri command: Get all running AI models and their resource usage
#[tauri::command]
pub fn cmd_get_model_usage() -> Vec<ModelUsage> {
    get_model_usage()
}

/// Tauri command: Get optimization suggestions for resource usage
#[tauri::command]
pub fn cmd_get_optimization_suggestions() -> Vec<OptimizationSuggestion> {
    get_optimization_suggestions()
}

// ============================================================================
// Quick Stats (lightweight, <5ms, for status bar polling)
// ============================================================================

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
    let mut sys = SYSTEM.lock().unwrap_or_else(|e| e.into_inner());
    sys.refresh_cpu_specifics(CpuRefreshKind::new().with_cpu_usage());
    sys.refresh_memory_kind(MemoryRefreshKind::new().with_ram());

    let cpu_percent = sys.global_cpu_usage();
    let ram_used_gb = sys.used_memory() as f32 / 1_073_741_824.0;
    let ram_total_gb = sys.total_memory() as f32 / 1_073_741_824.0;

    let (gpu_name, gpu_temp_c, gpu_vram_used_mb, gpu_vram_total_mb, gpu_usage_percent) =
        quick_amd_gpu_stats();

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

/// Read AMD GPU stats directly from sysfs (fastest path, <1ms)
fn quick_amd_gpu_stats() -> (Option<String>, Option<f32>, Option<u64>, Option<u64>, Option<f32>) {
    use std::fs;

    // Try card1 first (discrete GPU), then card0
    let cards = ["card1", "card0"];

    for card in &cards {
        let base = format!("/sys/class/drm/{}/device", card);

        // Check if this is an AMD GPU
        let vendor = fs::read_to_string(format!("{}/vendor", base)).unwrap_or_default();
        if !vendor.trim().starts_with("0x1002") {
            continue; // Not AMD
        }

        let name = fs::read_to_string(format!("{}/product_name", base))
            .ok()
            .map(|s| s.trim().to_string())
            .or_else(|| Some(format!("AMD GPU ({})", card)));

        // Temperature: find the right hwmon
        let temp = (0..10).find_map(|i| {
            let path = format!("{}/hwmon/hwmon{}/temp1_input", base, i);
            fs::read_to_string(&path)
                .ok()
                .and_then(|s| s.trim().parse::<f32>().ok())
                .map(|t| t / 1000.0)
        });

        let vram_used = fs::read_to_string(format!("{}/mem_info_vram_used", base))
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .map(|b| b / 1_048_576);

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

// ============================================================================
// Service Health Check
// ============================================================================

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

// ============================================================================
// INTEL GPU SUPPORT (via i915 sysfs)
// ============================================================================

/// Get Intel GPU info via sysfs
fn get_intel_gpu_info() -> Option<GpuInfo> {
    use std::fs;

    // Check for Intel i915 driver
    let card_path = "/sys/class/drm/card0/device";
    let vendor = fs::read_to_string(format!("{}/vendor", card_path))
        .ok()
        .and_then(|s| s.trim().strip_prefix("0x").map(|h| u32::from_str_radix(h, 16).ok()))
        .flatten();

    // Intel vendor ID is 0x8086
    if vendor != Some(0x8086) {
        return None;
    }

    // Read GPU frequency
    let freq_cur = fs::read_to_string("/sys/class/drm/card0/gt_cur_freq_mhz")
        .ok()
        .and_then(|s| s.trim().parse::<f32>().ok());

    let freq_max = fs::read_to_string("/sys/class/drm/card0/gt_max_freq_mhz")
        .ok()
        .and_then(|s| s.trim().parse::<f32>().ok());

    // Calculate usage based on frequency scaling
    let usage = match (freq_cur, freq_max) {
        (Some(cur), Some(max)) if max > 0.0 => Some((cur / max) * 100.0),
        _ => None,
    };

    // Try to get memory info from i915 debugfs
    let vram_total = fs::read_to_string("/sys/kernel/debug/dri/0/i915_gem_objects")
        .ok()
        .and_then(|s| {
            // Parse total memory from gem objects
            s.lines()
                .find(|l| l.contains("bytes"))
                .and_then(|l| l.split_whitespace().next())
                .and_then(|n| n.parse::<u64>().ok())
        })
        .unwrap_or(0);

    Some(GpuInfo {
        name: "Intel GPU".to_string(),
        vendor: "Intel".to_string(),
        vram_total_bytes: vram_total,
        vram_used_bytes: 0, // Not easily available for Intel
        usage_percent: usage,
        temperature_celsius: None, // Usually not available for Intel iGPUs
        power_watts: None,
        fan_speed_percent: None,
    })
}
