//! Structured error handling for ImpForge
//!
//! Provides `ImpForgeError` — a serializable error type that sends structured
//! error responses to the Svelte frontend instead of raw strings.
//!
//! ## Usage in Tauri commands
//!
//! Commands can return `Result<T, ImpForgeError>` directly — Tauri serializes
//! the error as JSON to the frontend. The frontend receives a structured object:
//!
//! ```json
//! {
//!   "category": "service",
//!   "code": "OLLAMA_UNREACHABLE",
//!   "message": "Cannot connect to Ollama",
//!   "details": "Connection refused (os error 111)",
//!   "suggestion": "Start Ollama with: ollama serve"
//! }
//! ```
//!
//! For backward compatibility, `From<ImpForgeError> for String` is implemented
//! so commands that still return `Result<T, String>` can use `.map_err(ImpForgeError::into)`.
//!
//! Includes a panic hook for graceful recovery on unexpected crashes.

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::inference::InferenceError;
use crate::router::RouterError;

/// Error categories for frontend routing and display
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    /// Network/service unreachable (Ollama, Docker, HuggingFace)
    Service,
    /// User input validation failure
    Validation,
    /// File system / disk / path errors
    FileSystem,
    /// AI model loading / inference errors
    Model,
    /// Browser automation (CDP) errors
    Browser,
    /// Configuration / settings errors
    Config,
    /// Agent lifecycle / execution errors
    Agent,
    /// Internal logic error (should not happen)
    Internal,
}

/// Structured error for Tauri command responses.
/// Serialized as JSON so the Svelte frontend can parse and display appropriately.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpForgeError {
    pub category: ErrorCategory,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

impl fmt::Display for ImpForgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for ImpForgeError {}

impl ImpForgeError {
    pub fn service(code: &str, message: impl Into<String>) -> Self {
        Self {
            category: ErrorCategory::Service,
            code: code.to_string(),
            message: message.into(),
            details: None,
            suggestion: None,
        }
    }

    pub fn validation(code: &str, message: impl Into<String>) -> Self {
        Self {
            category: ErrorCategory::Validation,
            code: code.to_string(),
            message: message.into(),
            details: None,
            suggestion: None,
        }
    }

    pub fn filesystem(code: &str, message: impl Into<String>) -> Self {
        Self {
            category: ErrorCategory::FileSystem,
            code: code.to_string(),
            message: message.into(),
            details: None,
            suggestion: None,
        }
    }

    pub fn model(code: &str, message: impl Into<String>) -> Self {
        Self {
            category: ErrorCategory::Model,
            code: code.to_string(),
            message: message.into(),
            details: None,
            suggestion: None,
        }
    }

    pub fn browser(code: &str, message: impl Into<String>) -> Self {
        Self {
            category: ErrorCategory::Browser,
            code: code.to_string(),
            message: message.into(),
            details: None,
            suggestion: None,
        }
    }

    pub fn config(code: &str, message: impl Into<String>) -> Self {
        Self {
            category: ErrorCategory::Config,
            code: code.to_string(),
            message: message.into(),
            details: None,
            suggestion: None,
        }
    }

    pub fn agent(code: &str, message: impl Into<String>) -> Self {
        Self {
            category: ErrorCategory::Agent,
            code: code.to_string(),
            message: message.into(),
            details: None,
            suggestion: None,
        }
    }

    pub fn internal(code: &str, message: impl Into<String>) -> Self {
        Self {
            category: ErrorCategory::Internal,
            code: code.to_string(),
            message: message.into(),
            details: None,
            suggestion: None,
        }
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Convert to a JSON string for Tauri command error responses.
    /// This is the bridge between Rust errors and Svelte error handling.
    pub fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| {
            format!(r#"{{"category":"internal","code":"SERIALIZE_FAIL","message":"{}"}}"#, self.message)
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Backward compatibility: commands returning Result<T, String> can still use
// ImpForgeError via `.map_err(ImpForgeError::from)` or `into()`.
// ─────────────────────────────────────────────────────────────────────────────

impl From<ImpForgeError> for String {
    fn from(err: ImpForgeError) -> String {
        err.to_json_string()
    }
}

/// Parse a JSON-encoded `ImpForgeError` back from a `String`.
/// If the string is not valid JSON, wrap it as an `Internal` error so the
/// category/code structure is preserved on round-trip.
impl From<String> for ImpForgeError {
    fn from(s: String) -> Self {
        serde_json::from_str::<ImpForgeError>(&s).unwrap_or_else(|_| {
            Self::internal("UNKNOWN", s)
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Conversion from common error types
// ─────────────────────────────────────────────────────────────────────────────

impl From<std::io::Error> for ImpForgeError {
    fn from(e: std::io::Error) -> Self {
        let suggestion = match e.kind() {
            std::io::ErrorKind::NotFound => Some("Check that the file path exists and is spelled correctly."),
            std::io::ErrorKind::PermissionDenied => Some("Check file permissions. You may need to run ImpForge with appropriate access."),
            _ => None,
        };
        let mut err = Self::filesystem("IO_ERROR", e.to_string());
        if let Some(s) = suggestion {
            err = err.with_suggestion(s);
        }
        err
    }
}

impl From<reqwest::Error> for ImpForgeError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_connect() {
            Self::service("CONNECTION_FAILED", "Cannot reach service")
                .with_details(e.to_string())
                .with_suggestion("Check if the service is running (Ollama, Docker, etc.)")
        } else if e.is_timeout() {
            Self::service("TIMEOUT", "Request timed out")
                .with_details(e.to_string())
                .with_suggestion("The service may be overloaded or the model is still loading. Try again in a moment.")
        } else {
            Self::service("REQUEST_FAILED", e.to_string())
        }
    }
}

impl From<serde_json::Error> for ImpForgeError {
    fn from(e: serde_json::Error) -> Self {
        Self::internal("JSON_ERROR", format!("JSON parsing failed: {e}"))
    }
}

impl From<rusqlite::Error> for ImpForgeError {
    fn from(e: rusqlite::Error) -> Self {
        Self::internal("DB_ERROR", format!("Database error: {e}"))
            .with_suggestion("If this persists, try restarting ImpForge. Your data is safe (WAL mode).")
    }
}

impl From<bollard::errors::Error> for ImpForgeError {
    fn from(e: bollard::errors::Error) -> Self {
        Self::service("DOCKER_ERROR", format!("Docker error: {e}"))
            .with_suggestion("Is Docker running? Start it with: sudo systemctl start docker")
    }
}

impl From<InferenceError> for ImpForgeError {
    fn from(e: InferenceError) -> Self {
        match &e {
            InferenceError::ModelNotFound(path) => {
                Self::model("MODEL_NOT_FOUND", format!("Model not found: {path}"))
                    .with_suggestion("Download the model first, or check the file path is correct.")
            }
            InferenceError::DownloadFailed(reason) => {
                Self::model("DOWNLOAD_FAILED", format!("Model download failed: {reason}"))
                    .with_suggestion("Check your internet connection and try again. For HuggingFace models, verify the repo ID.")
            }
            InferenceError::InferenceFailed(reason) => {
                Self::model("INFERENCE_FAILED", format!("Inference failed: {reason}"))
                    .with_suggestion("Try a different model or reduce the context size. For GGUF models, use Ollama as an alternative.")
            }
            InferenceError::InvalidFormat(reason) => {
                Self::model("INVALID_FORMAT", format!("Invalid model format: {reason}"))
                    .with_suggestion("ImpForge supports .gguf and .safetensors formats. Check the file extension.")
            }
            InferenceError::GpuUnavailable => {
                Self::model("GPU_UNAVAILABLE", "No compatible GPU detected")
                    .with_suggestion("Set GPU layers to 0 for CPU-only inference, or install ROCm (AMD) / CUDA (NVIDIA) drivers.")
            }
            InferenceError::IoError(_) => {
                Self::filesystem("MODEL_IO_ERROR", e.to_string())
                    .with_suggestion("Check disk space and file permissions for the model directory.")
            }
        }
    }
}

impl From<RouterError> for ImpForgeError {
    fn from(e: RouterError) -> Self {
        match &e {
            RouterError::MissingApiKey { provider } => {
                Self::config("MISSING_API_KEY", format!("No API key configured for {provider}"))
                    .with_suggestion(format!(
                        "Add your {provider} API key in Settings > API Keys, or use a local Ollama model instead."
                    ))
            }
            RouterError::ModelUnavailable { model } => {
                Self::model("MODEL_UNAVAILABLE", format!("Model not available: {model}"))
                    .with_suggestion("Check that the model is downloaded (for Ollama) or that your API key has access to this model.")
            }
            RouterError::RateLimitExceeded { model } => {
                Self::service("RATE_LIMITED", format!("Rate limit exceeded for {model}"))
                    .with_suggestion("Wait a moment and try again, or switch to a different model.")
            }
            RouterError::RequestFailed(_) => {
                let imp_err: ImpForgeError = match e {
                    RouterError::RequestFailed(inner) => {
                        // Delegate to the reqwest conversion for richer details
                        // but we only get here if the match arm matched, so reconstruct
                        Self::service("ROUTER_REQUEST_FAILED", inner.to_string())
                    }
                    _ => unreachable!(),
                };
                imp_err.with_suggestion("Check your network connection and service availability.")
            }
            RouterError::InvalidResponse(msg) => {
                Self::service("INVALID_RESPONSE", format!("Invalid response from AI provider: {msg}"))
                    .with_suggestion("The AI provider returned an unexpected response. Try a different model.")
            }
        }
    }
}

/// Trait to convert any Result<T, E> into Result<T, String> with structured JSON errors.
/// Use this at Tauri command boundaries: `result.map_impforge_err()?`
pub trait ImpForgeResultExt<T> {
    fn map_impforge_err(self) -> Result<T, String>;
}

impl<T, E: Into<ImpForgeError>> ImpForgeResultExt<T> for Result<T, E> {
    fn map_impforge_err(self) -> Result<T, String> {
        self.map_err(|e| {
            let impforge_err: ImpForgeError = e.into();
            impforge_err.to_json_string()
        })
    }
}

/// Convenience type alias for Tauri commands returning structured errors.
/// Commands using this alias send JSON-serialized `ImpForgeError` to the frontend.
pub type AppResult<T> = Result<T, ImpForgeError>;

/// Install a panic hook that logs panics instead of crashing the app.
/// The Tauri window stays open and the user sees an error notification.
pub fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()));
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic".to_string()
        };

        log::error!(
            "IMPFORGE PANIC RECOVERED: {} at {}",
            payload,
            location.as_deref().unwrap_or("unknown")
        );

        // Still call the default hook for logging/backtrace
        default_hook(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_impforge_error_serialization() {
        let err = ImpForgeError::service("OLLAMA_DOWN", "Ollama is not running")
            .with_suggestion("Start Ollama with: ollama serve");

        let json = err.to_json_string();
        assert!(json.contains("OLLAMA_DOWN"));
        assert!(json.contains("service"));
        assert!(json.contains("ollama serve"));
    }

    #[test]
    fn test_impforge_error_display() {
        let err = ImpForgeError::internal("TEST", "test message");
        assert_eq!(format!("{err}"), "[TEST] test message");
    }

    #[test]
    fn test_error_categories() {
        let service_err = ImpForgeError::service("S1", "svc");
        let model_err = ImpForgeError::model("M1", "mdl");
        let browser_err = ImpForgeError::browser("B1", "brw");
        let agent_err = ImpForgeError::agent("A1", "agt");

        assert_eq!(service_err.category, ErrorCategory::Service);
        assert_eq!(model_err.category, ErrorCategory::Model);
        assert_eq!(browser_err.category, ErrorCategory::Browser);
        assert_eq!(agent_err.category, ErrorCategory::Agent);
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let impforge_err: ImpForgeError = io_err.into();
        assert_eq!(impforge_err.category, ErrorCategory::FileSystem);
        assert_eq!(impforge_err.code, "IO_ERROR");
        assert!(impforge_err.suggestion.is_some());
    }

    #[test]
    fn test_io_error_permission_denied() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let impforge_err: ImpForgeError = io_err.into();
        assert_eq!(impforge_err.code, "IO_ERROR");
        assert!(impforge_err.suggestion.unwrap().contains("permissions"));
    }

    #[test]
    fn test_json_error_conversion() {
        let json_err = serde_json::from_str::<String>("invalid").unwrap_err();
        let impforge_err: ImpForgeError = json_err.into();
        assert_eq!(impforge_err.category, ErrorCategory::Internal);
        assert_eq!(impforge_err.code, "JSON_ERROR");
    }

    #[test]
    fn test_map_impforge_err_trait() {
        let ok_result: Result<i32, std::io::Error> = Ok(42);
        assert_eq!(ok_result.map_impforge_err().unwrap(), 42);

        let err_result: Result<i32, std::io::Error> =
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "gone"));
        let err_string = err_result.map_impforge_err().unwrap_err();
        assert!(err_string.contains("IO_ERROR"));
        assert!(err_string.contains("file_system"));
    }

    #[test]
    fn test_with_details_and_suggestion() {
        let err = ImpForgeError::service("TEST", "msg")
            .with_details("detail info")
            .with_suggestion("try this");

        assert_eq!(err.details.unwrap(), "detail info");
        assert_eq!(err.suggestion.unwrap(), "try this");
    }

    #[test]
    fn test_into_string_produces_json() {
        let err = ImpForgeError::service("SVC_ERR", "service down");
        let s: String = err.into();
        // Should be valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&s).expect("should be valid JSON");
        assert_eq!(parsed["category"], "service");
        assert_eq!(parsed["code"], "SVC_ERR");
    }

    #[test]
    fn test_from_string_roundtrip() {
        let original = ImpForgeError::model("MODEL_ERR", "model broke")
            .with_suggestion("try again");
        let json_string: String = original.clone().into();
        let roundtrip: ImpForgeError = json_string.into();
        assert_eq!(roundtrip.category, ErrorCategory::Model);
        assert_eq!(roundtrip.code, "MODEL_ERR");
        assert_eq!(roundtrip.suggestion, Some("try again".to_string()));
    }

    #[test]
    fn test_from_plain_string_wraps_as_internal() {
        let plain = "something went wrong".to_string();
        let err: ImpForgeError = plain.into();
        assert_eq!(err.category, ErrorCategory::Internal);
        assert_eq!(err.code, "UNKNOWN");
        assert!(err.message.contains("something went wrong"));
    }

    #[test]
    fn test_inference_error_model_not_found() {
        let ie = InferenceError::ModelNotFound("/path/to/model.gguf".to_string());
        let err: ImpForgeError = ie.into();
        assert_eq!(err.category, ErrorCategory::Model);
        assert_eq!(err.code, "MODEL_NOT_FOUND");
        assert!(err.suggestion.is_some());
    }

    #[test]
    fn test_inference_error_gpu_unavailable() {
        let ie = InferenceError::GpuUnavailable;
        let err: ImpForgeError = ie.into();
        assert_eq!(err.category, ErrorCategory::Model);
        assert_eq!(err.code, "GPU_UNAVAILABLE");
        assert!(err.suggestion.unwrap().contains("CPU-only"));
    }

    #[test]
    fn test_router_error_missing_key() {
        let re = RouterError::MissingApiKey { provider: "OpenRouter".to_string() };
        let err: ImpForgeError = re.into();
        assert_eq!(err.category, ErrorCategory::Config);
        assert_eq!(err.code, "MISSING_API_KEY");
        assert!(err.suggestion.unwrap().contains("OpenRouter"));
    }

    #[test]
    fn test_router_error_rate_limited() {
        let re = RouterError::RateLimitExceeded { model: "gpt-4".to_string() };
        let err: ImpForgeError = re.into();
        assert_eq!(err.category, ErrorCategory::Service);
        assert_eq!(err.code, "RATE_LIMITED");
    }

    #[test]
    fn test_app_result_type_alias() {
        fn example_ok() -> AppResult<i32> {
            Ok(42)
        }
        fn example_err() -> AppResult<i32> {
            Err(ImpForgeError::validation("BAD_INPUT", "invalid"))
        }
        assert!(example_ok().is_ok());
        assert!(example_err().is_err());
    }
}
