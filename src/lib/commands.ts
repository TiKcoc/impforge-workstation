/**
 * ImpForge Tauri Commands
 * Type-safe wrappers for all Rust backend commands
 */

import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// Types
// ============================================================================

/** Container information from Docker */
export interface ContainerInfo {
	id: string;
	name: string;
	image: string;
	status: string;
	state: string;
	ports: string[];
}

/** Container action types */
export type ContainerAction = 'Start' | 'Stop' | 'Restart' | 'Remove' | 'Logs';

/** GitHub repository info */
export interface RepoInfo {
	id: number;
	name: string;
	full_name: string;
	description: string | null;
	html_url: string;
	default_branch: string;
	stargazers_count: number;
	open_issues_count: number;
	is_private: boolean;
	language: string | null;
	updated_at: string;
}

/** GitHub issue info */
export interface IssueInfo {
	id: number;
	number: number;
	title: string;
	state: string;
	html_url: string;
	created_at: string;
	labels: LabelInfo[];
	user: UserInfo;
	body: string | null;
}

/** GitHub label */
export interface LabelInfo {
	name: string;
	color: string;
}

/** GitHub user */
export interface UserInfo {
	login: string;
	avatar_url: string;
}

/** Pull request info */
export interface PullRequestInfo {
	id: number;
	number: number;
	title: string;
	state: string;
	html_url: string;
	created_at: string;
	user: UserInfo;
	head: BranchRef;
	base: BranchRef;
	merged: boolean;
	draft: boolean;
}

/** Branch reference */
export interface BranchRef {
	branch_ref: string;
	sha: string;
}

/** Agent role types */
export type AgentRole =
	| 'orchestrator'
	| 'coder'
	| 'debugger'
	| 'researcher'
	| 'writer'
	| 'reviewer'
	| 'architect'
	| { Custom: string };

/** Agent configuration */
export interface AgentConfig {
	id: string;
	name: string;
	role: AgentRole;
	model_id: string;
	system_prompt: string;
	enabled: boolean;
	temperature: number;
	max_tokens: number;
}

/** Message for routing */
export interface RoutedMessage {
	content: string;
	model_id?: string;
	conversation_id?: string;
}

// ============================================================================
// Router Commands
// ============================================================================

/**
 * Route a message through the intelligent router
 */
export async function routeMessage(message: RoutedMessage): Promise<string> {
	return invoke<string>('route_message', { message });
}

/**
 * Get routing preview (task type and target model)
 */
export async function getRoutingPreview(
	prompt: string
): Promise<[string, string]> {
	return invoke<[string, string]>('get_routing_preview', { prompt });
}

/**
 * Get list of available models
 */
export async function getAvailableModels(): Promise<
	Array<{
		id: string;
		name: string;
		provider: string;
		free: boolean;
	}>
> {
	return invoke('get_available_models');
}

// ============================================================================
// Docker Commands
// ============================================================================

/**
 * List all Docker containers
 */
export async function listContainers(): Promise<ContainerInfo[]> {
	return invoke<ContainerInfo[]>('list_containers');
}

/**
 * Perform action on a container
 */
export async function containerAction(
	containerId: string,
	action: ContainerAction
): Promise<string> {
	return invoke<string>('container_action', {
		container_id: containerId,
		action,
	});
}

/**
 * Get Docker system info
 */
export async function dockerInfo(): Promise<Record<string, string>> {
	return invoke<Record<string, string>>('docker_info');
}

// ============================================================================
// GitHub Commands
// ============================================================================

/**
 * Get user's repositories
 */
export async function getRepos(): Promise<RepoInfo[]> {
	return invoke<RepoInfo[]>('get_repos');
}

/**
 * Get issues for a repository
 */
export async function getIssues(repo: string): Promise<IssueInfo[]> {
	return invoke<IssueInfo[]>('get_issues', { repo });
}

/**
 * Get pull requests for a repository
 */
export async function getPullRequests(repo: string): Promise<PullRequestInfo[]> {
	return invoke<PullRequestInfo[]>('get_pull_requests', { repo });
}

/**
 * Get authenticated user info
 */
export async function getUser(): Promise<UserInfo> {
	return invoke<UserInfo>('get_user');
}

// ============================================================================
// Agent Commands
// ============================================================================

/**
 * List all agents
 */
export async function listAgents(): Promise<AgentConfig[]> {
	return invoke<AgentConfig[]>('list_agents');
}

/**
 * Get a specific agent
 */
export async function getAgent(id: string): Promise<AgentConfig | null> {
	return invoke<AgentConfig | null>('get_agent', { id });
}

/**
 * Create a new agent
 */
export async function createAgent(params: {
	id: string;
	name: string;
	role: AgentRole;
	system_prompt?: string;
	model_id?: string;
}): Promise<AgentConfig> {
	return invoke<AgentConfig>('create_agent', params);
}

/**
 * Update an existing agent
 */
export async function updateAgent(params: {
	id: string;
	name?: string;
	system_prompt?: string;
	model_id?: string;
	enabled?: boolean;
	temperature?: number;
}): Promise<AgentConfig> {
	return invoke<AgentConfig>('update_agent', params);
}

/**
 * Delete an agent
 */
export async function deleteAgent(id: string): Promise<boolean> {
	return invoke<boolean>('delete_agent', { id });
}

/**
 * Get agent by role
 */
export async function getAgentByRole(
	role: AgentRole
): Promise<AgentConfig | null> {
	return invoke<AgentConfig | null>('get_agent_by_role', { role });
}

// ============================================================================
// System Monitoring Types
// ============================================================================

/** CPU information */
export interface CpuInfo {
	name: string;
	physical_cores: number;
	logical_cores: number;
	frequency_mhz: number;
	usage_percent: number;
	core_usage: number[];
	temperature_celsius: number | null;
	power_watts: number | null;
}

/** Memory information */
export interface MemoryInfo {
	total_bytes: number;
	used_bytes: number;
	available_bytes: number;
	usage_percent: number;
	swap_total_bytes: number;
	swap_used_bytes: number;
}

/** GPU information (AMD, NVIDIA, Intel) */
export interface GpuInfo {
	name: string;
	vendor: 'AMD' | 'NVIDIA' | 'Intel' | string;
	vram_total_bytes: number;
	vram_used_bytes: number;
	usage_percent: number | null;
	temperature_celsius: number | null;
	power_watts: number | null;
	fan_speed_percent: number | null;
}

/** Disk information */
export interface DiskInfo {
	name: string;
	mount_point: string;
	total_bytes: number;
	available_bytes: number;
	usage_percent: number;
	file_system: string;
}

/** Network interface information */
export interface NetworkInfo {
	name: string;
	received_bytes: number;
	transmitted_bytes: number;
}

/** Complete system status */
export interface SystemStatus {
	cpu: CpuInfo;
	memory: MemoryInfo;
	gpus: GpuInfo[];
	disks: DiskInfo[];
	networks: NetworkInfo[];
	uptime_seconds: number;
}

/** Resource sample for history tracking */
export interface ResourceSample {
	timestamp_ms: number;
	cpu_percent: number;
	memory_percent: number;
	gpu_percent: number | null;
	vram_percent: number | null;
	cpu_temp: number | null;
	gpu_temp: number | null;
	cpu_power: number | null;
	gpu_power: number | null;
}

/** Per-process resource usage */
export interface ProcessUsage {
	pid: number;
	name: string;
	cmd: string;
	cpu_percent: number;
	memory_bytes: number;
	memory_percent: number;
	gpu_percent: number | null;
	vram_bytes: number | null;
	runtime_seconds: number;
	cpu_time_seconds: number;
}

/** Duration statistics for high resource usage */
export interface UsageDuration {
	cpu_high_duration_secs: number;
	memory_high_duration_secs: number;
	gpu_high_duration_secs: number;
	vram_high_duration_secs: number;
	tracking_started_ms: number;
	tracking_duration_secs: number;
}

/** Top resource consumers */
export interface TopConsumers {
	cpu: ProcessUsage[];
	memory: ProcessUsage[];
	gpu: ProcessUsage[];
}

/** Historical usage data */
export interface UsageHistory {
	samples: ResourceSample[];
	duration: UsageDuration;
	top_consumers: TopConsumers;
}

// ============================================================================
// System Monitoring Commands
// ============================================================================

/** Get CPU information */
export async function getCpuInfo(): Promise<CpuInfo> {
	return invoke<CpuInfo>('cmd_get_cpu_info');
}

/** Get memory information */
export async function getMemoryInfo(): Promise<MemoryInfo> {
	return invoke<MemoryInfo>('cmd_get_memory_info');
}

/** Get GPU information (AMD, NVIDIA, Intel) */
export async function getGpuInfo(): Promise<GpuInfo[]> {
	return invoke<GpuInfo[]>('cmd_get_gpu_info');
}

/** Get disk information */
export async function getDiskInfo(): Promise<DiskInfo[]> {
	return invoke<DiskInfo[]>('cmd_get_disk_info');
}

/** Get network information */
export async function getNetworkInfo(): Promise<NetworkInfo[]> {
	return invoke<NetworkInfo[]>('cmd_get_network_info');
}

/** Get complete system status */
export async function getSystemStatus(): Promise<SystemStatus> {
	return invoke<SystemStatus>('cmd_get_system_status');
}

// ============================================================================
// Historical Tracking Commands
// ============================================================================

/** Record a sample to history (call this periodically) */
export async function recordSample(): Promise<void> {
	return invoke<void>('cmd_record_sample');
}

/**
 * Get usage history with time-series data
 * @param maxSamples Maximum number of samples to return (default: all)
 */
export async function getUsageHistory(maxSamples?: number): Promise<UsageHistory> {
	return invoke<UsageHistory>('cmd_get_usage_history', { max_samples: maxSamples });
}

/** Get usage duration statistics (how long resources were stressed) */
export async function getUsageDuration(): Promise<UsageDuration> {
	return invoke<UsageDuration>('cmd_get_usage_duration');
}

/**
 * Get top resource consumers (which processes use most CPU/RAM/GPU)
 * @param limit Number of top processes to return (default: 10)
 */
export async function getTopProcesses(limit?: number): Promise<TopConsumers> {
	return invoke<TopConsumers>('cmd_get_top_processes', { limit });
}

/** Reset history tracking (clears all samples and resets duration counters) */
export async function resetHistory(): Promise<void> {
	return invoke<void>('cmd_reset_history');
}

/**
 * Start background monitoring (records samples at regular intervals)
 * @param intervalMs Sampling interval in milliseconds (min: 100ms)
 */
export async function startMonitoring(intervalMs: number): Promise<void> {
	return invoke<void>('cmd_start_monitoring', { interval_ms: intervalMs });
}

// ============================================================================
// Utility Functions for Frontend
// ============================================================================

/** Format bytes to human readable string */
export function formatBytes(bytes: number): string {
	const units = ['B', 'KB', 'MB', 'GB', 'TB'];
	let unitIndex = 0;
	let value = bytes;

	while (value >= 1024 && unitIndex < units.length - 1) {
		value /= 1024;
		unitIndex++;
	}

	return `${value.toFixed(1)} ${units[unitIndex]}`;
}

/** Format duration in seconds to human readable string */
export function formatDuration(seconds: number): string {
	if (seconds < 60) return `${seconds}s`;
	if (seconds < 3600) return `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
	const hours = Math.floor(seconds / 3600);
	const mins = Math.floor((seconds % 3600) / 60);
	return `${hours}h ${mins}m`;
}

/** Format percentage with color hint */
export function getUsageColor(percent: number): 'green' | 'yellow' | 'red' {
	if (percent < 50) return 'green';
	if (percent < 80) return 'yellow';
	return 'red';
}

// ============================================================================
// AI Model Tracking Types
// ============================================================================

/** AI model resource usage information */
export interface ModelUsage {
	model_name: string;
	/** Provider: Ollama, llama.cpp, vLLM, HuggingFace, TGI, Candle, ExLlamaV2 */
	provider: 'Ollama' | 'llama.cpp' | 'vLLM' | 'HuggingFace' | 'TGI' | 'Candle' | 'ExLlamaV2' | string;
	pid: number | null;
	vram_bytes: number;
	ram_bytes: number;
	gpu_percent: number | null;
	cpu_percent: number;
	running_seconds: number;
	tokens_processed: number | null;
	tokens_per_second: number | null;
	model_size_bytes: number;
	/** Quantization: Q4_K_M, Q5_K_M, BNB-4bit, BNB-8bit, GPTQ, AWQ, EXL2, etc. */
	quantization: string | null;
	context_size: number | null;
}

/** Model provider information */
export interface ModelProvider {
	name: string;
	description: string;
	supports_gpu: boolean;
	supports_quantization: boolean;
	api_endpoint?: string;
}

/** Available model providers */
export const MODEL_PROVIDERS: ModelProvider[] = [
	{ name: 'Ollama', description: 'Local LLM server with easy model management', supports_gpu: true, supports_quantization: true, api_endpoint: 'http://localhost:11434' },
	{ name: 'llama.cpp-CPU', description: 'CPU-first inference engine (pure CPU mode)', supports_gpu: false, supports_quantization: true },
	{ name: 'llama.cpp-Hybrid', description: 'CPU+GPU hybrid inference (n_gpu_layers > 0)', supports_gpu: true, supports_quantization: true },
	{ name: 'vLLM', description: 'High-throughput GPU-based LLM serving', supports_gpu: true, supports_quantization: true },
	{ name: 'HuggingFace', description: 'Transformers library models', supports_gpu: true, supports_quantization: true },
	{ name: 'TGI', description: 'Text Generation Inference server', supports_gpu: true, supports_quantization: true, api_endpoint: 'http://localhost:8080' },
	{ name: 'Candle', description: 'Rust ML framework by HuggingFace', supports_gpu: true, supports_quantization: true },
	{ name: 'ExLlamaV2', description: 'Optimized GPU inference for EXL2 models', supports_gpu: true, supports_quantization: true },
];

/** llama.cpp specific configuration */
export interface LlamaCppConfig {
	model_path: string;
	n_gpu_layers: number; // 0 = CPU only, -1 = all on GPU
	threads: number;
	batch_size: number;
	context_size: number;
	mmap: boolean; // Memory-mapped for disk-backed inference
	mlock: boolean; // Lock model in RAM
}

// ============================================================================
// AI Model Recommendation Types (Enterprise-Grade)
// ============================================================================

/** Model category/use case */
export type ModelCategory = 'Chat' | 'Coding' | 'Reasoning' | 'Creative' | 'Embedding' | 'Vision' | 'Fast';

/** Model size tier */
export type ModelTier = 'Tiny' | 'Small' | 'Medium' | 'Large' | 'XLarge';

/** Quantization recommendation */
export interface QuantizationRecommendation {
	format: string;
	vram_gb: number;
	ram_gb: number;
	quality_score: number;
	speed_multiplier: number;
}

/** Recommended model with full details */
export interface RecommendedModel {
	name: string;
	model_id: string;
	gguf_filename: string | null;
	ollama_name: string | null;
	category: ModelCategory;
	tier: ModelTier;
	params_b: number;
	context_size: number;
	quality_score: number;
	speed_score: number;
	quantizations: QuantizationRecommendation[];
	recommendation_reason: string;
	benchmarks: Record<string, number>;
}

/** Complete model recommendation with settings */
export interface ModelRecommendation {
	model: RecommendedModel;
	best_quantization: QuantizationRecommendation;
	expected_tokens_per_sec: number;
	fits_in_vram: boolean;
	fits_in_ram: boolean;
	recommended_gpu_layers: number;
	recommended_threads: number;
	recommended_batch_size: number;
	recommended_context: number;
	llama_cpp_args: string;
	ollama_command: string | null;
}

// ============================================================================
// AI Model Recommendation Commands
// ============================================================================

/**
 * Get AI model recommendations based on hardware
 * @param vramGb Available VRAM in GB
 * @param ramGb Available RAM in GB
 * @param cpuCores Number of CPU cores
 * @param gpuVendor GPU vendor (AMD, NVIDIA, Intel)
 * @param category Optional category filter (Coding, Chat, etc.)
 */
export async function getModelRecommendations(
	vramGb: number,
	ramGb: number,
	cpuCores: number,
	gpuVendor: string,
	category?: string
): Promise<ModelRecommendation[]> {
	return invoke<ModelRecommendation[]>('cmd_get_model_recommendations', {
		vram_gb: vramGb,
		ram_gb: ramGb,
		cpu_cores: cpuCores,
		gpu_vendor: gpuVendor,
		category
	});
}

/**
 * Get all models in the curated database
 */
export async function getModelDatabase(): Promise<RecommendedModel[]> {
	return invoke<RecommendedModel[]>('cmd_get_model_database');
}

/**
 * Get recommended model for a specific task
 * @param task Task description (e.g., "write code", "analyze image", "quick chat")
 */
export async function recommendForTask(
	vramGb: number,
	ramGb: number,
	cpuCores: number,
	gpuVendor: string,
	task: string
): Promise<ModelRecommendation | null> {
	return invoke<ModelRecommendation | null>('cmd_recommend_for_task', {
		vram_gb: vramGb,
		ram_gb: ramGb,
		cpu_cores: cpuCores,
		gpu_vendor: gpuVendor,
		task
	});
}

// ============================================================================
// Hardware-Specific Optimization Types
// ============================================================================

/** GPU-specific optimization settings */
export interface GpuOptimization {
	vendor: 'NVIDIA' | 'AMD' | 'Intel' | string;
	llama_cpp_backend: 'cuda' | 'hip' | 'sycl' | string;
	env_vars: [string, string][];
	supports_flash_attention: boolean;
	has_tensor_cores: boolean;
	optimal_batch_size: number;
	memory_bandwidth_gbps: number | null;
	build_flags: string;
	optimization_tips: string[];
}

/** CPU-specific optimization settings */
export interface CpuOptimization {
	architecture: string;
	optimal_threads: number;
	thread_affinity: string;
	numa_enabled: boolean;
	optimal_batch_size: number;
	use_mmap: boolean;
	use_mlock: boolean;
	build_flags: string;
	optimization_tips: string[];
}

/** Combined hardware optimization */
export interface HardwareOptimization {
	gpu: GpuOptimization | null;
	cpu: CpuOptimization;
	inference_mode: 'GPU' | 'CPU' | 'Hybrid';
	llama_cpp_command: string;
	ollama_config: string;
	expected_performance: string;
}

// ============================================================================
// Hardware Optimization Commands
// ============================================================================

/**
 * Get hardware-specific optimization settings
 * Detects NVIDIA, AMD, Intel GPUs and optimizes for each
 */
export async function getHardwareOptimization(
	vramGb: number,
	ramGb: number,
	cpuCores: number,
	gpuVendor: string,
	gpuName?: string,
	cpuName?: string
): Promise<HardwareOptimization> {
	return invoke<HardwareOptimization>('cmd_get_hardware_optimization', {
		vram_gb: vramGb,
		ram_gb: ramGb,
		cpu_cores: cpuCores,
		gpu_vendor: gpuVendor,
		gpu_name: gpuName,
		cpu_name: cpuName
	});
}

/**
 * Get NVIDIA-specific optimization
 */
export async function getNvidiaOptimization(gpuName: string, vramGb: number): Promise<GpuOptimization> {
	return invoke<GpuOptimization>('cmd_get_nvidia_optimization', {
		gpu_name: gpuName,
		vram_gb: vramGb
	});
}

/**
 * Get AMD-specific optimization (ROCm)
 */
export async function getAmdOptimization(gpuName: string, vramGb: number): Promise<GpuOptimization> {
	return invoke<GpuOptimization>('cmd_get_amd_optimization', {
		gpu_name: gpuName,
		vram_gb: vramGb
	});
}

/**
 * Get CPU-specific optimization for llama.cpp
 */
export async function getCpuOptimization(cpuCores: number, ramGb: number, cpuName?: string): Promise<CpuOptimization> {
	return invoke<CpuOptimization>('cmd_get_cpu_optimization', {
		cpu_cores: cpuCores,
		ram_gb: ramGb,
		cpu_name: cpuName
	});
}

/**
 * Auto-detect hardware and get optimal settings
 * Uses system monitoring to detect GPU/CPU/RAM
 */
export async function autoOptimize(): Promise<HardwareOptimization> {
	return invoke<HardwareOptimization>('cmd_auto_optimize');
}

/**
 * Get model recommendations with hardware optimization
 * Returns both model recommendations and hardware-specific settings
 */
export async function getModelRecommendationsExtended(
	vramGb: number,
	ramGb: number,
	cpuCores: number,
	gpuVendor: string,
	gpuName?: string,
	cpuName?: string,
	category?: string
): Promise<[ModelRecommendation[], HardwareOptimization]> {
	return invoke<[ModelRecommendation[], HardwareOptimization]>('cmd_get_model_recommendations_extended', {
		vram_gb: vramGb,
		ram_gb: ramGb,
		cpu_cores: cpuCores,
		gpu_vendor: gpuVendor,
		gpu_name: gpuName,
		cpu_name: cpuName,
		category
	});
}

/** Recommended llama.cpp settings based on hardware */
export function getRecommendedLlamaCppConfig(
	ramGb: number,
	vramGb: number,
	cpuCores: number,
	modelSizeGb: number
): LlamaCppConfig {
	// CPU-only if no VRAM or model fits in RAM
	const useGpu = vramGb > 4 && modelSizeGb > ramGb * 0.5;

	// Calculate GPU layers (rough: 1 layer ≈ 0.2-0.5GB for 7B model)
	const layersPerGb = 2; // Conservative estimate
	const maxGpuLayers = useGpu ? Math.floor(vramGb * layersPerGb) : 0;

	// Threads: physical cores - 2 for system
	const threads = Math.max(1, cpuCores - 2);

	// Batch size: larger = faster but more RAM
	const batchSize = ramGb > 32 ? 2048 : ramGb > 16 ? 1024 : 512;

	// Context: balance between capability and RAM
	const contextSize = ramGb > 32 ? 8192 : ramGb > 16 ? 4096 : 2048;

	return {
		model_path: '',
		n_gpu_layers: maxGpuLayers,
		threads,
		batch_size: batchSize,
		context_size: contextSize,
		mmap: modelSizeGb > ramGb * 0.7, // Use mmap if model is large
		mlock: modelSizeGb < ramGb * 0.5, // Lock in RAM if it fits
	};
}

/** Optimization suggestion */
export interface OptimizationSuggestion {
	category: 'GPU' | 'VRAM' | 'RAM' | 'Model' | 'CPU' | string;
	title: string;
	description: string;
	potential_savings: string;
	priority: number; // 1-5, 5 being highest
}

// ============================================================================
// AI Model Tracking Commands
// ============================================================================

/**
 * Get all running AI models and their resource usage
 * Returns Ollama, llama.cpp, vLLM models with VRAM/RAM/GPU stats
 */
export async function getModelUsage(): Promise<ModelUsage[]> {
	return invoke<ModelUsage[]>('cmd_get_model_usage');
}

/**
 * Get optimization suggestions for current resource usage
 * Analyzes GPU/VRAM/RAM and provides actionable recommendations
 */
export async function getOptimizationSuggestions(): Promise<OptimizationSuggestion[]> {
	return invoke<OptimizationSuggestion[]>('cmd_get_optimization_suggestions');
}

// ============================================================================
// Extended Utility Functions
// ============================================================================

/** Format VRAM/RAM usage with percentage */
export function formatMemoryUsage(used: number, total: number): string {
	const percent = total > 0 ? (used / total) * 100 : 0;
	return `${formatBytes(used)} / ${formatBytes(total)} (${percent.toFixed(1)}%)`;
}

/** Get priority label */
export function getPriorityLabel(priority: number): string {
	const labels = ['Low', 'Medium', 'High', 'Critical', 'Urgent'];
	return labels[Math.min(priority - 1, labels.length - 1)] || 'Unknown';
}

/** Calculate VRAM percentage */
export function getVramPercent(gpu: GpuInfo): number {
	if (gpu.vram_total_bytes === 0) return 0;
	return (gpu.vram_used_bytes / gpu.vram_total_bytes) * 100;
}

/** Check if model can fit in available VRAM */
export function canFitInVram(modelSizeBytes: number, availableVram: number): boolean {
	// Models need ~20% extra VRAM for context and KV cache
	return modelSizeBytes * 1.2 <= availableVram;
}

/** Estimate VRAM needed for model with context */
export function estimateVramNeeded(
	modelParams: number, // in billions
	quantization: 'FP16' | 'Q8' | 'Q6' | 'Q5' | 'Q4' | 'Q3' | 'Q2',
	contextSize: number
): number {
	const bitsPerParam: Record<string, number> = {
		FP16: 16,
		Q8: 8,
		Q6: 6,
		Q5: 5,
		Q4: 4,
		Q3: 3,
		Q2: 2,
	};

	const bits = bitsPerParam[quantization] || 4;
	const modelVram = (modelParams * 1e9 * bits) / 8;
	// KV cache: ~2MB per 1K context for 7B model, scales with model size
	const kvCache = (contextSize / 1000) * 2 * 1024 * 1024 * (modelParams / 7);

	return modelVram + kvCache;
}
