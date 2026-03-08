use serde::Serialize;
use tauri::ipc::Channel;
use reqwest::Client;
use futures_util::StreamExt;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", content = "data")]
pub enum ChatEvent {
    Started { model: String, task_type: String },
    Delta { content: String },
    Finished { total_tokens: u32 },
    Error { message: String },
}

#[tauri::command]
pub async fn chat_stream(
    message: String,
    model_id: Option<String>,
    system_prompt: Option<String>,
    openrouter_key: Option<String>,
    on_event: Channel<ChatEvent>,
) -> Result<(), String> {
    let client = Client::new();
    let task_type = crate::router::classify_fast(&message);
    let task_type_str = format!("{:?}", task_type);

    let model = model_id.unwrap_or_else(|| {
        match task_type {
            crate::router::TaskType::CodeGeneration
            | crate::router::TaskType::DockerfileGen => {
                "mistralai/devstral-small:free".to_string()
            }
            crate::router::TaskType::MultiStepReasoning => {
                "qwen/qwen3-30b-a3b:free".to_string()
            }
            _ => "meta-llama/llama-4-scout:free".to_string(),
        }
    });

    on_event.send(ChatEvent::Started {
        model: model.clone(),
        task_type: task_type_str,
    }).map_err(|e| e.to_string())?;

    let key = openrouter_key.unwrap_or_default();
    if key.is_empty() {
        on_event.send(ChatEvent::Error {
            message: "No OpenRouter API key configured. Go to Settings > AI to add one.".into()
        }).map_err(|e| e.to_string())?;
        return Ok(());
    }

    let mut messages = Vec::new();
    if let Some(sys) = system_prompt {
        messages.push(serde_json::json!({"role": "system", "content": sys}));
    }
    messages.push(serde_json::json!({"role": "user", "content": message}));

    let response = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", key))
        .header("HTTP-Referer", "https://github.com/AiImpDevelopment/impforge-workstation")
        .header("X-Title", "ImpForge AI Workstation")
        .json(&serde_json::json!({
            "model": model,
            "messages": messages,
            "stream": true,
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        on_event.send(ChatEvent::Error {
            message: format!("OpenRouter API error {}: {}", status, body)
        }).map_err(|e| e.to_string())?;
        return Ok(());
    }

    let mut stream = response.bytes_stream();
    let mut total_tokens: u32 = 0;
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].trim().to_string();
            buffer = buffer[pos + 1..].to_string();

            if line.starts_with("data: [DONE]") {
                break;
            }
            if line.starts_with("data: ") {
                let json_str = &line[6..];
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                    if let Some(content) = val["choices"][0]["delta"]["content"].as_str() {
                        if !content.is_empty() {
                            total_tokens += 1;
                            let _ = on_event.send(ChatEvent::Delta {
                                content: content.to_string()
                            });
                        }
                    }
                }
            }
        }
    }

    on_event.send(ChatEvent::Finished { total_tokens }).map_err(|e| e.to_string())?;
    Ok(())
}
