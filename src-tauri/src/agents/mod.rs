//! Agent System
//!
//! Manages AI agents with different specializations and capabilities.
//! Agents are pre-configured AI personalities optimized for specific tasks.
//!
//! Runtime state tracking: each agent tracks active tasks, logs, and message counts.
//! Tasks are executed via the intelligent model router (Ollama / OpenRouter).

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::RwLock;
use once_cell::sync::Lazy;

// ════════════════════════════════════════════════════════════════
// TYPES
// ════════════════════════════════════════════════════════════════

/// Agent role/specialization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AgentRole {
    Orchestrator,
    Coder,
    Debugger,
    Researcher,
    Writer,
    Reviewer,
    Architect,
    Custom(String),
}

impl AgentRole {
    pub fn default_model(&self) -> &'static str {
        match self {
            Self::Orchestrator => "meta-llama/llama-4-scout:free",
            Self::Coder => "mistralai/devstral-small:free",
            Self::Debugger => "mistralai/devstral-small:free",
            Self::Researcher => "meta-llama/llama-4-scout:free",
            Self::Writer => "meta-llama/llama-4-scout:free",
            Self::Reviewer => "mistralai/devstral-small:free",
            Self::Architect => "qwen/qwen3-30b-a3b:free",
            Self::Custom(_) => "meta-llama/llama-4-scout:free",
        }
    }

    pub fn default_system_prompt(&self) -> &'static str {
        match self {
            Self::Orchestrator => "You are an AI orchestrator that coordinates complex tasks by breaking them into subtasks and delegating to specialized agents.",
            Self::Coder => "You are an expert software engineer. Write clean, efficient, well-documented code. Follow best practices and design patterns.",
            Self::Debugger => "You are a debugging specialist. Analyze errors, trace issues, and provide precise fixes with explanations.",
            Self::Researcher => "You are a research analyst. Find relevant information, synthesize findings, and provide comprehensive summaries with sources.",
            Self::Writer => "You are a technical writer. Create clear, well-structured documentation, READMEs, and explanations.",
            Self::Reviewer => "You are a code reviewer. Analyze code for bugs, security issues, performance problems, and suggest improvements.",
            Self::Architect => "You are a software architect. Design scalable systems, evaluate trade-offs, and create comprehensive architecture plans.",
            Self::Custom(_) => "You are a helpful AI assistant.",
        }
    }
}

/// Agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub id: String,
    pub name: String,
    pub role: AgentRole,
    pub model_id: String,
    pub system_prompt: String,
    pub enabled: bool,
    pub temperature: f32,
    pub max_tokens: u32,
}

impl AgentConfig {
    pub fn new(id: &str, name: &str, role: AgentRole) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            model_id: role.default_model().to_string(),
            system_prompt: role.default_system_prompt().to_string(),
            role,
            enabled: true,
            temperature: 0.7,
            max_tokens: 4096,
        }
    }
}

/// Agent execution status (returned to frontend)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatus {
    pub id: String,
    pub state: AgentState,
    pub current_task: Option<String>,
    pub messages_processed: u64,
    pub tasks_completed: u64,
    pub tasks_failed: u64,
    pub last_active: Option<String>,
    pub uptime_seconds: Option<u64>,
}

/// Discrete agent lifecycle state
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AgentState {
    /// Agent exists but is not running
    Idle,
    /// Agent is processing a task
    Running,
    /// Agent encountered an error on its last task
    Error,
    /// Agent is disabled by the user
    Disabled,
}

/// A single log entry for an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLog {
    pub timestamp: String,
    pub level: AgentLogLevel,
    pub message: String,
    /// The task ID this log belongs to, if any
    pub task_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentLogLevel {
    Info,
    Warn,
    Error,
    Debug,
}

// ════════════════════════════════════════════════════════════════
// RUNTIME STATE (per agent)
// ════════════════════════════════════════════════════════════════

/// Internal mutable runtime state for each agent.
/// Separated from AgentConfig so config remains pure/serializable.
struct AgentRuntime {
    state: AgentState,
    current_task: Option<String>,
    current_task_id: Option<String>,
    messages_processed: u64,
    tasks_completed: u64,
    tasks_failed: u64,
    last_active: Option<String>,
    started_at: Option<chrono::DateTime<Utc>>,
    /// Ring buffer of recent logs (max 500 per agent)
    logs: VecDeque<AgentLog>,
}

impl AgentRuntime {
    fn new() -> Self {
        Self {
            state: AgentState::Idle,
            current_task: None,
            current_task_id: None,
            messages_processed: 0,
            tasks_completed: 0,
            tasks_failed: 0,
            last_active: None,
            started_at: None,
            logs: VecDeque::with_capacity(500),
        }
    }

    fn push_log(&mut self, level: AgentLogLevel, message: String, task_id: Option<String>) {
        if self.logs.len() >= 500 {
            self.logs.pop_front();
        }
        self.logs.push_back(AgentLog {
            timestamp: Utc::now().to_rfc3339(),
            level,
            message,
            task_id,
        });
    }

    fn to_status(&self, id: &str) -> AgentStatus {
        let uptime = self.started_at.map(|s| {
            let elapsed = Utc::now().signed_duration_since(s);
            elapsed.num_seconds().max(0) as u64
        });
        AgentStatus {
            id: id.to_string(),
            state: self.state.clone(),
            current_task: self.current_task.clone(),
            messages_processed: self.messages_processed,
            tasks_completed: self.tasks_completed,
            tasks_failed: self.tasks_failed,
            last_active: self.last_active.clone(),
            uptime_seconds: uptime,
        }
    }
}

const MAX_LOG_ENTRIES: usize = 500;

// ════════════════════════════════════════════════════════════════
// GLOBAL REGISTRIES
// ════════════════════════════════════════════════════════════════

/// Global agent config registry
static AGENTS: Lazy<RwLock<HashMap<String, AgentConfig>>> = Lazy::new(|| {
    let mut agents = HashMap::new();

    let defaults = vec![
        AgentConfig::new("orchestrator", "Orchestrator", AgentRole::Orchestrator),
        AgentConfig::new("coder", "Code Expert", AgentRole::Coder),
        AgentConfig::new("debugger", "Debug Specialist", AgentRole::Debugger),
        AgentConfig::new("researcher", "Research Analyst", AgentRole::Researcher),
        AgentConfig::new("writer", "Tech Writer", AgentRole::Writer),
        AgentConfig::new("reviewer", "Code Reviewer", AgentRole::Reviewer),
        AgentConfig::new("architect", "System Architect", AgentRole::Architect),
    ];

    for agent in defaults {
        agents.insert(agent.id.clone(), agent);
    }

    RwLock::new(agents)
});

/// Global agent runtime state registry
static RUNTIMES: Lazy<RwLock<HashMap<String, AgentRuntime>>> = Lazy::new(|| {
    // Pre-populate with defaults matching AGENTS
    let ids = [
        "orchestrator", "coder", "debugger", "researcher", "writer", "reviewer", "architect",
    ];
    let mut runtimes = HashMap::new();
    for id in ids {
        runtimes.insert(id.to_string(), AgentRuntime::new());
    }
    RwLock::new(runtimes)
});

/// Ensure a runtime entry exists for a given agent ID
fn ensure_runtime(id: &str) {
    let needs_insert = {
        let runtimes = RUNTIMES.read().unwrap_or_else(|e| e.into_inner());
        !runtimes.contains_key(id)
    };
    if needs_insert {
        let mut runtimes = RUNTIMES.write().unwrap_or_else(|e| e.into_inner());
        runtimes.entry(id.to_string()).or_insert_with(AgentRuntime::new);
    }
}

// ════════════════════════════════════════════════════════════════
// TAURI COMMANDS — CRUD (existing, preserved)
// ════════════════════════════════════════════════════════════════

/// Get all agents
#[tauri::command]
pub fn list_agents() -> Result<Vec<AgentConfig>, String> {
    let agents = AGENTS.read()
        .map_err(|e| format!("Failed to read agents: {}", e))?;

    Ok(agents.values().cloned().collect())
}

/// Get a specific agent
#[tauri::command]
pub fn get_agent(id: String) -> Result<Option<AgentConfig>, String> {
    let agents = AGENTS.read()
        .map_err(|e| format!("Failed to read agents: {}", e))?;

    Ok(agents.get(&id).cloned())
}

/// Create a new agent
#[tauri::command]
pub fn create_agent(
    id: String,
    name: String,
    role: AgentRole,
    system_prompt: Option<String>,
    model_id: Option<String>,
) -> Result<AgentConfig, String> {
    let mut agents = AGENTS.write()
        .map_err(|e| format!("Failed to write agents: {}", e))?;

    if agents.contains_key(&id) {
        return Err(format!("Agent with ID '{}' already exists", id));
    }

    let mut agent = AgentConfig::new(&id, &name, role);

    if let Some(prompt) = system_prompt {
        agent.system_prompt = prompt;
    }
    if let Some(model) = model_id {
        agent.model_id = model;
    }

    agents.insert(id.clone(), agent.clone());

    // Create runtime entry
    ensure_runtime(&id);
    if let Ok(mut runtimes) = RUNTIMES.write() {
        if let Some(rt) = runtimes.get_mut(&id) {
            rt.push_log(AgentLogLevel::Info, format!("Agent '{}' created", agent.name), None);
        }
    }

    log::info!("Created agent: {}", agent.name);
    Ok(agent)
}

/// Update an existing agent
#[tauri::command]
pub fn update_agent(
    id: String,
    name: Option<String>,
    system_prompt: Option<String>,
    model_id: Option<String>,
    enabled: Option<bool>,
    temperature: Option<f32>,
) -> Result<AgentConfig, String> {
    let mut agents = AGENTS.write()
        .map_err(|e| format!("Failed to write agents: {}", e))?;

    let agent = agents.get_mut(&id)
        .ok_or_else(|| format!("Agent '{}' not found", id))?;

    if let Some(n) = name {
        agent.name = n;
    }
    if let Some(p) = system_prompt {
        agent.system_prompt = p;
    }
    if let Some(m) = model_id {
        agent.model_id = m;
    }
    if let Some(e) = enabled {
        agent.enabled = e;
    }
    if let Some(t) = temperature {
        agent.temperature = t.clamp(0.0, 2.0);
    }

    // Update runtime state if disabled
    ensure_runtime(&id);
    if let Ok(mut runtimes) = RUNTIMES.write() {
        if let Some(rt) = runtimes.get_mut(&id) {
            if agent.enabled {
                if rt.state == AgentState::Disabled {
                    rt.state = AgentState::Idle;
                }
            } else {
                rt.state = AgentState::Disabled;
                rt.current_task = None;
                rt.current_task_id = None;
            }
            rt.push_log(AgentLogLevel::Info, format!("Agent '{}' updated", agent.name), None);
        }
    }

    log::info!("Updated agent: {}", agent.name);
    Ok(agent.clone())
}

/// Delete an agent
#[tauri::command]
pub fn delete_agent(id: String) -> Result<bool, String> {
    let mut agents = AGENTS.write()
        .map_err(|e| format!("Failed to write agents: {}", e))?;

    let removed = agents.remove(&id).is_some();

    if removed {
        // Clean up runtime
        if let Ok(mut runtimes) = RUNTIMES.write() {
            runtimes.remove(&id);
        }
        log::info!("Deleted agent: {}", id);
    }

    Ok(removed)
}

/// Get agent for a specific role
#[tauri::command]
pub fn get_agent_by_role(role: AgentRole) -> Result<Option<AgentConfig>, String> {
    let agents = AGENTS.read()
        .map_err(|e| format!("Failed to read agents: {}", e))?;

    Ok(agents.values().find(|a| a.role == role && a.enabled).cloned())
}

// ════════════════════════════════════════════════════════════════
// TAURI COMMANDS — RUNTIME (new)
// ════════════════════════════════════════════════════════════════

/// Get runtime status of all agents (merged config + runtime)
#[tauri::command]
pub fn get_agent_statuses() -> Result<Vec<AgentStatus>, String> {
    let agents = AGENTS.read()
        .map_err(|e| format!("Failed to read agents: {}", e))?;
    let runtimes = RUNTIMES.read()
        .map_err(|e| format!("Failed to read runtimes: {}", e))?;

    Ok(agents.values().map(|a| {
        if let Some(rt) = runtimes.get(&a.id) {
            let mut status = rt.to_status(&a.id);
            // Override state for disabled agents
            if !a.enabled {
                status.state = AgentState::Disabled;
            }
            status
        } else {
            AgentStatus {
                id: a.id.clone(),
                state: if a.enabled { AgentState::Idle } else { AgentState::Disabled },
                current_task: None,
                messages_processed: 0,
                tasks_completed: 0,
                tasks_failed: 0,
                last_active: None,
                uptime_seconds: None,
            }
        }
    }).collect())
}

/// Get runtime status for a single agent
#[tauri::command]
pub fn agent_status(agent_id: String) -> Result<AgentStatus, String> {
    let agents = AGENTS.read()
        .map_err(|e| format!("Failed to read agents: {}", e))?;
    let runtimes = RUNTIMES.read()
        .map_err(|e| format!("Failed to read runtimes: {}", e))?;

    let agent = agents.get(&agent_id)
        .ok_or_else(|| format!("Agent '{}' not found", agent_id))?;

    if let Some(rt) = runtimes.get(&agent_id) {
        let mut status = rt.to_status(&agent_id);
        if !agent.enabled {
            status.state = AgentState::Disabled;
        }
        Ok(status)
    } else {
        Ok(AgentStatus {
            id: agent_id,
            state: if agent.enabled { AgentState::Idle } else { AgentState::Disabled },
            current_task: None,
            messages_processed: 0,
            tasks_completed: 0,
            tasks_failed: 0,
            last_active: None,
            uptime_seconds: None,
        })
    }
}

/// Run a task using a specific agent.
///
/// This executes the task via the intelligent model router, using the agent's
/// configured model and system prompt. The agent's runtime state is tracked
/// throughout execution.
///
/// Returns the LLM response text.
#[tauri::command]
pub async fn run_agent(agent_id: String, task: String) -> Result<String, String> {
    // 1. Validate agent exists and is enabled
    let (system_prompt, _model_id, agent_name) = {
        let agents = AGENTS.read()
            .map_err(|e| format!("Failed to read agents: {}", e))?;
        let agent = agents.get(&agent_id)
            .ok_or_else(|| format!("Agent '{}' not found", agent_id))?;
        if !agent.enabled {
            return Err(format!("Agent '{}' is disabled", agent_id));
        }
        (agent.system_prompt.clone(), agent.model_id.clone(), agent.name.clone())
    };

    let task_id = format!("task-{}", Utc::now().timestamp_millis());

    // 2. Set runtime to Running
    {
        ensure_runtime(&agent_id);
        let mut runtimes = RUNTIMES.write()
            .map_err(|e| format!("Failed to write runtimes: {}", e))?;
        if let Some(rt) = runtimes.get_mut(&agent_id) {
            rt.state = AgentState::Running;
            rt.current_task = Some(task.chars().take(120).collect());
            rt.current_task_id = Some(task_id.clone());
            rt.started_at = Some(Utc::now());
            rt.push_log(
                AgentLogLevel::Info,
                format!("Starting task: {}", task.chars().take(80).collect::<String>()),
                Some(task_id.clone()),
            );
        }
    }

    log::info!("Agent '{}' running task: {}", agent_name, task.chars().take(80).collect::<String>());

    // 3. Execute via router
    let config = crate::build_router_config();
    let result = crate::router::route_and_execute(&task, Some(&system_prompt), &config).await;

    // 4. Update runtime based on result
    {
        let mut runtimes = RUNTIMES.write()
            .map_err(|e| format!("Failed to write runtimes: {}", e))?;
        if let Some(rt) = runtimes.get_mut(&agent_id) {
            rt.messages_processed += 1;
            rt.last_active = Some(Utc::now().to_rfc3339());
            rt.current_task = None;
            rt.current_task_id = None;

            match &result {
                Ok(response) => {
                    rt.state = AgentState::Idle;
                    rt.tasks_completed += 1;
                    rt.push_log(
                        AgentLogLevel::Info,
                        format!(
                            "Task completed ({} chars response)",
                            response.len()
                        ),
                        Some(task_id.clone()),
                    );
                }
                Err(e) => {
                    rt.state = AgentState::Error;
                    rt.tasks_failed += 1;
                    rt.push_log(
                        AgentLogLevel::Error,
                        format!("Task failed: {}", e),
                        Some(task_id),
                    );
                }
            }
        }
    }

    result.map_err(|e| e.to_string())
}

/// Stop an agent's current task (sets state back to idle).
///
/// Note: This does not cancel in-flight HTTP requests (the router does not
/// support cancellation tokens yet). It resets the agent's state so the UI
/// reflects it as idle.
#[tauri::command]
pub fn stop_agent(agent_id: String) -> Result<(), String> {
    let agents = AGENTS.read()
        .map_err(|e| format!("Failed to read agents: {}", e))?;

    if !agents.contains_key(&agent_id) {
        return Err(format!("Agent '{}' not found", agent_id));
    }

    ensure_runtime(&agent_id);
    let mut runtimes = RUNTIMES.write()
        .map_err(|e| format!("Failed to write runtimes: {}", e))?;

    if let Some(rt) = runtimes.get_mut(&agent_id) {
        let was_running = rt.state == AgentState::Running;
        rt.state = AgentState::Idle;
        rt.current_task = None;
        rt.current_task_id = None;
        if was_running {
            rt.push_log(AgentLogLevel::Warn, "Agent stopped by user".to_string(), None);
        }
    }

    log::info!("Agent '{}' stopped", agent_id);
    Ok(())
}

/// Get recent logs for an agent.
///
/// Returns up to `limit` log entries (default 100, max 500).
/// Newest entries first.
#[tauri::command]
pub fn agent_logs(agent_id: String, limit: Option<u32>) -> Result<Vec<AgentLog>, String> {
    let agents = AGENTS.read()
        .map_err(|e| format!("Failed to read agents: {}", e))?;

    if !agents.contains_key(&agent_id) {
        return Err(format!("Agent '{}' not found", agent_id));
    }

    let runtimes = RUNTIMES.read()
        .map_err(|e| format!("Failed to read runtimes: {}", e))?;

    let max = (limit.unwrap_or(100) as usize).min(MAX_LOG_ENTRIES);

    if let Some(rt) = runtimes.get(&agent_id) {
        // Return newest first
        let logs: Vec<AgentLog> = rt.logs.iter().rev().take(max).cloned().collect();
        Ok(logs)
    } else {
        Ok(vec![])
    }
}
