// NEXUS Intelligent Model Router
// Routes tasks to the optimal AI model based on TaskType classification

mod classifier;
pub mod targets;

pub use classifier::{classify_fast, TaskType};
pub use targets::{LlmTarget, RouterConfig};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum RouterError {
    #[error("No API key configured for {provider}")]
    MissingApiKey { provider: String },

    #[error("Model not available: {model}")]
    ModelUnavailable { model: String },

    #[error("Rate limit exceeded for {model}")]
    RateLimitExceeded { model: String },

    #[error("Request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}

/// Main routing function - classifies prompt and routes to optimal model
pub async fn route_and_execute(
    prompt: &str,
    system_prompt: Option<&str>,
    config: &RouterConfig,
) -> Result<String, RouterError> {
    // Step 1: Classify the task type (<10ms, no LLM needed)
    let task_type = classify_fast(prompt);

    // Step 2: Select the optimal target based on task type and config
    let target = targets::select_target(task_type, config);

    // Step 3: Execute the request
    target.execute(system_prompt.unwrap_or("You are a helpful assistant."), prompt).await
}

/// Get routing decision without execution (for UI preview)
pub fn get_routing_decision(prompt: &str, config: &RouterConfig) -> (TaskType, LlmTarget) {
    let task_type = classify_fast(prompt);
    let target = targets::select_target(task_type.clone(), config);
    (task_type, target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classification_speed() {
        use std::time::Instant;

        let prompts = vec![
            "Write a function that sorts an array",
            "Create a Dockerfile for a Python app",
            "Build an n8n workflow for email automation",
            "What is the capital of France?",
            "Summarize this README file",
        ];

        let start = Instant::now();
        for prompt in &prompts {
            let _ = classify_fast(prompt);
        }
        let elapsed = start.elapsed();

        // Should complete in under 1ms for 5 prompts
        assert!(elapsed.as_millis() < 10, "Classification too slow: {:?}", elapsed);
    }
}
