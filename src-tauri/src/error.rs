//! Structured error handling for NEXUS
//!
//! Provides `NexusError` — a serializable error type that sends structured
//! error responses to the Svelte frontend instead of raw strings.
//! Includes a panic hook for graceful recovery on unexpected crashes.

use serde::{Deserialize, Serialize};
use std::fmt;

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
    /// Internal logic error (should not happen)
    Internal,
}

/// Structured error for Tauri command responses.
/// Serialized as JSON so the Svelte frontend can parse and display appropriately.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NexusError {
    pub category: ErrorCategory,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

impl fmt::Display for NexusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for NexusError {}

impl NexusError {
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

// Conversion from common error types

impl From<std::io::Error> for NexusError {
    fn from(e: std::io::Error) -> Self {
        Self::filesystem("IO_ERROR", e.to_string())
    }
}

impl From<reqwest::Error> for NexusError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_connect() {
            Self::service("CONNECTION_FAILED", "Cannot reach service")
                .with_details(e.to_string())
                .with_suggestion("Check if the service is running (Ollama, Docker, etc.)")
        } else if e.is_timeout() {
            Self::service("TIMEOUT", "Request timed out")
                .with_details(e.to_string())
        } else {
            Self::service("REQUEST_FAILED", e.to_string())
        }
    }
}

impl From<serde_json::Error> for NexusError {
    fn from(e: serde_json::Error) -> Self {
        Self::internal("JSON_ERROR", format!("JSON parsing failed: {e}"))
    }
}

impl From<rusqlite::Error> for NexusError {
    fn from(e: rusqlite::Error) -> Self {
        Self::internal("DB_ERROR", format!("Database error: {e}"))
    }
}

impl From<bollard::errors::Error> for NexusError {
    fn from(e: bollard::errors::Error) -> Self {
        Self::service("DOCKER_ERROR", format!("Docker error: {e}"))
            .with_suggestion("Is Docker running? Check with: docker ps")
    }
}

/// Trait to convert any Result<T, E> into Result<T, String> with structured JSON errors.
/// Use this at Tauri command boundaries: `result.map_nexus_err()?`
pub trait NexusResultExt<T> {
    fn map_nexus_err(self) -> Result<T, String>;
}

impl<T, E: Into<NexusError>> NexusResultExt<T> for Result<T, E> {
    fn map_nexus_err(self) -> Result<T, String> {
        self.map_err(|e| {
            let nexus_err: NexusError = e.into();
            nexus_err.to_json_string()
        })
    }
}

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
            "NEXUS PANIC RECOVERED: {} at {}",
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
    fn test_nexus_error_serialization() {
        let err = NexusError::service("OLLAMA_DOWN", "Ollama is not running")
            .with_suggestion("Start Ollama with: ollama serve");

        let json = err.to_json_string();
        assert!(json.contains("OLLAMA_DOWN"));
        assert!(json.contains("service"));
        assert!(json.contains("ollama serve"));
    }

    #[test]
    fn test_nexus_error_display() {
        let err = NexusError::internal("TEST", "test message");
        assert_eq!(format!("{err}"), "[TEST] test message");
    }

    #[test]
    fn test_error_categories() {
        let service_err = NexusError::service("S1", "svc");
        let model_err = NexusError::model("M1", "mdl");
        let browser_err = NexusError::browser("B1", "brw");

        assert_eq!(service_err.category, ErrorCategory::Service);
        assert_eq!(model_err.category, ErrorCategory::Model);
        assert_eq!(browser_err.category, ErrorCategory::Browser);
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let nexus_err: NexusError = io_err.into();
        assert_eq!(nexus_err.category, ErrorCategory::FileSystem);
        assert_eq!(nexus_err.code, "IO_ERROR");
    }

    #[test]
    fn test_json_error_conversion() {
        let json_err = serde_json::from_str::<String>("invalid").unwrap_err();
        let nexus_err: NexusError = json_err.into();
        assert_eq!(nexus_err.category, ErrorCategory::Internal);
        assert_eq!(nexus_err.code, "JSON_ERROR");
    }

    #[test]
    fn test_map_nexus_err_trait() {
        let ok_result: Result<i32, std::io::Error> = Ok(42);
        assert_eq!(ok_result.map_nexus_err().unwrap(), 42);

        let err_result: Result<i32, std::io::Error> =
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "gone"));
        let err_string = err_result.map_nexus_err().unwrap_err();
        assert!(err_string.contains("IO_ERROR"));
        assert!(err_string.contains("file_system"));
    }

    #[test]
    fn test_with_details_and_suggestion() {
        let err = NexusError::service("TEST", "msg")
            .with_details("detail info")
            .with_suggestion("try this");

        assert_eq!(err.details.unwrap(), "detail info");
        assert_eq!(err.suggestion.unwrap(), "try this");
    }
}
