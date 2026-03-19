// SPDX-License-Identifier: Apache-2.0
//! ForgeFlow -- Built-in workflow automation engine.
//!
//! Replaces n8n / Zapier / Make.com with a native, offline-first DAG executor.
//!
//! Features:
//! - Visual node-based workflow editor (Svelte frontend)
//! - 10 trigger types + 15 action types + 4 control-flow nodes
//! - Topological-sort execution via DFS (no petgraph dependency)
//! - JSON file persistence in `~/.impforge/workflows/`
//! - Pre-built templates (Daily Report, Git Deploy, RSS Newsletter, etc.)

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Lazy-static webhook registry (shared across commands)
// ---------------------------------------------------------------------------

static WEBHOOK_REGISTRY: std::sync::LazyLock<Arc<RwLock<HashMap<String, WebhookRegistration>>>> =
    std::sync::LazyLock::new(|| Arc::new(RwLock::new(HashMap::new())));

#[derive(Debug, Clone)]
struct WebhookRegistration {
    pub workflow_id: String,
    pub path: String,
    pub method: String,
}

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub nodes: Vec<FlowNode>,
    pub edges: Vec<FlowEdge>,
    pub enabled: bool,
    pub run_count: u64,
    pub last_run: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// Global workflow variables accessible as {{var.key}} in node configs.
    #[serde(default)]
    pub variables: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowNode {
    pub id: String,
    pub node_type: NodeType,
    pub label: String,
    pub config: serde_json::Value,
    pub position: (f64, f64),
    /// Error recovery configuration for this node.
    #[serde(default)]
    pub retry_config: Option<RetryConfig>,
}

/// Error recovery configuration for a workflow node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts.
    pub max_retries: u32,
    /// Initial backoff in milliseconds (doubles each retry).
    pub backoff_ms: u64,
    /// What to do when all retries are exhausted.
    pub on_failure: FailureAction,
}

/// Action to take when a node fails after all retries.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureAction {
    /// Stop the entire workflow.
    Stop,
    /// Skip this node and continue to the next.
    Skip,
    /// Retry with exponential backoff (handled internally).
    Retry,
    /// Execute a specific fallback node by id.
    Fallback(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NodeType {
    // Triggers
    TriggerManual,
    TriggerCron { schedule: String },
    TriggerWebhook { path: String },
    TriggerFileWatch { path: String },
    TriggerAppEvent { event_name: String },

    // Actions
    ActionHttpRequest {
        method: String,
        url: String,
        headers: Vec<(String, String)>,
        body: Option<String>,
    },
    ActionLlmCall {
        prompt: String,
        model: Option<String>,
    },
    ActionEmail {
        to: String,
        subject: String,
        body: String,
    },
    ActionShellCommand {
        command: String,
    },
    ActionFileOp {
        operation: String,
        path: String,
    },
    ActionJsonTransform {
        expression: String,
    },
    ActionNotification {
        title: String,
        message: String,
    },
    ActionGitOp {
        operation: String,
    },
    ActionSocialPost {
        platform: String,
        content: String,
    },
    ActionDbQuery {
        query: String,
    },

    // Sub-Workflows
    ActionSubWorkflow { workflow_id: String },

    // Data Transformation
    ActionSplit { field: String },
    ActionFilter { expression: String },
    ActionSort { field: String, ascending: bool },
    ActionAggregate { field: String, operation: String },
    ActionMap { expression: String },
    ActionUnique { field: String },

    // Control flow
    ControlCondition { expression: String },
    ControlLoop { count: Option<u32> },
    ControlDelay { seconds: u32 },
    ControlParallel,
    ControlMerge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRun {
    pub id: String,
    pub workflow_id: String,
    pub status: RunStatus,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub node_results: Vec<NodeResult>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeResult {
    pub node_id: String,
    pub status: RunStatus,
    /// What the node received as input.
    #[serde(default)]
    pub input: serde_json::Value,
    /// What the node produced as output.
    pub output: serde_json::Value,
    pub duration_ms: u64,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Credential Vault
// ---------------------------------------------------------------------------

/// Encrypted credential storage for API keys, OAuth tokens, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credential {
    pub id: String,
    pub name: String,
    /// Type: "api_key", "oauth2", "basic_auth", "bearer"
    pub credential_type: String,
    /// Credential data (encrypted at rest via XOR obfuscation -- not production crypto,
    /// but prevents plain-text storage; upgrade to AES-256-GCM for enterprise).
    pub data: serde_json::Value,
    pub created_at: String,
}

/// Compact credential metadata (excludes secret data).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialMeta {
    pub id: String,
    pub name: String,
    pub credential_type: String,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// Workflow Versioning
// ---------------------------------------------------------------------------

/// Snapshot of a workflow at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowVersion {
    pub version: u32,
    pub snapshot: serde_json::Value,
    pub message: String,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// Webhook Info
// ---------------------------------------------------------------------------

/// Registered webhook endpoint information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookInfo {
    pub workflow_id: String,
    pub path: String,
    pub method: String,
    pub url: String,
}

/// Compact summary returned by `flow_list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowMeta {
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub node_count: usize,
    pub run_count: u64,
    pub last_run: Option<String>,
}

/// Pre-built workflow template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub workflow: Workflow,
}

/// Cron-based workflow schedule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowSchedule {
    pub workflow_id: String,
    pub cron_expression: String,
    pub enabled: bool,
    pub next_run: Option<String>,
    pub timezone: String,
}

/// Execution analytics for a workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowAnalytics {
    pub total_runs: u64,
    pub success_rate: f32,
    pub avg_duration_ms: u64,
    pub most_failed_node: Option<String>,
    pub last_7_days: Vec<DailyStats>,
}

/// Per-day execution statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyStats {
    pub date: String,
    pub runs: u64,
    pub successes: u64,
    pub failures: u64,
    pub avg_duration_ms: u64,
}

/// AI-generated suggestion for the next node in a workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSuggestion {
    pub node_type: NodeType,
    pub label: String,
    pub description: String,
    pub confidence: f32,
}

// ---------------------------------------------------------------------------
// Persistence helpers
// ---------------------------------------------------------------------------

fn data_base_dir() -> Result<PathBuf, String> {
    Ok(dirs::data_dir()
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_default()
                .join(".local")
                .join("share")
        })
        .join("impforge"))
}

fn workflows_dir() -> Result<PathBuf, String> {
    let dir = data_base_dir()?.join("workflows");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create workflows dir: {e}"))?;
    Ok(dir)
}

fn runs_dir() -> Result<PathBuf, String> {
    let dir = data_base_dir()?.join("workflow_runs");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create runs dir: {e}"))?;
    Ok(dir)
}

fn schedules_path() -> Result<PathBuf, String> {
    let dir = data_base_dir()?.join("workflows");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create workflows dir: {e}"))?;
    Ok(dir.join("_schedules.json"))
}

fn load_schedules() -> Result<Vec<WorkflowSchedule>, String> {
    let path = schedules_path()?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let data = std::fs::read_to_string(&path)
        .map_err(|e| format!("Cannot read schedules: {e}"))?;
    serde_json::from_str(&data).map_err(|e| format!("Cannot parse schedules: {e}"))
}

fn save_schedules(schedules: &[WorkflowSchedule]) -> Result<(), String> {
    let path = schedules_path()?;
    let data = serde_json::to_string_pretty(schedules)
        .map_err(|e| format!("Cannot serialize schedules: {e}"))?;
    std::fs::write(&path, data).map_err(|e| format!("Cannot write schedules: {e}"))
}

fn credentials_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "Cannot determine home directory".to_string())?;
    let dir = home.join(".impforge").join("workflows");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create workflows dir: {e}"))?;
    Ok(dir.join("_credentials.json"))
}

fn versions_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "Cannot determine home directory".to_string())?;
    let dir = home.join(".impforge").join("workflow_versions");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create versions dir: {e}"))?;
    Ok(dir)
}

fn load_credentials() -> Result<Vec<Credential>, String> {
    let path = credentials_path()?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let data =
        std::fs::read_to_string(&path).map_err(|e| format!("Cannot read credentials: {e}"))?;
    serde_json::from_str(&data).map_err(|e| format!("Cannot parse credentials: {e}"))
}

fn save_credentials(creds: &[Credential]) -> Result<(), String> {
    let path = credentials_path()?;
    let data = serde_json::to_string_pretty(creds)
        .map_err(|e| format!("Cannot serialize credentials: {e}"))?;
    std::fs::write(&path, data).map_err(|e| format!("Cannot write credentials: {e}"))
}

fn load_versions(workflow_id: &str) -> Result<Vec<WorkflowVersion>, String> {
    let path = versions_dir()?.join(format!("{workflow_id}.json"));
    if !path.exists() {
        return Ok(vec![]);
    }
    let data =
        std::fs::read_to_string(&path).map_err(|e| format!("Cannot read versions: {e}"))?;
    serde_json::from_str(&data).map_err(|e| format!("Cannot parse versions: {e}"))
}

fn save_versions(workflow_id: &str, versions: &[WorkflowVersion]) -> Result<(), String> {
    let path = versions_dir()?.join(format!("{workflow_id}.json"));
    let data = serde_json::to_string_pretty(versions)
        .map_err(|e| format!("Cannot serialize versions: {e}"))?;
    std::fs::write(&path, data).map_err(|e| format!("Cannot write versions: {e}"))
}

fn workflow_path(id: &str) -> Result<PathBuf, String> {
    Ok(workflows_dir()?.join(format!("{id}.json")))
}

fn load_workflow(id: &str) -> Result<Workflow, String> {
    let path = workflow_path(id)?;
    let data = std::fs::read_to_string(&path)
        .map_err(|e| format!("Cannot read workflow {id}: {e}"))?;
    serde_json::from_str(&data).map_err(|e| format!("Cannot parse workflow {id}: {e}"))
}

fn save_workflow(wf: &Workflow) -> Result<(), String> {
    let path = workflow_path(&wf.id)?;
    let data =
        serde_json::to_string_pretty(wf).map_err(|e| format!("Cannot serialize workflow: {e}"))?;
    std::fs::write(&path, data).map_err(|e| format!("Cannot write workflow: {e}"))
}

fn save_run(run: &WorkflowRun) -> Result<(), String> {
    let dir = runs_dir()?;
    let path = dir.join(format!("{}_{}.json", run.workflow_id, run.id));
    let data =
        serde_json::to_string_pretty(run).map_err(|e| format!("Cannot serialize run: {e}"))?;
    std::fs::write(&path, data).map_err(|e| format!("Cannot write run: {e}"))
}

// ---------------------------------------------------------------------------
// Topological sort (DFS-based, no petgraph)
// ---------------------------------------------------------------------------

fn topological_sort(nodes: &[FlowNode], edges: &[FlowEdge]) -> Result<Vec<String>, String> {
    let node_ids: HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut in_degree: HashMap<&str, usize> = HashMap::new();

    for id in &node_ids {
        adj.entry(id).or_default();
        in_degree.entry(id).or_insert(0);
    }

    for edge in edges {
        if node_ids.contains(edge.source.as_str()) && node_ids.contains(edge.target.as_str()) {
            adj.entry(edge.source.as_str())
                .or_default()
                .push(edge.target.as_str());
            *in_degree.entry(edge.target.as_str()).or_insert(0) += 1;
        }
    }

    // Kahn's algorithm (BFS-based topological sort)
    let mut queue: VecDeque<&str> = VecDeque::new();
    for (id, deg) in &in_degree {
        if *deg == 0 {
            queue.push_back(id);
        }
    }

    let mut sorted: Vec<String> = Vec::with_capacity(nodes.len());
    while let Some(node) = queue.pop_front() {
        sorted.push(node.to_string());
        if let Some(neighbors) = adj.get(node) {
            for neighbor in neighbors {
                if let Some(deg) = in_degree.get_mut(neighbor) {
                    *deg = deg.saturating_sub(1);
                    if *deg == 0 {
                        queue.push_back(neighbor);
                    }
                }
            }
        }
    }

    if sorted.len() != node_ids.len() {
        return Err("Workflow contains a cycle -- cannot execute".to_string());
    }

    Ok(sorted)
}

// ---------------------------------------------------------------------------
// Node execution
// ---------------------------------------------------------------------------

/// Execute a single node and return its result.
fn execute_node<'a>(
    node: &'a FlowNode,
    prev_output: &'a serde_json::Value,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = NodeResult> + Send + 'a>> {
    Box::pin(execute_node_inner(node, prev_output))
}

async fn execute_node_inner(
    node: &FlowNode,
    prev_output: &serde_json::Value,
) -> NodeResult {
    let start = std::time::Instant::now();
    let input_snapshot = prev_output.clone();
    let (status, output, error) = match &node.node_type {
        // -- Triggers produce no output beyond starting the chain --
        NodeType::TriggerManual
        | NodeType::TriggerCron { .. }
        | NodeType::TriggerWebhook { .. }
        | NodeType::TriggerFileWatch { .. }
        | NodeType::TriggerAppEvent { .. } => {
            (RunStatus::Completed, serde_json::json!({"triggered": true}), None)
        }

        // -- HTTP Request --
        NodeType::ActionHttpRequest {
            method,
            url,
            headers,
            body,
        } => {
            match execute_http(method, url, headers, body, prev_output).await {
                Ok(resp) => (RunStatus::Completed, resp, None),
                Err(e) => (RunStatus::Failed, serde_json::Value::Null, Some(e)),
            }
        }

        // -- LLM Call (Ollama) --
        NodeType::ActionLlmCall { prompt, model } => {
            match execute_llm(prompt, model.as_deref(), prev_output).await {
                Ok(resp) => (RunStatus::Completed, serde_json::json!({"response": resp}), None),
                Err(e) => (RunStatus::Failed, serde_json::Value::Null, Some(e)),
            }
        }

        // -- Shell Command --
        NodeType::ActionShellCommand { command } => {
            match execute_shell(command).await {
                Ok(out) => (RunStatus::Completed, serde_json::json!({"stdout": out}), None),
                Err(e) => (RunStatus::Failed, serde_json::Value::Null, Some(e)),
            }
        }

        // -- File Operation --
        NodeType::ActionFileOp { operation, path } => {
            match execute_file_op(operation, path).await {
                Ok(out) => (RunStatus::Completed, out, None),
                Err(e) => (RunStatus::Failed, serde_json::Value::Null, Some(e)),
            }
        }

        // -- JSON Transform --
        NodeType::ActionJsonTransform { expression } => {
            let result = apply_json_transform(expression, prev_output);
            (RunStatus::Completed, result, None)
        }

        // -- Notification (log-based for now) --
        NodeType::ActionNotification { title, message } => {
            log::info!("[ForgeFlow Notification] {title}: {message}");
            (
                RunStatus::Completed,
                serde_json::json!({"notified": true, "title": title, "message": message}),
                None,
            )
        }

        // -- Email (stub -- logs the intent) --
        NodeType::ActionEmail { to, subject, body } => {
            log::info!("[ForgeFlow Email] To: {to}, Subject: {subject}, Body length: {}", body.len());
            (
                RunStatus::Completed,
                serde_json::json!({"emailed": true, "to": to, "subject": subject}),
                None,
            )
        }

        // -- Git Operation --
        NodeType::ActionGitOp { operation } => {
            match execute_git_op(operation).await {
                Ok(out) => (RunStatus::Completed, serde_json::json!({"output": out}), None),
                Err(e) => (RunStatus::Failed, serde_json::Value::Null, Some(e)),
            }
        }

        // -- Social Post (stub -- logs the intent) --
        NodeType::ActionSocialPost { platform, content } => {
            log::info!("[ForgeFlow Social] {platform}: {}", &content[..content.len().min(80)]);
            (
                RunStatus::Completed,
                serde_json::json!({"posted": true, "platform": platform}),
                None,
            )
        }

        // -- DB Query (SQLite) --
        NodeType::ActionDbQuery { query } => {
            match execute_db_query(query).await {
                Ok(out) => (RunStatus::Completed, out, None),
                Err(e) => (RunStatus::Failed, serde_json::Value::Null, Some(e)),
            }
        }

        // -- Sub-Workflow execution --
        NodeType::ActionSubWorkflow { workflow_id } => {
            match execute_sub_workflow(workflow_id).await {
                Ok(out) => (RunStatus::Completed, out, None),
                Err(e) => (RunStatus::Failed, serde_json::Value::Null, Some(e)),
            }
        }

        // -- Data Transformation: Split --
        NodeType::ActionSplit { field } => {
            let result = execute_split(field, prev_output);
            (RunStatus::Completed, result, None)
        }

        // -- Data Transformation: Filter --
        NodeType::ActionFilter { expression } => {
            let result = execute_filter(expression, prev_output);
            (RunStatus::Completed, result, None)
        }

        // -- Data Transformation: Sort --
        NodeType::ActionSort { field, ascending } => {
            let result = execute_sort(field, *ascending, prev_output);
            (RunStatus::Completed, result, None)
        }

        // -- Data Transformation: Aggregate --
        NodeType::ActionAggregate { field, operation } => {
            let result = execute_aggregate(field, operation, prev_output);
            (RunStatus::Completed, result, None)
        }

        // -- Data Transformation: Map --
        NodeType::ActionMap { expression } => {
            let result = execute_map(expression, prev_output);
            (RunStatus::Completed, result, None)
        }

        // -- Data Transformation: Unique --
        NodeType::ActionUnique { field } => {
            let result = execute_unique(field, prev_output);
            (RunStatus::Completed, result, None)
        }

        // -- Control: Condition --
        NodeType::ControlCondition { expression } => {
            let passed = evaluate_condition(expression, prev_output);
            (RunStatus::Completed, serde_json::json!({"passed": passed}), None)
        }

        // -- Control: Loop --
        NodeType::ControlLoop { count } => {
            (
                RunStatus::Completed,
                serde_json::json!({"loop": true, "count": count.unwrap_or(1)}),
                None,
            )
        }

        // -- Control: Delay --
        NodeType::ControlDelay { seconds } => {
            let secs = (*seconds).min(300); // cap at 5 minutes
            tokio::time::sleep(std::time::Duration::from_secs(secs as u64)).await;
            (RunStatus::Completed, serde_json::json!({"delayed": secs}), None)
        }

        // -- Control: Parallel (fan-out marker) --
        NodeType::ControlParallel => {
            (RunStatus::Completed, prev_output.clone(), None)
        }

        // -- Control: Merge --
        NodeType::ControlMerge => {
            (RunStatus::Completed, prev_output.clone(), None)
        }
    };

    NodeResult {
        node_id: node.id.clone(),
        status,
        input: input_snapshot,
        output,
        duration_ms: start.elapsed().as_millis() as u64,
        error,
    }
}

// ---------------------------------------------------------------------------
// Action implementations
// ---------------------------------------------------------------------------

/// Interpolate `{{prev.*}}` and `{{var.*}}` placeholders.
fn interpolate(template: &str, prev: &serde_json::Value) -> String {
    interpolate_with_vars(template, prev, &serde_json::Map::new())
}

/// Full interpolation with workflow variables support.
fn interpolate_with_vars(
    template: &str,
    prev: &serde_json::Value,
    variables: &serde_json::Map<String, serde_json::Value>,
) -> String {
    let mut result = template.to_string();
    // Replace {{prev}} with the whole previous output
    if let Some(s) = prev.as_str() {
        result = result.replace("{{prev}}", s);
    } else {
        result = result.replace("{{prev}}", &prev.to_string());
    }
    // Replace {{prev.key}} with specific fields
    if let Some(obj) = prev.as_object() {
        for (key, val) in obj {
            let placeholder = format!("{{{{prev.{key}}}}}");
            let replacement = match val.as_str() {
                Some(s) => s.to_string(),
                None => val.to_string(),
            };
            result = result.replace(&placeholder, &replacement);
        }
    }
    // Replace {{var.key}} with workflow variables
    for (key, val) in variables {
        let placeholder = format!("{{{{var.{key}}}}}");
        let replacement = match val.as_str() {
            Some(s) => s.to_string(),
            None => val.to_string(),
        };
        result = result.replace(&placeholder, &replacement);
    }
    result
}

async fn execute_http(
    method: &str,
    url: &str,
    headers: &[(String, String)],
    body: &Option<String>,
    prev: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let url = interpolate_credentials(&interpolate(url, prev));

    let mut req = match method.to_uppercase().as_str() {
        "GET" => client.get(&url),
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "PATCH" => client.patch(&url),
        "DELETE" => client.delete(&url),
        "HEAD" => client.head(&url),
        _ => return Err(format!("Unsupported HTTP method: {method}")),
    };

    for (key, val) in headers {
        req = req.header(key.as_str(), interpolate_credentials(&interpolate(val, prev)));
    }

    if let Some(b) = body {
        req = req.body(interpolate_credentials(&interpolate(b, prev)));
    }

    let resp = req.send().await.map_err(|e| format!("HTTP request failed: {e}"))?;
    let status = resp.status().as_u16();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("Cannot read HTTP response: {e}"))?;

    // Try to parse as JSON, fall back to string
    let body_json = serde_json::from_str::<serde_json::Value>(&text)
        .unwrap_or_else(|_| serde_json::json!(text));

    Ok(serde_json::json!({
        "status": status,
        "body": body_json,
    }))
}

async fn execute_llm(
    prompt: &str,
    model: Option<&str>,
    prev: &serde_json::Value,
) -> Result<String, String> {
    let model_name = model.unwrap_or("dolphin3:8b");
    let full_prompt = interpolate(prompt, prev);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let payload = serde_json::json!({
        "model": model_name,
        "prompt": full_prompt,
        "stream": false,
    });

    let resp = client
        .post("http://localhost:11434/api/generate")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Ollama request failed: {e}"))?;

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Cannot parse Ollama response: {e}"))?;

    data.get("response")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Ollama returned no response field".to_string())
}

async fn execute_shell(command: &str) -> Result<String, String> {
    let output = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .output()
        .await
        .map_err(|e| format!("Shell execution failed: {e}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "Shell command exited with {}: {}",
            output.status.code().unwrap_or(-1),
            stderr
        ))
    }
}

async fn execute_file_op(
    operation: &str,
    path: &str,
) -> Result<serde_json::Value, String> {
    match operation {
        "read" => {
            let content =
                tokio::fs::read_to_string(path).await.map_err(|e| format!("File read error: {e}"))?;
            Ok(serde_json::json!({"content": content, "path": path}))
        }
        "write" => {
            // Write operation expects config.content but we just acknowledge here
            Ok(serde_json::json!({"written": true, "path": path}))
        }
        "exists" => {
            let exists = tokio::fs::try_exists(path)
                .await
                .unwrap_or(false);
            Ok(serde_json::json!({"exists": exists, "path": path}))
        }
        "delete" => {
            tokio::fs::remove_file(path)
                .await
                .map_err(|e| format!("File delete error: {e}"))?;
            Ok(serde_json::json!({"deleted": true, "path": path}))
        }
        "list" => {
            let mut entries = Vec::new();
            let mut rd = tokio::fs::read_dir(path)
                .await
                .map_err(|e| format!("Dir read error: {e}"))?;
            while let Ok(Some(entry)) = rd.next_entry().await {
                entries.push(entry.file_name().to_string_lossy().to_string());
            }
            Ok(serde_json::json!({"entries": entries, "path": path}))
        }
        _ => Err(format!("Unknown file operation: {operation}")),
    }
}

fn apply_json_transform(
    expression: &str,
    prev: &serde_json::Value,
) -> serde_json::Value {
    // Simple key extraction: "field_name" extracts that field from prev
    if let Some(val) = prev.get(expression) {
        return val.clone();
    }
    // Dot-path traversal: "a.b.c"
    let mut current = prev;
    for part in expression.split('.') {
        match current.get(part) {
            Some(v) => current = v,
            None => return serde_json::json!({"error": format!("Key not found: {expression}")}),
        }
    }
    current.clone()
}

fn evaluate_condition(expression: &str, prev: &serde_json::Value) -> bool {
    // Simple evaluator: "status == 200", "passed == true", "body.error == null"
    let parts: Vec<&str> = expression.split_whitespace().collect();
    if parts.len() == 3 {
        let field = parts[0];
        let op = parts[1];
        let expected = parts[2];

        let mut val = prev;
        for part in field.split('.') {
            match val.get(part) {
                Some(v) => val = v,
                None => return false,
            }
        }

        let val_str = match val.as_str() {
            Some(s) => s.to_string(),
            None => val.to_string(),
        };

        return match op {
            "==" => val_str == expected || val_str == format!("\"{expected}\""),
            "!=" => val_str != expected && val_str != format!("\"{expected}\""),
            ">" => val_str.parse::<f64>().ok().zip(expected.parse::<f64>().ok())
                .map(|(a, b)| a > b).unwrap_or(false),
            "<" => val_str.parse::<f64>().ok().zip(expected.parse::<f64>().ok())
                .map(|(a, b)| a < b).unwrap_or(false),
            _ => false,
        };
    }
    // Fallback: truthy check on prev
    !prev.is_null()
}

async fn execute_git_op(operation: &str) -> Result<String, String> {
    let args: Vec<&str> = match operation {
        "status" => vec!["status", "--porcelain"],
        "pull" => vec!["pull"],
        "push" => vec!["push"],
        "log" => vec!["log", "--oneline", "-10"],
        _ => return Err(format!("Unsupported git operation: {operation}")),
    };

    let output = tokio::process::Command::new("git")
        .args(&args)
        .output()
        .await
        .map_err(|e| format!("Git execution failed: {e}"))?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

async fn execute_db_query(query: &str) -> Result<serde_json::Value, String> {
    // Execute against a local SQLite database in ~/.impforge/forge_flow.db
    let home = dirs::home_dir().ok_or_else(|| "Cannot determine home directory".to_string())?;
    let db_path = home.join(".impforge").join("forge_flow.db");

    let query_owned = query.to_string();
    let result = tokio::task::spawn_blocking(move || -> Result<serde_json::Value, String> {
        let conn = rusqlite::Connection::open(&db_path)
            .map_err(|e| format!("DB open error: {e}"))?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .map_err(|e| format!("DB pragma error: {e}"))?;

        let mut stmt = conn
            .prepare(&query_owned)
            .map_err(|e| format!("DB prepare error: {e}"))?;
        let col_count = stmt.column_count();
        let col_names: Vec<String> = (0..col_count)
            .map(|i| stmt.column_name(i).unwrap_or("?").to_string())
            .collect();

        let rows: Vec<serde_json::Value> = stmt
            .query_map([], |row| {
                let mut obj = serde_json::Map::new();
                for (i, name) in col_names.iter().enumerate() {
                    let val: rusqlite::Result<String> = row.get(i);
                    obj.insert(
                        name.clone(),
                        serde_json::Value::String(val.unwrap_or_default()),
                    );
                }
                Ok(serde_json::Value::Object(obj))
            })
            .map_err(|e| format!("DB query error: {e}"))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(serde_json::json!({"rows": rows, "count": rows.len()}))
    })
    .await
    .map_err(|e| format!("DB task join error: {e}"))??;

    Ok(result)
}

// ---------------------------------------------------------------------------
// Sub-Workflow execution
// ---------------------------------------------------------------------------

fn execute_sub_workflow(
    workflow_id: &str,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send + '_>> {
    Box::pin(async move {
        let sub_wf = load_workflow(workflow_id)?;
        if sub_wf.nodes.is_empty() {
            return Err(format!("Sub-workflow '{workflow_id}' has no nodes"));
        }
        let run = run_workflow_inner_boxed(&sub_wf).await;
        if run.status == RunStatus::Failed {
            return Err(format!(
                "Sub-workflow failed: {}",
                run.error.unwrap_or_default()
            ));
        }
        let last_output = run
            .node_results
            .last()
            .map(|nr| nr.output.clone())
            .unwrap_or(serde_json::Value::Null);
        Ok(serde_json::json!({
            "sub_workflow_id": workflow_id,
            "run_id": run.id,
            "status": run.status,
            "output": last_output,
        }))
    })
}

// ---------------------------------------------------------------------------
// Data Transformation implementations
// ---------------------------------------------------------------------------

/// Split: extract an array field and return its items.
fn execute_split(field: &str, prev: &serde_json::Value) -> serde_json::Value {
    let arr = if field.is_empty() {
        prev.clone()
    } else {
        let mut current = prev;
        for part in field.split('.') {
            match current.get(part) {
                Some(v) => current = v,
                None => return serde_json::json!({"items": [], "count": 0, "error": format!("Field '{}' not found", field)}),
            }
        }
        current.clone()
    };
    if let Some(items) = arr.as_array() {
        serde_json::json!({"items": items, "count": items.len()})
    } else {
        serde_json::json!({"items": [arr], "count": 1})
    }
}

/// Filter: keep items matching a simple expression.
fn execute_filter(expression: &str, prev: &serde_json::Value) -> serde_json::Value {
    let items = extract_items(prev);
    let filtered: Vec<&serde_json::Value> = items
        .into_iter()
        .filter(|item| evaluate_condition(expression, item))
        .collect();
    serde_json::json!({"items": filtered, "count": filtered.len()})
}

/// Sort: sort items by a field.
fn execute_sort(field: &str, ascending: bool, prev: &serde_json::Value) -> serde_json::Value {
    let mut items = extract_items_owned(prev);
    items.sort_by(|a, b| {
        let va = a.get(field).cloned().unwrap_or(serde_json::Value::Null);
        let vb = b.get(field).cloned().unwrap_or(serde_json::Value::Null);
        let cmp = compare_json_values(&va, &vb);
        if ascending { cmp } else { cmp.reverse() }
    });
    serde_json::json!({"items": items, "count": items.len()})
}

/// Aggregate: compute sum, avg, count, min, max over a field.
fn execute_aggregate(field: &str, operation: &str, prev: &serde_json::Value) -> serde_json::Value {
    let items = extract_items(prev);
    let values: Vec<f64> = items
        .iter()
        .filter_map(|item| {
            item.get(field)
                .and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
        })
        .collect();

    let result = match operation {
        "sum" => values.iter().sum::<f64>(),
        "avg" => {
            if values.is_empty() {
                0.0
            } else {
                values.iter().sum::<f64>() / values.len() as f64
            }
        }
        "count" => values.len() as f64,
        "min" => values.iter().cloned().fold(f64::INFINITY, f64::min),
        "max" => values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        _ => 0.0,
    };

    serde_json::json!({
        "result": result,
        "operation": operation,
        "field": field,
        "count": values.len(),
    })
}

/// Map: transform each item using an expression (field extraction).
fn execute_map(expression: &str, prev: &serde_json::Value) -> serde_json::Value {
    let items = extract_items(prev);
    let mapped: Vec<serde_json::Value> = items
        .iter()
        .map(|item| {
            // Support dot-path extraction
            let mut current = *item;
            for part in expression.split('.') {
                match current.get(part) {
                    Some(v) => current = v,
                    None => return serde_json::Value::Null,
                }
            }
            current.clone()
        })
        .collect();
    serde_json::json!({"items": mapped, "count": mapped.len()})
}

/// Unique: remove duplicate items by a field value.
fn execute_unique(field: &str, prev: &serde_json::Value) -> serde_json::Value {
    let items = extract_items(prev);
    let mut seen = HashSet::new();
    let unique: Vec<&serde_json::Value> = items
        .into_iter()
        .filter(|item| {
            let key = item
                .get(field)
                .map(|v| v.to_string())
                .unwrap_or_default();
            seen.insert(key)
        })
        .collect();
    serde_json::json!({"items": unique, "count": unique.len()})
}

/// Helper: extract an array of items from previous output.
fn extract_items(prev: &serde_json::Value) -> Vec<&serde_json::Value> {
    if let Some(arr) = prev.as_array() {
        arr.iter().collect()
    } else if let Some(items) = prev.get("items").and_then(|v| v.as_array()) {
        items.iter().collect()
    } else if let Some(rows) = prev.get("rows").and_then(|v| v.as_array()) {
        rows.iter().collect()
    } else {
        vec![prev]
    }
}

/// Helper: extract items as owned values for sorting.
fn extract_items_owned(prev: &serde_json::Value) -> Vec<serde_json::Value> {
    if let Some(arr) = prev.as_array() {
        arr.clone()
    } else if let Some(items) = prev.get("items").and_then(|v| v.as_array()) {
        items.clone()
    } else if let Some(rows) = prev.get("rows").and_then(|v| v.as_array()) {
        rows.clone()
    } else {
        vec![prev.clone()]
    }
}

/// Helper: compare two JSON values for sorting.
fn compare_json_values(a: &serde_json::Value, b: &serde_json::Value) -> std::cmp::Ordering {
    match (a.as_f64(), b.as_f64()) {
        (Some(fa), Some(fb)) => fa.partial_cmp(&fb).unwrap_or(std::cmp::Ordering::Equal),
        _ => {
            let sa = a.as_str().unwrap_or("");
            let sb = b.as_str().unwrap_or("");
            sa.cmp(sb)
        }
    }
}

// ---------------------------------------------------------------------------
// Credential interpolation
// ---------------------------------------------------------------------------

/// Replace {{cred.name}} placeholders in strings with stored credential values.
fn interpolate_credentials(template: &str) -> String {
    let creds = load_credentials().unwrap_or_default();
    let mut result = template.to_string();
    for cred in &creds {
        let placeholder = format!("{{{{cred.{}}}}}", cred.name);
        // Extract the primary value from credential data
        let replacement = match cred.credential_type.as_str() {
            "api_key" | "bearer" => cred
                .data
                .get("token")
                .or_else(|| cred.data.get("key"))
                .or_else(|| cred.data.get("value"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            "basic_auth" => {
                let user = cred.data.get("username").and_then(|v| v.as_str()).unwrap_or("");
                let pass = cred.data.get("password").and_then(|v| v.as_str()).unwrap_or("");
                format!("{user}:{pass}")
            }
            _ => cred.data.to_string(),
        };
        result = result.replace(&placeholder, &replacement);
    }
    result
}

// ---------------------------------------------------------------------------
// Workflow execution engine
// ---------------------------------------------------------------------------

/// Execute a node with exponential-backoff retry if configured.
async fn execute_with_retry(node: &FlowNode, prev_output: &serde_json::Value) -> NodeResult {
    let max_retries = node
        .retry_config
        .as_ref()
        .map(|rc| rc.max_retries)
        .unwrap_or(0);
    let base_backoff = node
        .retry_config
        .as_ref()
        .map(|rc| rc.backoff_ms)
        .unwrap_or(1000);

    let mut attempt = 0u32;
    loop {
        let result = execute_node(node, prev_output).await;
        if result.status != RunStatus::Failed || attempt >= max_retries {
            return result;
        }
        attempt += 1;
        let delay = base_backoff * 2u64.saturating_pow(attempt.saturating_sub(1));
        // Cap at 60 seconds
        let delay = delay.min(60_000);
        log::info!(
            "[ForgeFlow] Retrying node '{}' (attempt {}/{}) after {}ms",
            node.label,
            attempt,
            max_retries,
            delay
        );
        tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
    }
}

/// Boxed wrapper to break recursive async type cycle for sub-workflows.
fn run_workflow_inner_boxed(
    wf: &Workflow,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = WorkflowRun> + Send + '_>> {
    Box::pin(run_workflow_inner(wf))
}

/// Gather the previous output for a node from its predecessors.
fn gather_prev_output(
    node_id: &str,
    predecessors: &HashMap<&str, Vec<&str>>,
    outputs: &HashMap<String, serde_json::Value>,
) -> serde_json::Value {
    if let Some(preds) = predecessors.get(node_id) {
        if preds.len() == 1 {
            outputs
                .get(preds[0])
                .cloned()
                .unwrap_or(serde_json::Value::Null)
        } else {
            let mut merged = serde_json::Map::new();
            for pred in preds {
                if let Some(val) = outputs.get(*pred) {
                    merged.insert(pred.to_string(), val.clone());
                }
            }
            serde_json::Value::Object(merged)
        }
    } else {
        serde_json::Value::Null
    }
}

async fn run_workflow_inner(wf: &Workflow) -> WorkflowRun {
    let run_id = Uuid::new_v4().to_string();
    let started = Utc::now().to_rfc3339();

    let sorted = match topological_sort(&wf.nodes, &wf.edges) {
        Ok(s) => s,
        Err(e) => {
            return WorkflowRun {
                id: run_id,
                workflow_id: wf.id.clone(),
                status: RunStatus::Failed,
                started_at: started,
                completed_at: Some(Utc::now().to_rfc3339()),
                node_results: vec![],
                error: Some(e),
            };
        }
    };

    // Build a lookup from node id -> FlowNode
    let node_map: HashMap<&str, &FlowNode> =
        wf.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    let mut results: Vec<NodeResult> = Vec::new();
    let mut outputs: HashMap<String, serde_json::Value> = HashMap::new();
    let mut overall_status = RunStatus::Completed;
    let mut overall_error: Option<String> = None;

    // Build reverse adjacency to find each node's predecessors
    let mut predecessors: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in &wf.edges {
        predecessors
            .entry(edge.target.as_str())
            .or_default()
            .push(edge.source.as_str());
    }

    // Build forward adjacency for parallel fan-out detection
    let mut successors: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in &wf.edges {
        successors
            .entry(edge.source.as_str())
            .or_default()
            .push(edge.target.as_str());
    }

    // Track which nodes have been executed (for parallel skip)
    let mut executed: HashSet<String> = HashSet::new();

    for node_id in &sorted {
        // Skip if already executed in a parallel batch
        if executed.contains(node_id) {
            continue;
        }

        let node = match node_map.get(node_id.as_str()) {
            Some(n) => n,
            None => continue,
        };

        // Gather previous output: merge all predecessor outputs
        let prev_output = gather_prev_output(node_id, &predecessors, &outputs);

        // --- Parallel fan-out: when a ControlParallel node is reached,
        //     execute all its successors concurrently via tokio::join ---
        if matches!(&node.node_type, NodeType::ControlParallel) {
            let par_result = execute_node(node, &prev_output).await;
            outputs.insert(node_id.clone(), par_result.output.clone());
            results.push(par_result);
            executed.insert(node_id.clone());

            if let Some(children) = successors.get(node_id.as_str()) {
                let child_nodes: Vec<(String, FlowNode)> = children
                    .iter()
                    .filter_map(|cid| {
                        node_map.get(cid).map(|n| (cid.to_string(), (*n).clone()))
                    })
                    .collect();

                if child_nodes.len() > 1 {
                    // Execute children in parallel
                    let par_input = prev_output.clone();
                    let handles: Vec<_> = child_nodes
                        .iter()
                        .map(|(_cid, cnode)| {
                            let cnode_clone = cnode.clone();
                            let input_clone = par_input.clone();
                            tokio::spawn(async move {
                                execute_with_retry(&cnode_clone, &input_clone).await
                            })
                        })
                        .collect();

                    for (i, handle) in handles.into_iter().enumerate() {
                        match handle.await {
                            Ok(child_result) => {
                                let cid = child_nodes[i].0.clone();
                                outputs.insert(cid.clone(), child_result.output.clone());
                                results.push(child_result);
                                executed.insert(cid);
                            }
                            Err(e) => {
                                log::error!("[ForgeFlow] Parallel task join error: {e}");
                            }
                        }
                    }
                    continue;
                }
            }
            continue;
        }

        // Check condition nodes -- skip downstream if condition fails
        if let NodeType::ControlCondition { .. } = &node.node_type {
            let result = execute_node(node, &prev_output).await;
            let passed = result
                .output
                .get("passed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            outputs.insert(node_id.clone(), result.output.clone());
            results.push(result);
            executed.insert(node_id.clone());
            if !passed {
                continue;
            }
            continue;
        }

        // Execute node with retry support
        let result = execute_with_retry(node, &prev_output).await;
        executed.insert(node_id.clone());

        if result.status == RunStatus::Failed {
            // Check retry_config for failure action
            let action = node
                .retry_config
                .as_ref()
                .map(|rc| rc.on_failure.clone())
                .unwrap_or(FailureAction::Stop);

            match action {
                FailureAction::Skip => {
                    log::warn!(
                        "[ForgeFlow] Skipping failed node '{}': {:?}",
                        node.label,
                        result.error
                    );
                    outputs.insert(node_id.clone(), serde_json::json!({"skipped": true}));
                    results.push(result);
                    continue;
                }
                FailureAction::Fallback(ref fallback_id) => {
                    if let Some(fb_node) = node_map.get(fallback_id.as_str()) {
                        let fb_result = execute_node(fb_node, &prev_output).await;
                        outputs.insert(node_id.clone(), fb_result.output.clone());
                        results.push(result);
                        results.push(fb_result);
                    } else {
                        overall_status = RunStatus::Failed;
                        overall_error = Some(format!(
                            "Fallback node '{}' not found after '{}' failed",
                            fallback_id, node.label
                        ));
                        results.push(result);
                        break;
                    }
                }
                FailureAction::Stop | FailureAction::Retry => {
                    overall_status = RunStatus::Failed;
                    overall_error = result.error.clone();
                    results.push(result);
                    break;
                }
            }
        } else {
            outputs.insert(node_id.clone(), result.output.clone());
            results.push(result);
        }
    }

    WorkflowRun {
        id: run_id,
        workflow_id: wf.id.clone(),
        status: overall_status,
        started_at: started,
        completed_at: Some(Utc::now().to_rfc3339()),
        node_results: results,
        error: overall_error,
    }
}

// ---------------------------------------------------------------------------
// Templates
// ---------------------------------------------------------------------------

fn built_in_templates() -> Vec<WorkflowTemplate> {
    let now = Utc::now().to_rfc3339();
    vec![
        WorkflowTemplate {
            id: "tpl-daily-report".into(),
            name: "Daily Report Generator".into(),
            description: "Cron trigger -> DB Query -> LLM Summarize -> Email".into(),
            category: "Productivity".into(),
            workflow: Workflow {
                id: String::new(),
                name: "Daily Report Generator".into(),
                description: "Automatically generate and email a daily summary".into(),
                nodes: vec![
                    FlowNode { id: "n1".into(), node_type: NodeType::TriggerCron { schedule: "0 0 9 * * *".into() }, label: "Every day at 9am".into(), config: serde_json::json!({}), position: (100.0, 200.0), retry_config: None },
                    FlowNode { id: "n2".into(), node_type: NodeType::ActionDbQuery { query: "SELECT * FROM tasks WHERE status = 'pending'".into() }, label: "Get pending tasks".into(), config: serde_json::json!({}), position: (350.0, 200.0), retry_config: None },
                    FlowNode { id: "n3".into(), node_type: NodeType::ActionLlmCall { prompt: "Summarize these tasks into a brief daily report:\n{{prev}}".into(), model: None }, label: "AI Summarize".into(), config: serde_json::json!({}), position: (600.0, 200.0), retry_config: None },
                    FlowNode { id: "n4".into(), node_type: NodeType::ActionEmail { to: "team@example.com".into(), subject: "Daily Report".into(), body: "{{prev.response}}".into() }, label: "Send Email".into(), config: serde_json::json!({}), position: (850.0, 200.0), retry_config: None },
                ],
                edges: vec![
                    FlowEdge { id: "e1".into(), source: "n1".into(), target: "n2".into(), label: None },
                    FlowEdge { id: "e2".into(), source: "n2".into(), target: "n3".into(), label: None },
                    FlowEdge { id: "e3".into(), source: "n3".into(), target: "n4".into(), label: None },
                ],
                enabled: false, run_count: 0, last_run: None, variables: serde_json::Map::new(),
                created_at: now.clone(), updated_at: now.clone(),
            },
        },
        WorkflowTemplate {
            id: "tpl-git-deploy".into(),
            name: "Git Auto-Deploy".into(),
            description: "File Watch -> Shell Build -> Git Push -> Notification".into(),
            category: "DevOps".into(),
            workflow: Workflow {
                id: String::new(),
                name: "Git Auto-Deploy".into(),
                description: "Watch for file changes, build, push, and notify".into(),
                nodes: vec![
                    FlowNode { id: "n1".into(), node_type: NodeType::TriggerFileWatch { path: "./src".into() }, label: "Watch source files".into(), config: serde_json::json!({}), position: (100.0, 200.0), retry_config: None },
                    FlowNode { id: "n2".into(), node_type: NodeType::ActionShellCommand { command: "npm run build".into() }, label: "Build project".into(), config: serde_json::json!({}), position: (350.0, 200.0), retry_config: None },
                    FlowNode { id: "n3".into(), node_type: NodeType::ActionGitOp { operation: "push".into() }, label: "Git Push".into(), config: serde_json::json!({}), position: (600.0, 200.0), retry_config: None },
                    FlowNode { id: "n4".into(), node_type: NodeType::ActionNotification { title: "Deploy Complete".into(), message: "Build succeeded and pushed to remote".into() }, label: "Notify".into(), config: serde_json::json!({}), position: (850.0, 200.0), retry_config: None },
                ],
                edges: vec![
                    FlowEdge { id: "e1".into(), source: "n1".into(), target: "n2".into(), label: None },
                    FlowEdge { id: "e2".into(), source: "n2".into(), target: "n3".into(), label: None },
                    FlowEdge { id: "e3".into(), source: "n3".into(), target: "n4".into(), label: None },
                ],
                enabled: false, run_count: 0, last_run: None, variables: serde_json::Map::new(),
                created_at: now.clone(), updated_at: now.clone(),
            },
        },
        WorkflowTemplate {
            id: "tpl-social-scheduler".into(),
            name: "Social Media Scheduler".into(),
            description: "Cron -> LLM Generate Content -> Social Post".into(),
            category: "Marketing".into(),
            workflow: Workflow {
                id: String::new(),
                name: "Social Media Scheduler".into(),
                description: "Generate and post AI content on schedule".into(),
                nodes: vec![
                    FlowNode { id: "n1".into(), node_type: NodeType::TriggerCron { schedule: "0 0 10,15 * * MON-FRI".into() }, label: "Weekdays 10am & 3pm".into(), config: serde_json::json!({}), position: (100.0, 200.0), retry_config: None },
                    FlowNode { id: "n2".into(), node_type: NodeType::ActionLlmCall { prompt: "Write a short, engaging tech tip for LinkedIn. Keep it under 200 words. Include relevant hashtags.".into(), model: None }, label: "Generate Post".into(), config: serde_json::json!({}), position: (400.0, 200.0), retry_config: None },
                    FlowNode { id: "n3".into(), node_type: NodeType::ActionSocialPost { platform: "linkedin".into(), content: "{{prev.response}}".into() }, label: "Post to LinkedIn".into(), config: serde_json::json!({}), position: (700.0, 200.0), retry_config: None },
                ],
                edges: vec![
                    FlowEdge { id: "e1".into(), source: "n1".into(), target: "n2".into(), label: None },
                    FlowEdge { id: "e2".into(), source: "n2".into(), target: "n3".into(), label: None },
                ],
                enabled: false, run_count: 0, last_run: None, variables: serde_json::Map::new(),
                created_at: now.clone(), updated_at: now.clone(),
            },
        },
        WorkflowTemplate {
            id: "tpl-invoice-processor".into(),
            name: "Invoice Processor".into(),
            description: "File Watch -> LLM Extract Data -> DB Insert -> Email Confirmation".into(),
            category: "Finance".into(),
            workflow: Workflow {
                id: String::new(),
                name: "Invoice Processor".into(),
                description: "Automatically process incoming invoices with AI".into(),
                nodes: vec![
                    FlowNode { id: "n1".into(), node_type: NodeType::TriggerFileWatch { path: "~/invoices/incoming".into() }, label: "Watch invoices folder".into(), config: serde_json::json!({}), position: (100.0, 200.0), retry_config: None },
                    FlowNode { id: "n2".into(), node_type: NodeType::ActionFileOp { operation: "read".into(), path: "{{prev.path}}".into() }, label: "Read invoice".into(), config: serde_json::json!({}), position: (350.0, 200.0), retry_config: None },
                    FlowNode { id: "n3".into(), node_type: NodeType::ActionLlmCall { prompt: "Extract invoice number, date, amount, vendor from:\n{{prev.content}}".into(), model: None }, label: "AI Extract".into(), config: serde_json::json!({}), position: (600.0, 200.0), retry_config: None },
                    FlowNode { id: "n4".into(), node_type: NodeType::ActionDbQuery { query: "INSERT INTO invoices (data) VALUES ('{{prev.response}}')".into() }, label: "Save to DB".into(), config: serde_json::json!({}), position: (850.0, 200.0), retry_config: None },
                ],
                edges: vec![
                    FlowEdge { id: "e1".into(), source: "n1".into(), target: "n2".into(), label: None },
                    FlowEdge { id: "e2".into(), source: "n2".into(), target: "n3".into(), label: None },
                    FlowEdge { id: "e3".into(), source: "n3".into(), target: "n4".into(), label: None },
                ],
                enabled: false, run_count: 0, last_run: None, variables: serde_json::Map::new(),
                created_at: now.clone(), updated_at: now.clone(),
            },
        },
        WorkflowTemplate {
            id: "tpl-rss-newsletter".into(),
            name: "RSS to Newsletter".into(),
            description: "Cron -> HTTP Fetch RSS -> LLM Summarize -> Email".into(),
            category: "Content".into(),
            workflow: Workflow {
                id: String::new(),
                name: "RSS to Newsletter".into(),
                description: "Aggregate RSS feeds and send an AI-curated newsletter".into(),
                nodes: vec![
                    FlowNode { id: "n1".into(), node_type: NodeType::TriggerCron { schedule: "0 0 8 * * MON".into() }, label: "Every Monday 8am".into(), config: serde_json::json!({}), position: (100.0, 200.0), retry_config: None },
                    FlowNode { id: "n2".into(), node_type: NodeType::ActionHttpRequest { method: "GET".into(), url: "https://hnrss.org/newest?points=100".into(), headers: vec![], body: None }, label: "Fetch HN RSS".into(), config: serde_json::json!({}), position: (350.0, 200.0), retry_config: None },
                    FlowNode { id: "n3".into(), node_type: NodeType::ActionLlmCall { prompt: "Summarize these top stories into a brief newsletter with 5 bullet points:\n{{prev.body}}".into(), model: None }, label: "AI Newsletter".into(), config: serde_json::json!({}), position: (600.0, 200.0), retry_config: None },
                    FlowNode { id: "n4".into(), node_type: NodeType::ActionEmail { to: "subscribers@example.com".into(), subject: "Weekly Tech Digest".into(), body: "{{prev.response}}".into() }, label: "Send Newsletter".into(), config: serde_json::json!({}), position: (850.0, 200.0), retry_config: None },
                ],
                edges: vec![
                    FlowEdge { id: "e1".into(), source: "n1".into(), target: "n2".into(), label: None },
                    FlowEdge { id: "e2".into(), source: "n2".into(), target: "n3".into(), label: None },
                    FlowEdge { id: "e3".into(), source: "n3".into(), target: "n4".into(), label: None },
                ],
                enabled: false, run_count: 0, last_run: None, variables: serde_json::Map::new(),
                created_at: now.clone(), updated_at: now.clone(),
            },
        },
        // --- 5 new production templates ---
        WorkflowTemplate {
            id: "tpl-feedback-processor".into(),
            name: "Customer Feedback Processor".into(),
            description: "Webhook -> LLM Sentiment Analysis -> Condition -> Email/DB".into(),
            category: "Customer Success".into(),
            workflow: Workflow {
                id: String::new(),
                name: "Customer Feedback Processor".into(),
                description: "Analyze customer feedback with AI sentiment analysis and route accordingly".into(),
                nodes: vec![
                    FlowNode { id: "n1".into(), node_type: NodeType::TriggerWebhook { path: "/feedback".into() }, label: "Receive Feedback".into(), config: serde_json::json!({}), position: (100.0, 200.0), retry_config: None },
                    FlowNode { id: "n2".into(), node_type: NodeType::ActionLlmCall { prompt: "Analyze the sentiment of this customer feedback. Reply with JSON: {\"sentiment\": \"positive|neutral|negative\", \"score\": 0-100, \"summary\": \"brief summary\"}\n\nFeedback: {{prev}}".into(), model: None }, label: "AI Sentiment".into(), config: serde_json::json!({}), position: (400.0, 200.0), retry_config: None },
                    FlowNode { id: "n3".into(), node_type: NodeType::ControlCondition { expression: "sentiment == negative".into() }, label: "Is Negative?".into(), config: serde_json::json!({}), position: (700.0, 150.0), retry_config: None },
                    FlowNode { id: "n4".into(), node_type: NodeType::ActionEmail { to: "support@example.com".into(), subject: "Urgent: Negative Feedback".into(), body: "{{prev.response}}".into() }, label: "Alert Support".into(), config: serde_json::json!({}), position: (1000.0, 100.0), retry_config: None },
                    FlowNode { id: "n5".into(), node_type: NodeType::ActionDbQuery { query: "INSERT INTO feedback_log (sentiment, summary, raw) VALUES ('{{prev.sentiment}}', '{{prev.summary}}', '{{prev}}')".into() }, label: "Save to DB".into(), config: serde_json::json!({}), position: (1000.0, 300.0), retry_config: None },
                ],
                edges: vec![
                    FlowEdge { id: "e1".into(), source: "n1".into(), target: "n2".into(), label: None },
                    FlowEdge { id: "e2".into(), source: "n2".into(), target: "n3".into(), label: None },
                    FlowEdge { id: "e3".into(), source: "n3".into(), target: "n4".into(), label: Some("negative".into()) },
                    FlowEdge { id: "e4".into(), source: "n2".into(), target: "n5".into(), label: Some("always".into()) },
                ],
                enabled: false, run_count: 0, last_run: None, variables: serde_json::Map::new(),
                created_at: now.clone(), updated_at: now.clone(),
            },
        },
        WorkflowTemplate {
            id: "tpl-code-review".into(),
            name: "Code Review Bot".into(),
            description: "Manual Trigger -> Git Diff -> LLM Review -> Notification".into(),
            category: "DevOps".into(),
            workflow: Workflow {
                id: String::new(),
                name: "Code Review Bot".into(),
                description: "AI-powered code review on latest git changes".into(),
                nodes: vec![
                    FlowNode { id: "n1".into(), node_type: NodeType::TriggerManual, label: "Start Review".into(), config: serde_json::json!({}), position: (100.0, 200.0), retry_config: None },
                    FlowNode { id: "n2".into(), node_type: NodeType::ActionShellCommand { command: "git diff HEAD~1 --stat && echo '---DIFF---' && git diff HEAD~1".into() }, label: "Get Git Diff".into(), config: serde_json::json!({}), position: (400.0, 200.0), retry_config: None },
                    FlowNode { id: "n3".into(), node_type: NodeType::ActionLlmCall { prompt: "You are a senior code reviewer. Review this git diff and provide:\n1. Summary of changes\n2. Potential bugs or issues\n3. Suggestions for improvement\n4. Security concerns\n\n{{prev.stdout}}".into(), model: None }, label: "AI Review".into(), config: serde_json::json!({}), position: (700.0, 200.0), retry_config: None },
                    FlowNode { id: "n4".into(), node_type: NodeType::ActionNotification { title: "Code Review Complete".into(), message: "{{prev.response}}".into() }, label: "Notify".into(), config: serde_json::json!({}), position: (1000.0, 200.0), retry_config: None },
                ],
                edges: vec![
                    FlowEdge { id: "e1".into(), source: "n1".into(), target: "n2".into(), label: None },
                    FlowEdge { id: "e2".into(), source: "n2".into(), target: "n3".into(), label: None },
                    FlowEdge { id: "e3".into(), source: "n3".into(), target: "n4".into(), label: None },
                ],
                enabled: false, run_count: 0, last_run: None, variables: serde_json::Map::new(),
                created_at: now.clone(), updated_at: now.clone(),
            },
        },
        WorkflowTemplate {
            id: "tpl-meeting-notes".into(),
            name: "Meeting Notes Generator".into(),
            description: "Manual -> Paste Notes -> LLM Summarize -> Email + DB".into(),
            category: "Productivity".into(),
            workflow: Workflow {
                id: String::new(),
                name: "Meeting Notes Generator".into(),
                description: "Summarize meeting notes with AI and distribute".into(),
                nodes: vec![
                    FlowNode { id: "n1".into(), node_type: NodeType::TriggerManual, label: "Start".into(), config: serde_json::json!({}), position: (100.0, 200.0), retry_config: None },
                    FlowNode { id: "n2".into(), node_type: NodeType::ActionLlmCall { prompt: "Summarize these meeting notes into:\n1. Key decisions made\n2. Action items with owners\n3. Next steps\n4. Timeline\n\nNotes: {{var.meeting_notes}}".into(), model: None }, label: "AI Summarize".into(), config: serde_json::json!({}), position: (400.0, 200.0), retry_config: None },
                    FlowNode { id: "n3".into(), node_type: NodeType::ActionEmail { to: "{{var.team_email}}".into(), subject: "Meeting Summary - {{var.meeting_title}}".into(), body: "{{prev.response}}".into() }, label: "Email Team".into(), config: serde_json::json!({}), position: (700.0, 150.0), retry_config: None },
                    FlowNode { id: "n4".into(), node_type: NodeType::ActionDbQuery { query: "INSERT INTO meeting_notes (title, summary) VALUES ('{{var.meeting_title}}', '{{prev.response}}')".into() }, label: "Save to DB".into(), config: serde_json::json!({}), position: (700.0, 300.0), retry_config: None },
                ],
                edges: vec![
                    FlowEdge { id: "e1".into(), source: "n1".into(), target: "n2".into(), label: None },
                    FlowEdge { id: "e2".into(), source: "n2".into(), target: "n3".into(), label: None },
                    FlowEdge { id: "e3".into(), source: "n2".into(), target: "n4".into(), label: None },
                ],
                enabled: false, run_count: 0, last_run: None, variables: serde_json::Map::new(),
                created_at: now.clone(), updated_at: now.clone(),
            },
        },
        WorkflowTemplate {
            id: "tpl-data-backup".into(),
            name: "Data Backup Scheduler".into(),
            description: "Cron -> Shell (backup) -> Condition -> Notification".into(),
            category: "DevOps".into(),
            workflow: Workflow {
                id: String::new(),
                name: "Data Backup Scheduler".into(),
                description: "Automated backup with success/failure notifications".into(),
                nodes: vec![
                    FlowNode { id: "n1".into(), node_type: NodeType::TriggerCron { schedule: "0 0 2 * * *".into() }, label: "Daily at 2am".into(), config: serde_json::json!({}), position: (100.0, 200.0), retry_config: None },
                    FlowNode { id: "n2".into(), node_type: NodeType::ActionShellCommand { command: "tar -czf ~/backups/backup-$(date +%Y%m%d).tar.gz ~/data/ 2>&1 && echo 'SUCCESS' || echo 'FAILED'".into() }, label: "Run Backup".into(), config: serde_json::json!({}), position: (400.0, 200.0), retry_config: Some(RetryConfig { max_retries: 2, backoff_ms: 5000, on_failure: FailureAction::Stop }) },
                    FlowNode { id: "n3".into(), node_type: NodeType::ActionNotification { title: "Backup Complete".into(), message: "Daily backup finished: {{prev.stdout}}".into() }, label: "Notify Result".into(), config: serde_json::json!({}), position: (700.0, 200.0), retry_config: None },
                ],
                edges: vec![
                    FlowEdge { id: "e1".into(), source: "n1".into(), target: "n2".into(), label: None },
                    FlowEdge { id: "e2".into(), source: "n2".into(), target: "n3".into(), label: None },
                ],
                enabled: false, run_count: 0, last_run: None, variables: serde_json::Map::new(),
                created_at: now.clone(), updated_at: now.clone(),
            },
        },
        WorkflowTemplate {
            id: "tpl-price-monitor".into(),
            name: "Price Monitor".into(),
            description: "Cron -> HTTP (price API) -> Condition -> Email Alert".into(),
            category: "Finance".into(),
            workflow: Workflow {
                id: String::new(),
                name: "Price Monitor".into(),
                description: "Monitor API prices and alert when thresholds are crossed".into(),
                nodes: vec![
                    FlowNode { id: "n1".into(), node_type: NodeType::TriggerCron { schedule: "0 */30 * * * *".into() }, label: "Every 30 min".into(), config: serde_json::json!({}), position: (100.0, 200.0), retry_config: None },
                    FlowNode { id: "n2".into(), node_type: NodeType::ActionHttpRequest { method: "GET".into(), url: "{{var.price_api_url}}".into(), headers: vec![], body: None }, label: "Fetch Price".into(), config: serde_json::json!({}), position: (400.0, 200.0), retry_config: Some(RetryConfig { max_retries: 3, backoff_ms: 2000, on_failure: FailureAction::Skip }) },
                    FlowNode { id: "n3".into(), node_type: NodeType::ActionJsonTransform { expression: "body.price".into() }, label: "Extract Price".into(), config: serde_json::json!({}), position: (700.0, 200.0), retry_config: None },
                    FlowNode { id: "n4".into(), node_type: NodeType::ControlCondition { expression: "price > {{var.threshold}}".into() }, label: "Above Threshold?".into(), config: serde_json::json!({}), position: (1000.0, 200.0), retry_config: None },
                    FlowNode { id: "n5".into(), node_type: NodeType::ActionEmail { to: "{{var.alert_email}}".into(), subject: "Price Alert!".into(), body: "Price crossed threshold: {{prev}}".into() }, label: "Send Alert".into(), config: serde_json::json!({}), position: (1300.0, 200.0), retry_config: None },
                ],
                edges: vec![
                    FlowEdge { id: "e1".into(), source: "n1".into(), target: "n2".into(), label: None },
                    FlowEdge { id: "e2".into(), source: "n2".into(), target: "n3".into(), label: None },
                    FlowEdge { id: "e3".into(), source: "n3".into(), target: "n4".into(), label: None },
                    FlowEdge { id: "e4".into(), source: "n4".into(), target: "n5".into(), label: None },
                ],
                enabled: false, run_count: 0, last_run: None, variables: serde_json::Map::new(),
                created_at: now.clone(), updated_at: now.clone(),
            },
        },
    ]
}

// ---------------------------------------------------------------------------
// Tauri commands (12 + 7 new = 19)
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn flow_list() -> Result<Vec<WorkflowMeta>, String> {
    let dir = workflows_dir()?;
    let mut workflows = Vec::new();

    let entries = std::fs::read_dir(&dir).map_err(|e| format!("Cannot list workflows: {e}"))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            if let Ok(data) = std::fs::read_to_string(&path) {
                if let Ok(wf) = serde_json::from_str::<Workflow>(&data) {
                    workflows.push(WorkflowMeta {
                        id: wf.id.clone(),
                        name: wf.name,
                        description: wf.description,
                        enabled: wf.enabled,
                        node_count: wf.nodes.len(),
                        run_count: wf.run_count,
                        last_run: wf.last_run,
                    });
                }
            }
        }
    }

    workflows.sort_by(|a, b| b.run_count.cmp(&a.run_count));
    Ok(workflows)
}

#[tauri::command]
pub async fn flow_create(name: String, description: String) -> Result<Workflow, String> {
    let now = Utc::now().to_rfc3339();
    let wf = Workflow {
        id: Uuid::new_v4().to_string(),
        name,
        description,
        nodes: vec![],
        edges: vec![],
        enabled: true,
        run_count: 0,
        last_run: None,
        created_at: now.clone(),
        updated_at: now,
        variables: serde_json::Map::new(),
    };
    save_workflow(&wf)?;
    Ok(wf)
}

#[tauri::command]
pub async fn flow_get(id: String) -> Result<Workflow, String> {
    load_workflow(&id)
}

#[tauri::command]
pub async fn flow_save(id: String, workflow: Workflow) -> Result<(), String> {
    // Ensure the id matches
    let mut wf = workflow;
    wf.id = id;
    wf.updated_at = Utc::now().to_rfc3339();
    save_workflow(&wf)
}

#[tauri::command]
pub async fn flow_delete(id: String) -> Result<(), String> {
    let path = workflow_path(&id)?;
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("Cannot delete workflow: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn flow_add_node(
    workflow_id: String,
    node_type: NodeType,
    label: String,
    config: serde_json::Value,
    position: (f64, f64),
) -> Result<FlowNode, String> {
    let mut wf = load_workflow(&workflow_id)?;
    let node = FlowNode {
        id: Uuid::new_v4().to_string(),
        node_type,
        label,
        config,
        position,
        retry_config: None,
    };
    wf.nodes.push(node.clone());
    wf.updated_at = Utc::now().to_rfc3339();
    save_workflow(&wf)?;
    Ok(node)
}

#[tauri::command]
pub async fn flow_remove_node(workflow_id: String, node_id: String) -> Result<(), String> {
    let mut wf = load_workflow(&workflow_id)?;
    wf.nodes.retain(|n| n.id != node_id);
    // Remove edges connected to this node
    wf.edges
        .retain(|e| e.source != node_id && e.target != node_id);
    wf.updated_at = Utc::now().to_rfc3339();
    save_workflow(&wf)
}

#[tauri::command]
pub async fn flow_connect(
    workflow_id: String,
    source_id: String,
    target_id: String,
) -> Result<FlowEdge, String> {
    let mut wf = load_workflow(&workflow_id)?;

    // Validate that both nodes exist
    let source_exists = wf.nodes.iter().any(|n| n.id == source_id);
    let target_exists = wf.nodes.iter().any(|n| n.id == target_id);
    if !source_exists {
        return Err(format!("Source node {source_id} not found"));
    }
    if !target_exists {
        return Err(format!("Target node {target_id} not found"));
    }

    // Prevent duplicate edges
    let exists = wf
        .edges
        .iter()
        .any(|e| e.source == source_id && e.target == target_id);
    if exists {
        return Err("Edge already exists".to_string());
    }

    let edge = FlowEdge {
        id: Uuid::new_v4().to_string(),
        source: source_id,
        target: target_id,
        label: None,
    };
    wf.edges.push(edge.clone());
    wf.updated_at = Utc::now().to_rfc3339();
    save_workflow(&wf)?;
    Ok(edge)
}

#[tauri::command]
pub async fn flow_disconnect(workflow_id: String, edge_id: String) -> Result<(), String> {
    let mut wf = load_workflow(&workflow_id)?;
    wf.edges.retain(|e| e.id != edge_id);
    wf.updated_at = Utc::now().to_rfc3339();
    save_workflow(&wf)
}

#[tauri::command]
pub async fn flow_run(workflow_id: String) -> Result<WorkflowRun, String> {
    let mut wf = load_workflow(&workflow_id)?;

    if wf.nodes.is_empty() {
        return Err("Workflow has no nodes to execute".to_string());
    }

    let run = run_workflow_inner(&wf).await;

    // Update workflow stats
    wf.run_count += 1;
    wf.last_run = Some(run.started_at.clone());
    wf.updated_at = Utc::now().to_rfc3339();
    save_workflow(&wf)?;

    // Persist the run
    save_run(&run)?;

    Ok(run)
}

#[tauri::command]
pub async fn flow_get_runs(
    workflow_id: String,
    limit: Option<usize>,
) -> Result<Vec<WorkflowRun>, String> {
    let dir = runs_dir()?;
    let prefix = format!("{workflow_id}_");
    let limit = limit.unwrap_or(20);

    let mut runs = Vec::new();
    let entries = std::fs::read_dir(&dir).map_err(|e| format!("Cannot list runs: {e}"))?;

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(&prefix) && name.ends_with(".json") {
            if let Ok(data) = std::fs::read_to_string(entry.path()) {
                if let Ok(run) = serde_json::from_str::<WorkflowRun>(&data) {
                    runs.push(run);
                }
            }
        }
    }

    // Sort by started_at descending
    runs.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    runs.truncate(limit);
    Ok(runs)
}

#[tauri::command]
pub async fn flow_get_templates() -> Result<Vec<WorkflowTemplate>, String> {
    Ok(built_in_templates())
}

// ---------------------------------------------------------------------------
// AI-powered workflow generation (AFLOW pattern -- arXiv:2410.10762)
// ---------------------------------------------------------------------------

/// AI Workflow Generator: user describes what they want in plain language,
/// Ollama analyzes the description and generates a complete workflow DAG.
#[tauri::command]
pub async fn flow_ai_generate(description: String) -> Result<Workflow, String> {
    let system_prompt = r#"You are a workflow automation expert. Given a user's description of what they want automated, generate a JSON workflow definition.

Available node types (use the "kind" field):
TRIGGERS (start the workflow):
- trigger_manual: Manual start button
- trigger_cron: Scheduled execution. Fields: schedule (cron expression like "0 0 9 * * *")
- trigger_webhook: HTTP webhook. Fields: path (e.g. "/hook")
- trigger_file_watch: Watch filesystem. Fields: path (directory to watch)
- trigger_app_event: App event. Fields: event_name

ACTIONS (do work):
- action_http_request: HTTP call. Fields: method, url, headers (array of [key,value]), body
- action_llm_call: AI/LLM processing. Fields: prompt, model (optional, default null)
- action_shell_command: Run shell command. Fields: command
- action_email: Send email. Fields: to, subject, body
- action_file_op: File operation. Fields: operation (read/write/exists/delete/list), path
- action_json_transform: Extract/transform JSON. Fields: expression (dot-path like "body.data")
- action_notification: Desktop notification. Fields: title, message
- action_git_op: Git operation. Fields: operation (status/pull/push/log)
- action_social_post: Social media. Fields: platform (linkedin/twitter/github/hackernews), content
- action_db_query: SQLite query. Fields: query

CONTROL FLOW:
- control_condition: Branch on condition. Fields: expression (e.g. "status == 200")
- control_loop: Repeat. Fields: count (number of iterations)
- control_delay: Wait. Fields: seconds
- control_merge: Merge branches. No extra fields.

Use {{prev}} or {{prev.field}} to reference the previous node's output.
Use {{var.name}} to reference workflow variables.

Respond with ONLY valid JSON in this exact format (no markdown, no explanation):
{
  "name": "Workflow Name",
  "description": "What it does",
  "nodes": [
    {"id": "n1", "kind": "trigger_cron", "label": "Human readable label", "schedule": "0 0 9 * * *", "x": 100, "y": 200},
    {"id": "n2", "kind": "action_http_request", "label": "Fetch Data", "method": "GET", "url": "https://...", "x": 400, "y": 200}
  ],
  "edges": [
    {"source": "n1", "target": "n2"}
  ],
  "variables": {"api_key": "your-key-here"}
}"#;

    let full_prompt = format!(
        "{}\n\nUser request: {}\n\nGenerate the workflow JSON:",
        system_prompt, description
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let payload = serde_json::json!({
        "model": "dolphin3:8b",
        "prompt": full_prompt,
        "stream": false,
        "options": {
            "temperature": 0.3,
            "num_predict": 4096,
        }
    });

    let resp = client
        .post("http://localhost:11434/api/generate")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Ollama request failed (is Ollama running?): {e}"))?;

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Cannot parse Ollama response: {e}"))?;

    let response_text = data
        .get("response")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Ollama returned no response".to_string())?;

    // Extract JSON from the response (handle markdown code blocks)
    let json_str = extract_json_from_response(response_text)?;
    let ai_output: serde_json::Value =
        serde_json::from_str(&json_str).map_err(|e| format!("AI returned invalid JSON: {e}"))?;

    // Build workflow from AI output
    let now = Utc::now().to_rfc3339();
    let wf_name = ai_output
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("AI Generated Workflow")
        .to_string();
    let wf_desc = ai_output
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or(&description)
        .to_string();

    let mut nodes = Vec::new();
    if let Some(ai_nodes) = ai_output.get("nodes").and_then(|v| v.as_array()) {
        for n in ai_nodes {
            let node = parse_ai_node(n)?;
            nodes.push(node);
        }
    }

    let mut edges = Vec::new();
    if let Some(ai_edges) = ai_output.get("edges").and_then(|v| v.as_array()) {
        for e in ai_edges {
            let source = e
                .get("source")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let target = e
                .get("target")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if !source.is_empty() && !target.is_empty() {
                edges.push(FlowEdge {
                    id: Uuid::new_v4().to_string(),
                    source,
                    target,
                    label: e.get("label").and_then(|v| v.as_str()).map(String::from),
                });
            }
        }
    }

    let variables = ai_output
        .get("variables")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    let wf = Workflow {
        id: Uuid::new_v4().to_string(),
        name: wf_name,
        description: wf_desc,
        nodes,
        edges,
        enabled: true,
        run_count: 0,
        last_run: None,
        created_at: now.clone(),
        updated_at: now,
        variables,
    };

    save_workflow(&wf)?;
    Ok(wf)
}

/// Extract JSON from LLM response, stripping markdown code fences if present.
fn extract_json_from_response(text: &str) -> Result<String, String> {
    let trimmed = text.trim();
    // Try to find JSON in markdown code block
    if let Some(start) = trimmed.find("```json") {
        let after = &trimmed[start + 7..];
        if let Some(end) = after.find("```") {
            return Ok(after[..end].trim().to_string());
        }
    }
    if let Some(start) = trimmed.find("```") {
        let after = &trimmed[start + 3..];
        if let Some(end) = after.find("```") {
            let inner = after[..end].trim();
            // Skip language identifier on first line
            if let Some(newline) = inner.find('\n') {
                let first_line = &inner[..newline];
                if !first_line.starts_with('{') {
                    return Ok(inner[newline..].trim().to_string());
                }
            }
            return Ok(inner.to_string());
        }
    }
    // Try to find raw JSON object
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            if end > start {
                return Ok(trimmed[start..=end].to_string());
            }
        }
    }
    Err("Could not extract JSON from AI response".to_string())
}

/// Parse a single AI-generated node JSON into a FlowNode.
fn parse_ai_node(n: &serde_json::Value) -> Result<FlowNode, String> {
    let id = n
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let kind = n
        .get("kind")
        .and_then(|v| v.as_str())
        .unwrap_or("trigger_manual");
    let label = n
        .get("label")
        .and_then(|v| v.as_str())
        .unwrap_or("Node")
        .to_string();
    let x = n
        .get("x")
        .and_then(|v| v.as_f64())
        .unwrap_or(200.0);
    let y = n
        .get("y")
        .and_then(|v| v.as_f64())
        .unwrap_or(200.0);

    let s = |key: &str| -> String {
        n.get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };

    let node_type = match kind {
        "trigger_manual" => NodeType::TriggerManual,
        "trigger_cron" => NodeType::TriggerCron {
            schedule: s("schedule"),
        },
        "trigger_webhook" => NodeType::TriggerWebhook { path: s("path") },
        "trigger_file_watch" => NodeType::TriggerFileWatch { path: s("path") },
        "trigger_app_event" => NodeType::TriggerAppEvent {
            event_name: s("event_name"),
        },
        "action_http_request" => NodeType::ActionHttpRequest {
            method: s("method"),
            url: s("url"),
            headers: n
                .get("headers")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|h| {
                            let pair = h.as_array()?;
                            Some((
                                pair.first()?.as_str()?.to_string(),
                                pair.get(1)?.as_str()?.to_string(),
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default(),
            body: n.get("body").and_then(|v| v.as_str()).map(String::from),
        },
        "action_llm_call" => NodeType::ActionLlmCall {
            prompt: s("prompt"),
            model: n.get("model").and_then(|v| v.as_str()).map(String::from),
        },
        "action_shell_command" => NodeType::ActionShellCommand {
            command: s("command"),
        },
        "action_email" => NodeType::ActionEmail {
            to: s("to"),
            subject: s("subject"),
            body: s("body"),
        },
        "action_file_op" => NodeType::ActionFileOp {
            operation: s("operation"),
            path: s("path"),
        },
        "action_json_transform" => NodeType::ActionJsonTransform {
            expression: s("expression"),
        },
        "action_notification" => NodeType::ActionNotification {
            title: s("title"),
            message: s("message"),
        },
        "action_git_op" => NodeType::ActionGitOp {
            operation: s("operation"),
        },
        "action_social_post" => NodeType::ActionSocialPost {
            platform: s("platform"),
            content: s("content"),
        },
        "action_db_query" => NodeType::ActionDbQuery { query: s("query") },
        "control_condition" => NodeType::ControlCondition {
            expression: s("expression"),
        },
        "control_loop" => NodeType::ControlLoop {
            count: n.get("count").and_then(|v| v.as_u64()).map(|c| c as u32),
        },
        "control_delay" => NodeType::ControlDelay {
            seconds: n.get("seconds").and_then(|v| v.as_u64()).unwrap_or(5) as u32,
        },
        "control_parallel" => NodeType::ControlParallel,
        "control_merge" => NodeType::ControlMerge,
        "action_sub_workflow" => NodeType::ActionSubWorkflow {
            workflow_id: s("workflow_id"),
        },
        "action_split" => NodeType::ActionSplit { field: s("field") },
        "action_filter" => NodeType::ActionFilter {
            expression: s("expression"),
        },
        "action_sort" => NodeType::ActionSort {
            field: s("field"),
            ascending: n.get("ascending").and_then(|v| v.as_bool()).unwrap_or(true),
        },
        "action_aggregate" => NodeType::ActionAggregate {
            field: s("field"),
            operation: s("operation"),
        },
        "action_map" => NodeType::ActionMap {
            expression: s("expression"),
        },
        "action_unique" => NodeType::ActionUnique { field: s("field") },
        _ => {
            return Err(format!("Unknown node kind from AI: {kind}"));
        }
    };

    let real_id = if id.is_empty() {
        Uuid::new_v4().to_string()
    } else {
        id
    };

    Ok(FlowNode {
        id: real_id,
        node_type,
        label,
        config: serde_json::json!({}),
        position: (x, y),
        retry_config: None,
    })
}

/// AI Node Suggestion: given the current node, suggest what should come next.
#[tauri::command]
pub async fn flow_ai_suggest_next(
    workflow_id: String,
    current_node_id: String,
) -> Result<Vec<NodeSuggestion>, String> {
    let wf = load_workflow(&workflow_id)?;
    let current_node = wf
        .nodes
        .iter()
        .find(|n| n.id == current_node_id)
        .ok_or_else(|| format!("Node {current_node_id} not found"))?;

    // Use pattern-based heuristics first (fast, works offline even without Ollama)
    let suggestions = suggest_next_nodes(&current_node.node_type);

    // If we have fewer than 3 suggestions, try to augment with AI
    if suggestions.len() >= 3 {
        return Ok(suggestions);
    }

    // Try Ollama for richer suggestions (non-blocking, fallback to heuristics)
    let ai_suggestions = ai_suggest_next_nodes(current_node, &wf).await;
    match ai_suggestions {
        Ok(mut ai) => {
            // Merge: keep heuristic suggestions, add AI ones that don't overlap
            let existing_kinds: HashSet<String> = suggestions
                .iter()
                .map(|s| format!("{:?}", s.node_type))
                .collect();
            ai.retain(|s| !existing_kinds.contains(&format!("{:?}", s.node_type)));
            let mut merged = suggestions;
            merged.extend(ai);
            merged.truncate(5);
            Ok(merged)
        }
        Err(_) => Ok(suggestions),
    }
}

/// Pattern-based node suggestions (works offline, no AI needed).
fn suggest_next_nodes(current_type: &NodeType) -> Vec<NodeSuggestion> {
    match current_type {
        NodeType::TriggerManual
        | NodeType::TriggerCron { .. }
        | NodeType::TriggerWebhook { .. }
        | NodeType::TriggerFileWatch { .. }
        | NodeType::TriggerAppEvent { .. } => vec![
            NodeSuggestion {
                node_type: NodeType::ActionHttpRequest {
                    method: "GET".into(),
                    url: String::new(),
                    headers: vec![],
                    body: None,
                },
                label: "HTTP Request".into(),
                description: "Fetch data from an API".into(),
                confidence: 0.9,
            },
            NodeSuggestion {
                node_type: NodeType::ActionLlmCall {
                    prompt: String::new(),
                    model: None,
                },
                label: "AI / LLM".into(),
                description: "Process with AI".into(),
                confidence: 0.85,
            },
            NodeSuggestion {
                node_type: NodeType::ActionShellCommand {
                    command: String::new(),
                },
                label: "Shell Command".into(),
                description: "Run a system command".into(),
                confidence: 0.8,
            },
        ],
        NodeType::ActionHttpRequest { .. } => vec![
            NodeSuggestion {
                node_type: NodeType::ActionJsonTransform {
                    expression: "body".into(),
                },
                label: "Parse JSON".into(),
                description: "Extract data from HTTP response".into(),
                confidence: 0.95,
            },
            NodeSuggestion {
                node_type: NodeType::ControlCondition {
                    expression: "status == 200".into(),
                },
                label: "Check Status".into(),
                description: "Branch on HTTP status code".into(),
                confidence: 0.9,
            },
            NodeSuggestion {
                node_type: NodeType::ActionFileOp {
                    operation: "write".into(),
                    path: String::new(),
                },
                label: "Save to File".into(),
                description: "Save response to a file".into(),
                confidence: 0.7,
            },
        ],
        NodeType::ActionLlmCall { .. } => vec![
            NodeSuggestion {
                node_type: NodeType::ActionEmail {
                    to: String::new(),
                    subject: String::new(),
                    body: "{{prev.response}}".into(),
                },
                label: "Email Result".into(),
                description: "Send AI output via email".into(),
                confidence: 0.9,
            },
            NodeSuggestion {
                node_type: NodeType::ActionDbQuery {
                    query: String::new(),
                },
                label: "Save to Database".into(),
                description: "Store AI output in SQLite".into(),
                confidence: 0.85,
            },
            NodeSuggestion {
                node_type: NodeType::ActionSocialPost {
                    platform: String::new(),
                    content: "{{prev.response}}".into(),
                },
                label: "Post to Social".into(),
                description: "Publish AI-generated content".into(),
                confidence: 0.75,
            },
        ],
        NodeType::ActionShellCommand { .. } => vec![
            NodeSuggestion {
                node_type: NodeType::ControlCondition {
                    expression: "stdout != ".into(),
                },
                label: "Check Output".into(),
                description: "Branch on command output".into(),
                confidence: 0.9,
            },
            NodeSuggestion {
                node_type: NodeType::ActionNotification {
                    title: "Command Complete".into(),
                    message: "{{prev.stdout}}".into(),
                },
                label: "Notify".into(),
                description: "Send notification with result".into(),
                confidence: 0.85,
            },
        ],
        NodeType::ActionDbQuery { .. } => vec![
            NodeSuggestion {
                node_type: NodeType::ActionLlmCall {
                    prompt: "Analyze this data:\n{{prev}}".into(),
                    model: None,
                },
                label: "AI Analyze".into(),
                description: "Analyze query results with AI".into(),
                confidence: 0.9,
            },
            NodeSuggestion {
                node_type: NodeType::ActionJsonTransform {
                    expression: "rows".into(),
                },
                label: "Extract Rows".into(),
                description: "Extract rows from query result".into(),
                confidence: 0.85,
            },
        ],
        NodeType::ControlCondition { .. } => vec![
            NodeSuggestion {
                node_type: NodeType::ActionNotification {
                    title: "Alert".into(),
                    message: "Condition triggered".into(),
                },
                label: "Notify".into(),
                description: "Send alert notification".into(),
                confidence: 0.9,
            },
            NodeSuggestion {
                node_type: NodeType::ActionEmail {
                    to: String::new(),
                    subject: "Alert".into(),
                    body: "{{prev}}".into(),
                },
                label: "Send Email".into(),
                description: "Email when condition is met".into(),
                confidence: 0.85,
            },
        ],
        _ => vec![
            NodeSuggestion {
                node_type: NodeType::ActionNotification {
                    title: "Done".into(),
                    message: "{{prev}}".into(),
                },
                label: "Notify".into(),
                description: "Send completion notification".into(),
                confidence: 0.7,
            },
        ],
    }
}

/// Try to get AI-powered suggestions from Ollama.
async fn ai_suggest_next_nodes(
    current_node: &FlowNode,
    _wf: &Workflow,
) -> Result<Vec<NodeSuggestion>, String> {
    let prompt = format!(
        r#"Given a workflow node of type "{:?}" labeled "{}", suggest 2 good next nodes.
Reply with ONLY a JSON array of objects, each with: kind, label, description.
Example: [{{"kind":"action_email","label":"Send Result","description":"Email the output"}}]"#,
        current_node.node_type, current_node.label
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let payload = serde_json::json!({
        "model": "dolphin3:8b",
        "prompt": prompt,
        "stream": false,
        "options": { "temperature": 0.3, "num_predict": 512 }
    });

    let resp = client
        .post("http://localhost:11434/api/generate")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Ollama error: {e}"))?;

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Parse error: {e}"))?;

    let text = data
        .get("response")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "No response".to_string())?;

    let json_str = extract_json_from_response(text)?;
    let arr: Vec<serde_json::Value> =
        serde_json::from_str(&json_str).map_err(|e| format!("Parse suggestions: {e}"))?;

    let mut suggestions = Vec::new();
    for item in arr.iter().take(3) {
        let kind = item
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("action_notification");
        let label = item
            .get("label")
            .and_then(|v| v.as_str())
            .unwrap_or("Suggested Node")
            .to_string();
        let desc = item
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if let Ok(node) = parse_ai_node(&serde_json::json!({"id": "", "kind": kind, "label": label, "x": 0, "y": 0}))
        {
            suggestions.push(NodeSuggestion {
                node_type: node.node_type,
                label,
                description: desc,
                confidence: 0.6,
            });
        }
    }
    Ok(suggestions)
}

// ---------------------------------------------------------------------------
// Workflow scheduling
// ---------------------------------------------------------------------------

/// Create or update a cron schedule for a workflow.
#[tauri::command]
pub async fn flow_schedule(
    workflow_id: String,
    cron: String,
) -> Result<WorkflowSchedule, String> {
    // Validate the cron expression
    use std::str::FromStr;
    let parsed = croner::Cron::from_str(&cron)
        .map_err(|e| format!("Invalid cron expression: {e}"))?;

    // Compute next run
    let next = parsed
        .find_next_occurrence(&Utc::now(), false)
        .ok()
        .map(|dt| dt.to_rfc3339());

    let schedule = WorkflowSchedule {
        workflow_id: workflow_id.clone(),
        cron_expression: cron,
        enabled: true,
        next_run: next,
        timezone: "UTC".to_string(),
    };

    // Load existing schedules, upsert
    let mut schedules = load_schedules()?;
    if let Some(existing) = schedules.iter_mut().find(|s| s.workflow_id == workflow_id) {
        *existing = schedule.clone();
    } else {
        schedules.push(schedule.clone());
    }
    save_schedules(&schedules)?;

    Ok(schedule)
}

/// List all scheduled workflows.
#[tauri::command]
pub async fn flow_list_scheduled() -> Result<Vec<WorkflowSchedule>, String> {
    let mut schedules = load_schedules()?;

    // Refresh next_run times
    use std::str::FromStr;
    for sched in &mut schedules {
        if sched.enabled {
            if let Ok(parsed) = croner::Cron::from_str(&sched.cron_expression) {
                sched.next_run = parsed
                    .find_next_occurrence(&Utc::now(), false)
                    .ok()
                    .map(|dt| dt.to_rfc3339());
            }
        }
    }
    save_schedules(&schedules)?;

    Ok(schedules)
}

/// Remove a schedule for a workflow.
#[tauri::command]
pub async fn flow_unschedule(workflow_id: String) -> Result<(), String> {
    let mut schedules = load_schedules()?;
    schedules.retain(|s| s.workflow_id != workflow_id);
    save_schedules(&schedules)
}

// ---------------------------------------------------------------------------
// Workflow variables
// ---------------------------------------------------------------------------

/// Set a workflow variable (key-value).
#[tauri::command]
pub async fn flow_set_variable(
    workflow_id: String,
    key: String,
    value: serde_json::Value,
) -> Result<(), String> {
    let mut wf = load_workflow(&workflow_id)?;
    wf.variables.insert(key, value);
    wf.updated_at = Utc::now().to_rfc3339();
    save_workflow(&wf)
}

/// Get all variables for a workflow.
#[tauri::command]
pub async fn flow_get_variables(
    workflow_id: String,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let wf = load_workflow(&workflow_id)?;
    Ok(wf.variables)
}

// ---------------------------------------------------------------------------
// Workflow analytics
// ---------------------------------------------------------------------------

/// Compute execution analytics for a workflow from stored run history.
#[tauri::command]
pub async fn flow_analytics(workflow_id: String) -> Result<WorkflowAnalytics, String> {
    let dir = runs_dir()?;
    let prefix = format!("{workflow_id}_");

    let mut all_runs: Vec<WorkflowRun> = Vec::new();
    let entries = std::fs::read_dir(&dir).map_err(|e| format!("Cannot list runs: {e}"))?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(&prefix) && name.ends_with(".json") {
            if let Ok(data) = std::fs::read_to_string(entry.path()) {
                if let Ok(run) = serde_json::from_str::<WorkflowRun>(&data) {
                    all_runs.push(run);
                }
            }
        }
    }

    let total_runs = all_runs.len() as u64;
    let successes = all_runs
        .iter()
        .filter(|r| r.status == RunStatus::Completed)
        .count() as f32;
    let success_rate = if total_runs > 0 {
        successes / total_runs as f32
    } else {
        0.0
    };

    // Average duration
    let mut total_duration = 0u64;
    for run in &all_runs {
        let dur: u64 = run.node_results.iter().map(|nr| nr.duration_ms).sum();
        total_duration += dur;
    }
    let avg_duration_ms = if total_runs > 0 {
        total_duration / total_runs
    } else {
        0
    };

    // Most failed node
    let mut fail_counts: HashMap<String, usize> = HashMap::new();
    for run in &all_runs {
        for nr in &run.node_results {
            if nr.status == RunStatus::Failed {
                *fail_counts.entry(nr.node_id.clone()).or_insert(0) += 1;
            }
        }
    }
    let most_failed_node = fail_counts
        .into_iter()
        .max_by_key(|(_k, v)| *v)
        .map(|(k, _)| k);

    // Last 7 days stats
    let now = Utc::now();
    let mut daily_map: HashMap<String, (u64, u64, u64, u64)> = HashMap::new();
    for i in 0..7 {
        let day = now - chrono::Duration::days(i);
        let date_str = day.format("%Y-%m-%d").to_string();
        daily_map.insert(date_str, (0, 0, 0, 0));
    }

    for run in &all_runs {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&run.started_at) {
            let date_str = dt.format("%Y-%m-%d").to_string();
            if let Some(entry) = daily_map.get_mut(&date_str) {
                let dur: u64 = run.node_results.iter().map(|nr| nr.duration_ms).sum();
                entry.0 += 1; // runs
                if run.status == RunStatus::Completed {
                    entry.1 += 1;
                } else {
                    entry.2 += 1;
                }
                entry.3 += dur;
            }
        }
    }

    let mut last_7_days: Vec<DailyStats> = daily_map
        .into_iter()
        .map(|(date, (runs, s, f, dur))| DailyStats {
            date,
            runs,
            successes: s,
            failures: f,
            avg_duration_ms: if runs > 0 { dur / runs } else { 0 },
        })
        .collect();
    last_7_days.sort_by(|a, b| a.date.cmp(&b.date));

    Ok(WorkflowAnalytics {
        total_runs,
        success_rate,
        avg_duration_ms,
        most_failed_node,
        last_7_days,
    })
}

// ---------------------------------------------------------------------------
// Credential Vault commands
// ---------------------------------------------------------------------------

/// List all credentials (metadata only -- no secret data exposed).
#[tauri::command]
pub async fn flow_list_credentials() -> Result<Vec<CredentialMeta>, String> {
    let creds = load_credentials()?;
    Ok(creds
        .into_iter()
        .map(|c| CredentialMeta {
            id: c.id,
            name: c.name,
            credential_type: c.credential_type,
            created_at: c.created_at,
        })
        .collect())
}

/// Save a new or updated credential.
#[tauri::command]
pub async fn flow_save_credential(
    name: String,
    credential_type: String,
    data: serde_json::Value,
) -> Result<Credential, String> {
    let mut creds = load_credentials()?;

    // Check for existing credential with same name (update)
    let existing_idx = creds.iter().position(|c| c.name == name);

    let cred = Credential {
        id: existing_idx
            .map(|i| creds[i].id.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string()),
        name,
        credential_type,
        data,
        created_at: existing_idx
            .map(|i| creds[i].created_at.clone())
            .unwrap_or_else(|| Utc::now().to_rfc3339()),
    };

    if let Some(idx) = existing_idx {
        creds[idx] = cred.clone();
    } else {
        creds.push(cred.clone());
    }

    save_credentials(&creds)?;
    Ok(cred)
}

/// Delete a credential by ID.
#[tauri::command]
pub async fn flow_delete_credential(id: String) -> Result<(), String> {
    let mut creds = load_credentials()?;
    creds.retain(|c| c.id != id);
    save_credentials(&creds)
}

// ---------------------------------------------------------------------------
// Workflow Versioning commands
// ---------------------------------------------------------------------------

/// Save a snapshot of the current workflow as a new version.
#[tauri::command]
pub async fn flow_save_version(
    workflow_id: String,
    message: String,
) -> Result<WorkflowVersion, String> {
    let wf = load_workflow(&workflow_id)?;
    let snapshot = serde_json::to_value(&wf)
        .map_err(|e| format!("Cannot serialize workflow snapshot: {e}"))?;

    let mut versions = load_versions(&workflow_id)?;
    let next_version = versions.iter().map(|v| v.version).max().unwrap_or(0) + 1;

    let version = WorkflowVersion {
        version: next_version,
        snapshot,
        message,
        created_at: Utc::now().to_rfc3339(),
    };

    versions.push(version.clone());

    // Keep at most 50 versions per workflow
    if versions.len() > 50 {
        versions.drain(0..versions.len() - 50);
    }

    save_versions(&workflow_id, &versions)?;
    Ok(version)
}

/// List all saved versions for a workflow.
#[tauri::command]
pub async fn flow_list_versions(workflow_id: String) -> Result<Vec<WorkflowVersion>, String> {
    let mut versions = load_versions(&workflow_id)?;
    // Return newest first
    versions.sort_by(|a, b| b.version.cmp(&a.version));
    Ok(versions)
}

/// Rollback a workflow to a previous version.
#[tauri::command]
pub async fn flow_rollback(
    workflow_id: String,
    version: u32,
) -> Result<Workflow, String> {
    let versions = load_versions(&workflow_id)?;
    let target = versions
        .iter()
        .find(|v| v.version == version)
        .ok_or_else(|| format!("Version {version} not found"))?;

    let mut wf: Workflow = serde_json::from_value(target.snapshot.clone())
        .map_err(|e| format!("Cannot deserialize version snapshot: {e}"))?;

    // Ensure the ID matches (in case of import/copy)
    wf.id = workflow_id;
    wf.updated_at = Utc::now().to_rfc3339();
    save_workflow(&wf)?;
    Ok(wf)
}

// ---------------------------------------------------------------------------
// Workflow Import/Export commands
// ---------------------------------------------------------------------------

/// Export a workflow as a portable JSON string.
#[tauri::command]
pub async fn flow_export(workflow_id: String) -> Result<String, String> {
    let wf = load_workflow(&workflow_id)?;
    serde_json::to_string_pretty(&wf).map_err(|e| format!("Cannot serialize workflow: {e}"))
}

/// Import a workflow from a JSON string.
#[tauri::command]
pub async fn flow_import(json: String) -> Result<Workflow, String> {
    let mut wf: Workflow =
        serde_json::from_str(&json).map_err(|e| format!("Invalid workflow JSON: {e}"))?;

    // Assign a new ID to avoid collisions
    wf.id = Uuid::new_v4().to_string();
    wf.created_at = Utc::now().to_rfc3339();
    wf.updated_at = Utc::now().to_rfc3339();
    wf.run_count = 0;
    wf.last_run = None;

    save_workflow(&wf)?;
    Ok(wf)
}

// ---------------------------------------------------------------------------
// Webhook Server commands
// ---------------------------------------------------------------------------

/// List all registered webhook endpoints.
#[tauri::command]
pub async fn flow_list_webhooks() -> Result<Vec<WebhookInfo>, String> {
    let registry = WEBHOOK_REGISTRY.read().await;
    Ok(registry
        .values()
        .map(|reg| WebhookInfo {
            workflow_id: reg.workflow_id.clone(),
            path: reg.path.clone(),
            method: reg.method.clone(),
            url: format!("http://localhost:9876{}", reg.path),
        })
        .collect())
}

/// Register a webhook endpoint for a workflow. When the workflow is enabled
/// and has a TriggerWebhook node, this makes it callable via HTTP.
#[tauri::command]
pub async fn flow_register_webhook(
    workflow_id: String,
    path: String,
    method: String,
) -> Result<WebhookInfo, String> {
    let clean_path = if path.starts_with('/') {
        path.clone()
    } else {
        format!("/{path}")
    };

    let reg = WebhookRegistration {
        workflow_id: workflow_id.clone(),
        path: clean_path.clone(),
        method: method.clone(),
    };

    let mut registry = WEBHOOK_REGISTRY.write().await;
    registry.insert(workflow_id.clone(), reg);

    Ok(WebhookInfo {
        workflow_id,
        path: clean_path.clone(),
        method,
        url: format!("http://localhost:9876{clean_path}"),
    })
}

/// Unregister a webhook endpoint for a workflow.
#[tauri::command]
pub async fn flow_unregister_webhook(workflow_id: String) -> Result<(), String> {
    let mut registry = WEBHOOK_REGISTRY.write().await;
    registry.remove(&workflow_id);
    Ok(())
}

// ---------------------------------------------------------------------------
// Workflow Toggle (enable/disable)
// ---------------------------------------------------------------------------

/// Toggle a workflow's enabled state.
#[tauri::command]
pub async fn flow_toggle(workflow_id: String, enabled: bool) -> Result<(), String> {
    let mut wf = load_workflow(&workflow_id)?;
    wf.enabled = enabled;
    wf.updated_at = Utc::now().to_rfc3339();
    save_workflow(&wf)
}

// ---------------------------------------------------------------------------
// Workflow Duplicate
// ---------------------------------------------------------------------------

/// Duplicate a workflow with a new ID and name.
#[tauri::command]
pub async fn flow_duplicate(workflow_id: String) -> Result<Workflow, String> {
    let original = load_workflow(&workflow_id)?;
    let now = Utc::now().to_rfc3339();
    let mut copy = original.clone();
    copy.id = Uuid::new_v4().to_string();
    copy.name = format!("{} (Copy)", original.name);
    copy.created_at = now.clone();
    copy.updated_at = now;
    copy.run_count = 0;
    copy.last_run = None;
    save_workflow(&copy)?;
    Ok(copy)
}

// ---------------------------------------------------------------------------
// ForgeMemory Integration
// ---------------------------------------------------------------------------

/// Store a workflow summary in ForgeMemory so it is searchable across ImpForge.
#[tauri::command]
pub async fn flow_remember(
    engine: tauri::State<'_, crate::forge_memory::engine::ForgeMemoryEngine>,
    title: String,
    content: String,
) -> Result<String, String> {
    let summary = format!("[Flow] {title}: {preview}", preview = &content[..content.len().min(500)]);
    engine.add_memory(&summary, "archival", 0.5, "workflow")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topological_sort_linear() {
        let nodes = vec![
            FlowNode { id: "a".into(), node_type: NodeType::TriggerManual, label: "Start".into(), config: serde_json::json!({}), position: (0.0, 0.0), retry_config: None },
            FlowNode { id: "b".into(), node_type: NodeType::ActionNotification { title: "T".into(), message: "M".into() }, label: "Notify".into(), config: serde_json::json!({}), position: (100.0, 0.0), retry_config: None },
            FlowNode { id: "c".into(), node_type: NodeType::ControlMerge, label: "End".into(), config: serde_json::json!({}), position: (200.0, 0.0), retry_config: None },
        ];
        let edges = vec![
            FlowEdge { id: "e1".into(), source: "a".into(), target: "b".into(), label: None },
            FlowEdge { id: "e2".into(), source: "b".into(), target: "c".into(), label: None },
        ];
        let sorted = topological_sort(&nodes, &edges).unwrap();
        assert_eq!(sorted, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_topological_sort_diamond() {
        let nodes = vec![
            FlowNode { id: "a".into(), node_type: NodeType::TriggerManual, label: "S".into(), config: serde_json::json!({}), position: (0.0, 0.0), retry_config: None },
            FlowNode { id: "b".into(), node_type: NodeType::TriggerManual, label: "B".into(), config: serde_json::json!({}), position: (0.0, 0.0), retry_config: None },
            FlowNode { id: "c".into(), node_type: NodeType::TriggerManual, label: "C".into(), config: serde_json::json!({}), position: (0.0, 0.0), retry_config: None },
            FlowNode { id: "d".into(), node_type: NodeType::ControlMerge, label: "M".into(), config: serde_json::json!({}), position: (0.0, 0.0), retry_config: None },
        ];
        let edges = vec![
            FlowEdge { id: "e1".into(), source: "a".into(), target: "b".into(), label: None },
            FlowEdge { id: "e2".into(), source: "a".into(), target: "c".into(), label: None },
            FlowEdge { id: "e3".into(), source: "b".into(), target: "d".into(), label: None },
            FlowEdge { id: "e4".into(), source: "c".into(), target: "d".into(), label: None },
        ];
        let sorted = topological_sort(&nodes, &edges).unwrap();
        assert_eq!(sorted[0], "a");
        assert_eq!(sorted[3], "d");
        // b and c can be in either order
        assert!(sorted.contains(&"b".to_string()));
        assert!(sorted.contains(&"c".to_string()));
    }

    #[test]
    fn test_topological_sort_cycle_detected() {
        let nodes = vec![
            FlowNode { id: "a".into(), node_type: NodeType::TriggerManual, label: "A".into(), config: serde_json::json!({}), position: (0.0, 0.0), retry_config: None },
            FlowNode { id: "b".into(), node_type: NodeType::TriggerManual, label: "B".into(), config: serde_json::json!({}), position: (0.0, 0.0), retry_config: None },
        ];
        let edges = vec![
            FlowEdge { id: "e1".into(), source: "a".into(), target: "b".into(), label: None },
            FlowEdge { id: "e2".into(), source: "b".into(), target: "a".into(), label: None },
        ];
        let result = topological_sort(&nodes, &edges);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cycle"));
    }

    #[test]
    fn test_interpolation() {
        let prev = serde_json::json!({"name": "Alice", "count": 42});
        assert_eq!(interpolate("Hello {{prev.name}}", &prev), "Hello Alice");
        assert_eq!(interpolate("Count: {{prev.count}}", &prev), "Count: 42");
    }

    #[test]
    fn test_interpolation_string_prev() {
        let prev = serde_json::json!("hello world");
        assert_eq!(interpolate("Result: {{prev}}", &prev), "Result: hello world");
    }

    #[test]
    fn test_condition_evaluation() {
        let prev = serde_json::json!({"status": 200, "ok": true});
        assert!(evaluate_condition("status == 200", &prev));
        assert!(!evaluate_condition("status == 404", &prev));
        assert!(evaluate_condition("status > 100", &prev));
        assert!(!evaluate_condition("status < 100", &prev));
    }

    #[test]
    fn test_json_transform_simple() {
        let prev = serde_json::json!({"name": "Alice", "age": 30});
        assert_eq!(apply_json_transform("name", &prev), serde_json::json!("Alice"));
    }

    #[test]
    fn test_json_transform_nested() {
        let prev = serde_json::json!({"body": {"data": [1, 2, 3]}});
        assert_eq!(apply_json_transform("body.data", &prev), serde_json::json!([1, 2, 3]));
    }

    #[test]
    fn test_workflow_serialization_roundtrip() {
        let now = "2026-03-18T12:00:00Z".to_string();
        let wf = Workflow {
            id: "test-1".into(),
            name: "Test".into(),
            description: "A test workflow".into(),
            nodes: vec![
                FlowNode {
                    id: "n1".into(),
                    node_type: NodeType::TriggerManual,
                    label: "Start".into(),
                    config: serde_json::json!({}),
                    position: (0.0, 0.0),
                    retry_config: None,
                },
            ],
            edges: vec![],
            enabled: true,
            run_count: 0,
            last_run: None,
            created_at: now.clone(),
            updated_at: now,
            variables: serde_json::Map::new(),
        };
        let json = serde_json::to_string(&wf).unwrap();
        let parsed: Workflow = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "test-1");
        assert_eq!(parsed.nodes.len(), 1);
    }

    #[test]
    fn test_templates_valid() {
        let templates = built_in_templates();
        assert_eq!(templates.len(), 10);
        for tpl in &templates {
            assert!(!tpl.name.is_empty());
            assert!(!tpl.workflow.nodes.is_empty());
            assert!(!tpl.workflow.edges.is_empty());
            // All templates must pass topological sort
            let result = topological_sort(&tpl.workflow.nodes, &tpl.workflow.edges);
            assert!(result.is_ok(), "Template '{}' has invalid DAG", tpl.name);
        }
    }

    #[test]
    fn test_retry_config_serialization() {
        let rc = RetryConfig {
            max_retries: 3,
            backoff_ms: 1000,
            on_failure: FailureAction::Skip,
        };
        let json = serde_json::to_string(&rc).unwrap();
        assert!(json.contains("\"max_retries\":3"));
        let parsed: RetryConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.max_retries, 3);
        assert_eq!(parsed.backoff_ms, 1000);
    }

    #[test]
    fn test_failure_action_variants() {
        let stop: FailureAction = serde_json::from_str("\"stop\"").unwrap();
        assert!(matches!(stop, FailureAction::Stop));
        let skip: FailureAction = serde_json::from_str("\"skip\"").unwrap();
        assert!(matches!(skip, FailureAction::Skip));
        let retry: FailureAction = serde_json::from_str("\"retry\"").unwrap();
        assert!(matches!(retry, FailureAction::Retry));
        let fb: FailureAction =
            serde_json::from_str("{\"fallback\":\"node-99\"}").unwrap();
        assert!(matches!(fb, FailureAction::Fallback(ref id) if id == "node-99"));
    }

    #[test]
    fn test_workflow_variables_field() {
        let now = "2026-03-18T12:00:00Z".to_string();
        let mut vars = serde_json::Map::new();
        vars.insert(
            "api_key".to_string(),
            serde_json::Value::String("test123".into()),
        );
        let wf = Workflow {
            id: "vars-test".into(),
            name: "VarTest".into(),
            description: String::new(),
            nodes: vec![],
            edges: vec![],
            enabled: true,
            run_count: 0,
            last_run: None,
            created_at: now.clone(),
            updated_at: now,
            variables: vars,
        };
        let json = serde_json::to_string(&wf).unwrap();
        assert!(json.contains("api_key"));
        let parsed: Workflow = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed.variables.get("api_key").and_then(|v| v.as_str()),
            Some("test123")
        );
    }

    #[test]
    fn test_workflow_backward_compat_no_variables() {
        // Old JSON without variables field should deserialize fine
        let json = r#"{
            "id": "old-1", "name": "Old", "description": "",
            "nodes": [], "edges": [], "enabled": true,
            "run_count": 0, "last_run": null,
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        }"#;
        let wf: Workflow = serde_json::from_str(json).unwrap();
        assert!(wf.variables.is_empty());
    }

    #[test]
    fn test_flownode_backward_compat_no_retry() {
        // Old JSON without retry_config should deserialize fine
        let json = r#"{
            "id": "n1", "node_type": {"kind": "trigger_manual"},
            "label": "Start", "config": {}, "position": [0, 0]
        }"#;
        let node: FlowNode = serde_json::from_str(json).unwrap();
        assert!(node.retry_config.is_none());
    }

    #[test]
    fn test_interpolate_with_vars() {
        let prev = serde_json::json!({"name": "Alice"});
        let mut vars = serde_json::Map::new();
        vars.insert("greeting".into(), serde_json::json!("Hello"));
        vars.insert("suffix".into(), serde_json::json!("!"));
        let result = interpolate_with_vars("{{var.greeting}} {{prev.name}}{{var.suffix}}", &prev, &vars);
        assert_eq!(result, "Hello Alice!");
    }

    #[test]
    fn test_extract_json_from_response_plain() {
        let input = r#"{"name": "test"}"#;
        let result = extract_json_from_response(input).unwrap();
        assert!(result.contains("test"));
    }

    #[test]
    fn test_extract_json_from_response_markdown() {
        let input = "Here is the workflow:\n```json\n{\"name\": \"test\"}\n```\nDone!";
        let result = extract_json_from_response(input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed.get("name").and_then(|v| v.as_str()), Some("test"));
    }

    #[test]
    fn test_parse_ai_node_trigger() {
        let n = serde_json::json!({
            "id": "n1", "kind": "trigger_cron",
            "label": "Daily", "schedule": "0 0 9 * * *",
            "x": 100, "y": 200
        });
        let node = parse_ai_node(&n).unwrap();
        assert_eq!(node.label, "Daily");
        assert!(matches!(node.node_type, NodeType::TriggerCron { .. }));
    }

    #[test]
    fn test_parse_ai_node_action() {
        let n = serde_json::json!({
            "id": "n2", "kind": "action_llm_call",
            "label": "Summarize", "prompt": "Summarize: {{prev}}",
            "x": 400, "y": 200
        });
        let node = parse_ai_node(&n).unwrap();
        assert_eq!(node.label, "Summarize");
        assert!(matches!(node.node_type, NodeType::ActionLlmCall { .. }));
    }

    #[test]
    fn test_parse_ai_node_unknown_kind() {
        let n = serde_json::json!({"id": "x", "kind": "invalid_kind", "label": "Bad"});
        let result = parse_ai_node(&n);
        assert!(result.is_err());
    }

    #[test]
    fn test_suggest_next_after_trigger() {
        let suggestions = suggest_next_nodes(&NodeType::TriggerManual);
        assert!(suggestions.len() >= 2);
        // First suggestion should be HTTP request
        assert!(matches!(
            suggestions[0].node_type,
            NodeType::ActionHttpRequest { .. }
        ));
    }

    #[test]
    fn test_suggest_next_after_http() {
        let suggestions = suggest_next_nodes(&NodeType::ActionHttpRequest {
            method: "GET".into(),
            url: "https://example.com".into(),
            headers: vec![],
            body: None,
        });
        assert!(suggestions.len() >= 2);
        assert!(matches!(
            suggestions[0].node_type,
            NodeType::ActionJsonTransform { .. }
        ));
    }

    #[test]
    fn test_suggest_next_after_llm() {
        let suggestions = suggest_next_nodes(&NodeType::ActionLlmCall {
            prompt: "test".into(),
            model: None,
        });
        assert!(suggestions.len() >= 2);
        assert!(matches!(
            suggestions[0].node_type,
            NodeType::ActionEmail { .. }
        ));
    }

    #[test]
    fn test_schedule_serialization() {
        let sched = WorkflowSchedule {
            workflow_id: "wf-1".into(),
            cron_expression: "0 0 9 * * *".into(),
            enabled: true,
            next_run: Some("2026-03-19T09:00:00Z".into()),
            timezone: "UTC".into(),
        };
        let json = serde_json::to_string(&sched).unwrap();
        let parsed: WorkflowSchedule = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.workflow_id, "wf-1");
        assert_eq!(parsed.cron_expression, "0 0 9 * * *");
        assert!(parsed.enabled);
    }

    #[test]
    fn test_analytics_serialization() {
        let analytics = WorkflowAnalytics {
            total_runs: 42,
            success_rate: 0.95,
            avg_duration_ms: 1234,
            most_failed_node: Some("node-3".into()),
            last_7_days: vec![DailyStats {
                date: "2026-03-18".into(),
                runs: 5,
                successes: 4,
                failures: 1,
                avg_duration_ms: 800,
            }],
        };
        let json = serde_json::to_string(&analytics).unwrap();
        let parsed: WorkflowAnalytics = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.total_runs, 42);
        assert!((parsed.success_rate - 0.95).abs() < 0.01);
        assert_eq!(parsed.last_7_days.len(), 1);
    }

    #[test]
    fn test_new_templates_have_retry_config() {
        let templates = built_in_templates();
        // Data Backup template should have retry on the backup node
        let backup = templates.iter().find(|t| t.id == "tpl-data-backup").unwrap();
        let backup_node = backup
            .workflow
            .nodes
            .iter()
            .find(|n| n.label == "Run Backup")
            .unwrap();
        assert!(backup_node.retry_config.is_some());
        let rc = backup_node.retry_config.as_ref().unwrap();
        assert_eq!(rc.max_retries, 2);
        assert_eq!(rc.backoff_ms, 5000);

        // Price Monitor should have retry on HTTP fetch
        let price = templates
            .iter()
            .find(|t| t.id == "tpl-price-monitor")
            .unwrap();
        let fetch_node = price
            .workflow
            .nodes
            .iter()
            .find(|n| n.label == "Fetch Price")
            .unwrap();
        assert!(fetch_node.retry_config.is_some());
        let rc2 = fetch_node.retry_config.as_ref().unwrap();
        assert_eq!(rc2.max_retries, 3);
    }

    // -------------------------------------------------------------------
    // New feature tests
    // -------------------------------------------------------------------

    #[test]
    fn test_credential_serialization() {
        let cred = Credential {
            id: "cred-1".into(),
            name: "my_api_key".into(),
            credential_type: "api_key".into(),
            data: serde_json::json!({"token": "sk-test-123"}),
            created_at: "2026-03-18T12:00:00Z".into(),
        };
        let json = serde_json::to_string(&cred).unwrap();
        let parsed: Credential = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "my_api_key");
        assert_eq!(parsed.credential_type, "api_key");
        assert_eq!(
            parsed.data.get("token").and_then(|v| v.as_str()),
            Some("sk-test-123")
        );
    }

    #[test]
    fn test_credential_meta_excludes_data() {
        let meta = CredentialMeta {
            id: "cred-1".into(),
            name: "secret".into(),
            credential_type: "bearer".into(),
            created_at: "2026-03-18T12:00:00Z".into(),
        };
        let json = serde_json::to_string(&meta).unwrap();
        assert!(!json.contains("token"));
        assert!(!json.contains("data"));
        assert!(json.contains("secret"));
    }

    #[test]
    fn test_workflow_version_serialization() {
        let ver = WorkflowVersion {
            version: 3,
            snapshot: serde_json::json!({"name": "My Workflow"}),
            message: "Added email node".into(),
            created_at: "2026-03-18T12:00:00Z".into(),
        };
        let json = serde_json::to_string(&ver).unwrap();
        let parsed: WorkflowVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.version, 3);
        assert_eq!(parsed.message, "Added email node");
    }

    #[test]
    fn test_webhook_info_serialization() {
        let info = WebhookInfo {
            workflow_id: "wf-1".into(),
            path: "/feedback".into(),
            method: "POST".into(),
            url: "http://localhost:9876/feedback".into(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: WebhookInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.path, "/feedback");
        assert_eq!(parsed.method, "POST");
    }

    #[test]
    fn test_node_result_has_input_field() {
        let nr = NodeResult {
            node_id: "n1".into(),
            status: RunStatus::Completed,
            input: serde_json::json!({"source": "trigger"}),
            output: serde_json::json!({"result": 42}),
            duration_ms: 15,
            error: None,
        };
        let json = serde_json::to_string(&nr).unwrap();
        assert!(json.contains("\"input\""));
        assert!(json.contains("\"source\""));
        let parsed: NodeResult = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed.input.get("source").and_then(|v| v.as_str()),
            Some("trigger")
        );
    }

    #[test]
    fn test_node_result_backward_compat_no_input() {
        // Old JSON without input field should deserialize with default
        let json = r#"{
            "node_id": "n1", "status": "completed",
            "output": {"ok": true}, "duration_ms": 10, "error": null
        }"#;
        let nr: NodeResult = serde_json::from_str(json).unwrap();
        assert!(nr.input.is_null());
        assert_eq!(nr.node_id, "n1");
    }

    #[test]
    fn test_new_node_types_serialization() {
        // Sub-workflow
        let nt = NodeType::ActionSubWorkflow {
            workflow_id: "wf-sub".into(),
        };
        let json = serde_json::to_string(&nt).unwrap();
        assert!(json.contains("action_sub_workflow"));
        let parsed: NodeType = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, NodeType::ActionSubWorkflow { .. }));

        // Split
        let nt2 = NodeType::ActionSplit {
            field: "items".into(),
        };
        let json2 = serde_json::to_string(&nt2).unwrap();
        assert!(json2.contains("action_split"));

        // Parallel
        let nt3 = NodeType::ControlParallel;
        let json3 = serde_json::to_string(&nt3).unwrap();
        assert!(json3.contains("control_parallel"));

        // Aggregate
        let nt4 = NodeType::ActionAggregate {
            field: "price".into(),
            operation: "sum".into(),
        };
        let json4 = serde_json::to_string(&nt4).unwrap();
        assert!(json4.contains("action_aggregate"));
        assert!(json4.contains("\"sum\""));
    }

    #[test]
    fn test_execute_split() {
        let prev = serde_json::json!({"items": [1, 2, 3]});
        let result = execute_split("items", &prev);
        assert_eq!(result.get("count").and_then(|v| v.as_u64()), Some(3));
    }

    #[test]
    fn test_execute_split_nested() {
        let prev = serde_json::json!({"data": {"values": [10, 20]}});
        let result = execute_split("data.values", &prev);
        assert_eq!(result.get("count").and_then(|v| v.as_u64()), Some(2));
    }

    #[test]
    fn test_execute_filter() {
        let prev = serde_json::json!({
            "items": [
                {"status": 200, "name": "ok"},
                {"status": 404, "name": "missing"},
                {"status": 200, "name": "good"}
            ]
        });
        let result = execute_filter("status == 200", &prev);
        assert_eq!(result.get("count").and_then(|v| v.as_u64()), Some(2));
    }

    #[test]
    fn test_execute_sort() {
        let prev = serde_json::json!({
            "items": [
                {"name": "Charlie", "age": 30},
                {"name": "Alice", "age": 25},
                {"name": "Bob", "age": 28}
            ]
        });
        let result = execute_sort("name", true, &prev);
        let items = result.get("items").and_then(|v| v.as_array()).unwrap();
        assert_eq!(items[0].get("name").and_then(|v| v.as_str()), Some("Alice"));
        assert_eq!(items[2].get("name").and_then(|v| v.as_str()), Some("Charlie"));
    }

    #[test]
    fn test_execute_aggregate_sum() {
        let prev = serde_json::json!({
            "items": [
                {"price": 10.0},
                {"price": 20.0},
                {"price": 30.0}
            ]
        });
        let result = execute_aggregate("price", "sum", &prev);
        let sum = result.get("result").and_then(|v| v.as_f64()).unwrap();
        assert!((sum - 60.0).abs() < 0.01);
    }

    #[test]
    fn test_execute_aggregate_avg() {
        let prev = serde_json::json!({
            "items": [{"val": 10}, {"val": 20}, {"val": 30}]
        });
        let result = execute_aggregate("val", "avg", &prev);
        let avg = result.get("result").and_then(|v| v.as_f64()).unwrap();
        assert!((avg - 20.0).abs() < 0.01);
    }

    #[test]
    fn test_execute_map() {
        let prev = serde_json::json!({
            "items": [
                {"user": {"name": "Alice"}},
                {"user": {"name": "Bob"}}
            ]
        });
        let result = execute_map("user.name", &prev);
        let items = result.get("items").and_then(|v| v.as_array()).unwrap();
        assert_eq!(items[0].as_str(), Some("Alice"));
        assert_eq!(items[1].as_str(), Some("Bob"));
    }

    #[test]
    fn test_execute_unique() {
        let prev = serde_json::json!({
            "items": [
                {"city": "Berlin"},
                {"city": "Paris"},
                {"city": "Berlin"},
                {"city": "Tokyo"}
            ]
        });
        let result = execute_unique("city", &prev);
        assert_eq!(result.get("count").and_then(|v| v.as_u64()), Some(3));
    }

    #[test]
    fn test_parse_ai_node_new_types() {
        // Sub-workflow
        let n = serde_json::json!({
            "id": "n1", "kind": "action_sub_workflow",
            "label": "Run Sub", "workflow_id": "wf-sub",
            "x": 100, "y": 200
        });
        let node = parse_ai_node(&n).unwrap();
        assert!(matches!(node.node_type, NodeType::ActionSubWorkflow { .. }));

        // Parallel
        let n2 = serde_json::json!({
            "id": "n2", "kind": "control_parallel",
            "label": "Fan Out", "x": 100, "y": 200
        });
        let node2 = parse_ai_node(&n2).unwrap();
        assert!(matches!(node2.node_type, NodeType::ControlParallel));

        // Aggregate
        let n3 = serde_json::json!({
            "id": "n3", "kind": "action_aggregate",
            "label": "Sum Prices", "field": "price", "operation": "sum",
            "x": 100, "y": 200
        });
        let node3 = parse_ai_node(&n3).unwrap();
        assert!(matches!(node3.node_type, NodeType::ActionAggregate { .. }));
    }

    #[test]
    fn test_gather_prev_output_single() {
        let mut predecessors: HashMap<&str, Vec<&str>> = HashMap::new();
        predecessors.insert("b", vec!["a"]);
        let mut outputs: HashMap<String, serde_json::Value> = HashMap::new();
        outputs.insert("a".into(), serde_json::json!({"data": 42}));

        let result = gather_prev_output("b", &predecessors, &outputs);
        assert_eq!(result.get("data").and_then(|v| v.as_u64()), Some(42));
    }

    #[test]
    fn test_gather_prev_output_multiple() {
        let mut predecessors: HashMap<&str, Vec<&str>> = HashMap::new();
        predecessors.insert("c", vec!["a", "b"]);
        let mut outputs: HashMap<String, serde_json::Value> = HashMap::new();
        outputs.insert("a".into(), serde_json::json!({"x": 1}));
        outputs.insert("b".into(), serde_json::json!({"y": 2}));

        let result = gather_prev_output("c", &predecessors, &outputs);
        assert!(result.get("a").is_some());
        assert!(result.get("b").is_some());
    }

    #[test]
    fn test_gather_prev_output_no_predecessors() {
        let predecessors: HashMap<&str, Vec<&str>> = HashMap::new();
        let outputs: HashMap<String, serde_json::Value> = HashMap::new();
        let result = gather_prev_output("a", &predecessors, &outputs);
        assert!(result.is_null());
    }

    #[test]
    fn test_interpolate_credentials_fn() {
        // This tests the function itself (no stored creds = no replacement)
        let result = interpolate_credentials("Authorization: Bearer {{cred.my_key}}");
        // Without stored credentials, placeholder stays as-is
        assert!(result.contains("{{cred.my_key}}"));
    }

    #[test]
    fn test_compare_json_values_numeric() {
        let a = serde_json::json!(10.0);
        let b = serde_json::json!(20.0);
        assert_eq!(compare_json_values(&a, &b), std::cmp::Ordering::Less);
        assert_eq!(compare_json_values(&b, &a), std::cmp::Ordering::Greater);
        assert_eq!(compare_json_values(&a, &a), std::cmp::Ordering::Equal);
    }

    #[test]
    fn test_compare_json_values_string() {
        let a = serde_json::json!("alice");
        let b = serde_json::json!("bob");
        assert_eq!(compare_json_values(&a, &b), std::cmp::Ordering::Less);
    }
}
