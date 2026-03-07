//! Agent System
//!
//! Manages AI agents with different specializations and capabilities.
//! Agents are pre-configured AI personalities optimized for specific tasks.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use once_cell::sync::Lazy;

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

/// Agent execution status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatus {
    pub id: String,
    pub active: bool,
    pub current_task: Option<String>,
    pub messages_processed: u64,
    pub last_active: Option<String>,
}

/// Global agent registry
static AGENTS: Lazy<RwLock<HashMap<String, AgentConfig>>> = Lazy::new(|| {
    let mut agents = HashMap::new();

    // Pre-configured default agents
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

    agents.insert(id, agent.clone());
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
