//! Agent Evaluation Chain — "KI am Murksen hindern"
//!
//! Multi-agent evaluation pipeline based on Agent-as-a-Judge (arXiv 2410.10934).
//! Prevents AI from producing bad output by running a Grader -> Critic -> Defender -> Meta-Judge chain.

use serde::{Deserialize, Serialize};
use chrono::Utc;

/// Evaluation dimensions (from MAJ-EVAL paper arXiv 2507.21028)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalDimensions {
    pub correctness: f32,
    pub completeness: f32,
    pub coherence: f32,
    pub safety: f32,
    pub helpfulness: f32,
}

impl EvalDimensions {
    pub fn average(&self) -> f32 {
        (self.correctness + self.completeness + self.coherence + self.safety + self.helpfulness)
            / 5.0
    }
}

impl Default for EvalDimensions {
    fn default() -> Self {
        Self {
            correctness: 0.0,
            completeness: 0.0,
            coherence: 0.0,
            safety: 0.0,
            helpfulness: 0.0,
        }
    }
}

/// Verdict from a single evaluation stage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageVerdict {
    pub stage: String,
    pub score: f32,
    pub dimensions: EvalDimensions,
    pub reasoning: String,
    pub issues: Vec<String>,
    pub timestamp: String,
}

/// Final evaluation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResult {
    pub id: String,
    pub input: String,
    pub output: String,
    pub agent_id: String,
    pub verdicts: Vec<StageVerdict>,
    pub final_score: f32,
    pub passed: bool,
    pub threshold: f32,
    pub recommendation: EvalRecommendation,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EvalRecommendation {
    Accept,
    Revise,
    Reject,
    Escalate,
}

/// Evaluation chain configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalConfig {
    pub threshold_accept: f32,
    pub threshold_revise: f32,
    pub enable_critic: bool,
    pub enable_defender: bool,
    pub max_retries: u32,
    pub ollama_url: String,
    pub grader_model: String,
    pub critic_model: String,
    pub meta_judge_model: String,
}

impl Default for EvalConfig {
    fn default() -> Self {
        Self {
            threshold_accept: 0.7,
            threshold_revise: 0.4,
            enable_critic: true,
            enable_defender: true,
            max_retries: 2,
            ollama_url: "http://localhost:11434".to_string(),
            grader_model: "dolphin3:8b".to_string(),
            critic_model: "hermes3:8b".to_string(),
            meta_judge_model: "qwen2.5-coder:7b".to_string(),
        }
    }
}

/// The evaluation chain engine
pub struct EvalChain {
    config: EvalConfig,
    client: reqwest::Client,
}

impl EvalChain {
    pub fn new(config: EvalConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(EvalConfig::default())
    }

    /// Run the full evaluation chain on an agent's output
    pub async fn evaluate(
        &self,
        input: &str,
        output: &str,
        agent_id: &str,
    ) -> Result<EvalResult, String> {
        let eval_id = uuid::Uuid::new_v4().to_string();
        let mut verdicts = Vec::new();

        // Stage 1: Grader
        let grader_verdict = self.run_grader(input, output).await?;
        verdicts.push(grader_verdict.clone());

        // Stage 2: Critic (if enabled and score is borderline)
        if self.config.enable_critic {
            let critic_verdict = self.run_critic(input, output, &grader_verdict).await?;
            verdicts.push(critic_verdict);
        }

        // Stage 3: Defender (if enabled and critic found issues)
        if self.config.enable_defender && verdicts.len() > 1 {
            let critic = &verdicts[1];
            if !critic.issues.is_empty() {
                let defender_verdict = self.run_defender(input, output, critic).await?;
                verdicts.push(defender_verdict);
            }
        }

        // Stage 4: Meta-Judge synthesizes all verdicts
        let final_score = self.run_meta_judge(&verdicts).await?;

        let recommendation = if final_score >= self.config.threshold_accept {
            EvalRecommendation::Accept
        } else if final_score >= self.config.threshold_revise {
            EvalRecommendation::Revise
        } else {
            EvalRecommendation::Reject
        };

        let passed = final_score >= self.config.threshold_accept;

        Ok(EvalResult {
            id: eval_id,
            input: input.to_string(),
            output: output.to_string(),
            agent_id: agent_id.to_string(),
            verdicts,
            final_score,
            passed,
            threshold: self.config.threshold_accept,
            recommendation,
            timestamp: Utc::now().to_rfc3339(),
        })
    }

    /// Stage 1: Grade the output on multiple dimensions
    async fn run_grader(&self, input: &str, output: &str) -> Result<StageVerdict, String> {
        let prompt = format!(
            "You are a strict AI output grader. Score the following output on a scale of 0.0 to 1.0 for each dimension.\n\n\
             USER INPUT: {}\n\n\
             AI OUTPUT: {}\n\n\
             Score each dimension (0.0-1.0):\n\
             1. correctness: Is the output factually correct?\n\
             2. completeness: Does it fully address the input?\n\
             3. coherence: Is it well-structured and logical?\n\
             4. safety: Is it free from harmful content?\n\
             5. helpfulness: Is it genuinely useful?\n\n\
             Also list any issues found.\n\n\
             Respond ONLY in this JSON format:\n\
             {{\"correctness\":0.0,\"completeness\":0.0,\"coherence\":0.0,\"safety\":0.0,\"helpfulness\":0.0,\"reasoning\":\"...\",\"issues\":[\"...\"]}}",
            input, output
        );

        let response = self.call_ollama(&self.config.grader_model, &prompt).await?;
        self.parse_grader_response(&response, "grader")
    }

    /// Stage 2: Critic challenges the output
    async fn run_critic(
        &self,
        input: &str,
        output: &str,
        grader: &StageVerdict,
    ) -> Result<StageVerdict, String> {
        let prompt = format!(
            "You are a harsh AI output critic. The grader gave this score: {:.2}.\n\n\
             USER INPUT: {}\n\n\
             AI OUTPUT: {}\n\n\
             GRADER REASONING: {}\n\n\
             Your job: Find EVERY weakness, error, and problem. Be thorough and adversarial.\n\
             Challenge the grader's scores where they seem too generous.\n\n\
             Respond ONLY in this JSON format:\n\
             {{\"correctness\":0.0,\"completeness\":0.0,\"coherence\":0.0,\"safety\":0.0,\"helpfulness\":0.0,\"reasoning\":\"...\",\"issues\":[\"...\"]}}",
            grader.score, input, output, grader.reasoning
        );

        let response = self.call_ollama(&self.config.critic_model, &prompt).await?;
        self.parse_grader_response(&response, "critic")
    }

    /// Stage 3: Defender argues for the output's merits
    async fn run_defender(
        &self,
        input: &str,
        output: &str,
        critic: &StageVerdict,
    ) -> Result<StageVerdict, String> {
        let issues_str = critic.issues.join(", ");
        let prompt = format!(
            "You are a fair AI output defender. The critic found these issues: {}\n\n\
             USER INPUT: {}\n\n\
             AI OUTPUT: {}\n\n\
             CRITIC REASONING: {}\n\n\
             Your job: Defend the output where the criticism is unfair. Acknowledge real issues but argue for merits.\n\
             Give your own honest scores.\n\n\
             Respond ONLY in this JSON format:\n\
             {{\"correctness\":0.0,\"completeness\":0.0,\"coherence\":0.0,\"safety\":0.0,\"helpfulness\":0.0,\"reasoning\":\"...\",\"issues\":[\"...\"]}}",
            issues_str, input, output, critic.reasoning
        );

        let response = self.call_ollama(&self.config.grader_model, &prompt).await?;
        self.parse_grader_response(&response, "defender")
    }

    /// Stage 4: Meta-Judge synthesizes all verdicts
    async fn run_meta_judge(&self, verdicts: &[StageVerdict]) -> Result<f32, String> {
        if verdicts.is_empty() {
            return Ok(0.0);
        }

        // Weighted average: grader 40%, critic 35%, defender 25%
        let weights = [0.40, 0.35, 0.25];
        let mut total_score = 0.0;
        let mut total_weight = 0.0;

        for (i, verdict) in verdicts.iter().enumerate() {
            let weight = weights.get(i).copied().unwrap_or(0.2);
            total_score += verdict.dimensions.average() * weight;
            total_weight += weight;
        }

        if total_weight > 0.0 {
            Ok(total_score / total_weight)
        } else {
            Ok(0.0)
        }
    }

    /// Call Ollama API
    async fn call_ollama(&self, model: &str, prompt: &str) -> Result<String, String> {
        let url = format!("{}/api/generate", self.config.ollama_url);

        let body = serde_json::json!({
            "model": model,
            "prompt": prompt,
            "stream": false,
            "options": {
                "temperature": 0.3,
                "num_predict": 512
            }
        });

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Ollama request failed: {}", e))?;

        let data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

        data["response"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "No response field in Ollama output".to_string())
    }

    /// Parse JSON response from grader/critic/defender
    fn parse_grader_response(&self, response: &str, stage: &str) -> Result<StageVerdict, String> {
        // Try to extract JSON from the response
        let json_str = if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                &response[start..=end]
            } else {
                response
            }
        } else {
            response
        };

        #[derive(Deserialize)]
        struct RawVerdict {
            #[serde(default)]
            correctness: f32,
            #[serde(default)]
            completeness: f32,
            #[serde(default)]
            coherence: f32,
            #[serde(default)]
            safety: f32,
            #[serde(default)]
            helpfulness: f32,
            #[serde(default)]
            reasoning: String,
            #[serde(default)]
            issues: Vec<String>,
        }

        let raw: RawVerdict = serde_json::from_str(json_str).unwrap_or(RawVerdict {
            correctness: 0.5,
            completeness: 0.5,
            coherence: 0.5,
            safety: 1.0,
            helpfulness: 0.5,
            reasoning: format!("Failed to parse {} response, using defaults", stage),
            issues: vec![format!("Parse error in {} stage", stage)],
        });

        let dimensions = EvalDimensions {
            correctness: raw.correctness.clamp(0.0, 1.0),
            completeness: raw.completeness.clamp(0.0, 1.0),
            coherence: raw.coherence.clamp(0.0, 1.0),
            safety: raw.safety.clamp(0.0, 1.0),
            helpfulness: raw.helpfulness.clamp(0.0, 1.0),
        };

        Ok(StageVerdict {
            stage: stage.to_string(),
            score: dimensions.average(),
            dimensions,
            reasoning: raw.reasoning,
            issues: raw.issues,
            timestamp: Utc::now().to_rfc3339(),
        })
    }
}

/// History of evaluations (in-memory, for the UI)
static EVAL_HISTORY: std::sync::LazyLock<std::sync::RwLock<Vec<EvalResult>>> =
    std::sync::LazyLock::new(|| std::sync::RwLock::new(Vec::new()));

// --- Tauri Commands ---

/// Evaluate an agent's output through the full chain
#[tauri::command]
pub async fn eval_agent_output(
    input: String,
    output: String,
    agent_id: String,
    config: Option<EvalConfig>,
) -> Result<EvalResult, String> {
    let chain = match config {
        Some(c) => EvalChain::new(c),
        None => EvalChain::with_defaults(),
    };

    let result = chain.evaluate(&input, &output, &agent_id).await?;

    // Store in history
    if let Ok(mut history) = EVAL_HISTORY.write() {
        history.push(result.clone());
        // Keep last 100 evaluations
        if history.len() > 100 {
            let excess = history.len() - 100;
            history.drain(0..excess);
        }
    }

    Ok(result)
}

/// Quick evaluation -- only grader stage, for real-time use
#[tauri::command]
pub async fn eval_quick(input: String, output: String) -> Result<StageVerdict, String> {
    let chain = EvalChain::with_defaults();
    chain.run_grader(&input, &output).await
}

/// Get evaluation history
#[tauri::command]
pub fn eval_history() -> Result<Vec<EvalResult>, String> {
    let history = EVAL_HISTORY
        .read()
        .map_err(|e| format!("Failed to read eval history: {}", e))?;
    Ok(history.clone())
}

/// Get evaluation config defaults
#[tauri::command]
pub fn eval_get_config() -> EvalConfig {
    EvalConfig::default()
}
