//! HTTP Client — Postman-like REST API testing engine
//!
//! Built on reqwest for async HTTP with full control over
//! method, headers, body, auth. Enterprise features:
//! - All HTTP methods (GET/POST/PUT/PATCH/DELETE/HEAD/OPTIONS)
//! - Custom headers and authentication (Bearer, Basic, API Key)
//! - Request body (JSON, form, raw text)
//! - Response timing, size, status analysis
//! - Request collections (saved to JSON)
//! - cURL import/export

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    pub body_type: Option<String>, // "json", "form", "text", "none"
    pub auth: Option<AuthConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub auth_type: String, // "bearer", "basic", "api_key"
    pub token: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub key_name: Option<String>,
    pub key_value: Option<String>,
    pub key_location: Option<String>, // "header" or "query"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub size_bytes: usize,
    pub elapsed_ms: f64,
    pub content_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedRequest {
    pub id: String,
    pub name: String,
    pub collection: String,
    pub request: HttpRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestCollection {
    pub name: String,
    pub requests: Vec<SavedRequest>,
}

/// Execute an HTTP request
#[tauri::command]
pub async fn http_send_request(request: HttpRequest) -> Result<HttpResponse, String> {
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(false)
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Client build error: {}", e))?;

    let method: Method = request
        .method
        .parse()
        .map_err(|_| format!("Invalid method: {}", request.method))?;

    let mut req_builder = client.request(method, &request.url);

    // Apply headers
    let mut header_map = HeaderMap::new();
    for (key, val) in &request.headers {
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(key.as_bytes()),
            HeaderValue::from_str(val),
        ) {
            header_map.insert(name, value);
        }
    }

    // Apply auth
    if let Some(auth) = &request.auth {
        match auth.auth_type.as_str() {
            "bearer" => {
                if let Some(token) = &auth.token {
                    header_map.insert(
                        reqwest::header::AUTHORIZATION,
                        HeaderValue::from_str(&format!("Bearer {}", token))
                            .map_err(|e| e.to_string())?,
                    );
                }
            }
            "basic" => {
                let user = auth.username.as_deref().unwrap_or("");
                let pass = auth.password.as_deref().unwrap_or("");
                req_builder = req_builder.basic_auth(user, Some(pass));
            }
            "api_key" => {
                if let (Some(name), Some(value)) = (&auth.key_name, &auth.key_value) {
                    let location = auth.key_location.as_deref().unwrap_or("header");
                    if location == "header" {
                        if let (Ok(h_name), Ok(h_val)) = (
                            HeaderName::from_bytes(name.as_bytes()),
                            HeaderValue::from_str(value),
                        ) {
                            header_map.insert(h_name, h_val);
                        }
                    }
                    // Query param auth handled below in URL
                }
            }
            _ => {}
        }
    }

    req_builder = req_builder.headers(header_map);

    // Apply body
    if let Some(body) = &request.body {
        let body_type = request.body_type.as_deref().unwrap_or("text");
        match body_type {
            "json" => {
                req_builder = req_builder
                    .header("Content-Type", "application/json")
                    .body(body.clone());
            }
            "form" => {
                // Parse key=value pairs
                let pairs: HashMap<String, String> = body
                    .lines()
                    .filter_map(|line| {
                        let parts: Vec<&str> = line.splitn(2, '=').collect();
                        if parts.len() == 2 {
                            Some((parts[0].to_string(), parts[1].to_string()))
                        } else {
                            None
                        }
                    })
                    .collect();
                req_builder = req_builder.form(&pairs);
            }
            _ => {
                req_builder = req_builder.body(body.clone());
            }
        }
    }

    // Handle API key in query params
    if let Some(auth) = &request.auth {
        if auth.auth_type == "api_key" {
            if let Some(location) = &auth.key_location {
                if location == "query" {
                    if let (Some(name), Some(value)) = (&auth.key_name, &auth.key_value) {
                        req_builder = req_builder.query(&[(name, value)]);
                    }
                }
            }
        }
    }

    let start = Instant::now();
    let response = req_builder
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let status = response.status().as_u16();
    let status_text = response
        .status()
        .canonical_reason()
        .unwrap_or("Unknown")
        .to_string();

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let resp_headers: HashMap<String, String> = response
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let body_bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Body read error: {}", e))?;

    let size_bytes = body_bytes.len();
    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;

    // Try to decode as UTF-8, fallback to lossy
    let body = String::from_utf8(body_bytes.to_vec())
        .unwrap_or_else(|_| String::from_utf8_lossy(&body_bytes).to_string());

    Ok(HttpResponse {
        status,
        status_text,
        headers: resp_headers,
        body,
        size_bytes,
        elapsed_ms,
        content_type,
    })
}

/// Generate a cURL command from a request
#[tauri::command]
pub fn http_to_curl(request: HttpRequest) -> Result<String, String> {
    let mut parts = vec![format!("curl -X {} '{}'", request.method, request.url)];

    for (key, val) in &request.headers {
        parts.push(format!("-H '{}: {}'", key, val));
    }

    if let Some(auth) = &request.auth {
        match auth.auth_type.as_str() {
            "bearer" => {
                if let Some(token) = &auth.token {
                    parts.push(format!("-H 'Authorization: Bearer {}'", token));
                }
            }
            "basic" => {
                let user = auth.username.as_deref().unwrap_or("");
                let pass = auth.password.as_deref().unwrap_or("");
                parts.push(format!("-u '{}:{}'", user, pass));
            }
            _ => {}
        }
    }

    if let Some(body) = &request.body {
        let body_type = request.body_type.as_deref().unwrap_or("text");
        if body_type == "json" {
            parts.push("-H 'Content-Type: application/json'".to_string());
        }
        parts.push(format!("-d '{}'", body.replace('\'', "'\\''")));
    }

    Ok(parts.join(" \\\n  "))
}

/// Save a request collection to a JSON file
#[tauri::command]
pub async fn http_save_collection(
    path: String,
    collection: RequestCollection,
) -> Result<(), String> {
    let json = serde_json::to_string_pretty(&collection).map_err(|e| e.to_string())?;
    tokio::fs::write(&path, json)
        .await
        .map_err(|e| format!("Failed to save collection: {}", e))
}

/// Load a request collection from a JSON file
#[tauri::command]
pub async fn http_load_collection(path: String) -> Result<RequestCollection, String> {
    let json = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("Failed to read collection: {}", e))?;
    serde_json::from_str(&json).map_err(|e| format!("Invalid collection format: {}", e))
}
