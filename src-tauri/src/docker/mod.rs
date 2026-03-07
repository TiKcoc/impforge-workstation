//! Docker Management
//!
//! Provides Docker container management through the bollard crate.
//! Connects via Unix socket for optimal performance and security.

use bollard::container::{ListContainersOptions, StartContainerOptions, StopContainerOptions, RemoveContainerOptions, LogsOptions};
use bollard::Docker;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Container information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub state: String,
    pub ports: Vec<String>,
}

/// Container action types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContainerAction {
    Start,
    Stop,
    Restart,
    Remove,
    Logs,
}

/// Get Docker client (Unix socket connection)
fn get_docker() -> Result<Docker, String> {
    Docker::connect_with_socket_defaults()
        .map_err(|e| format!("Failed to connect to Docker: {}", e))
}

/// List all Docker containers
#[tauri::command]
pub async fn list_containers() -> Result<Vec<ContainerInfo>, String> {
    log::info!("Listing Docker containers");

    let docker = get_docker()?;

    let options = ListContainersOptions::<String> {
        all: true,
        ..Default::default()
    };

    let containers = docker
        .list_containers(Some(options))
        .await
        .map_err(|e| format!("Failed to list containers: {}", e))?;

    let result: Vec<ContainerInfo> = containers
        .into_iter()
        .map(|c| {
            let ports: Vec<String> = c.ports.unwrap_or_default()
                .iter()
                .filter_map(|p| {
                    p.public_port.map(|pub_port| {
                        format!("{}:{}", pub_port, p.private_port)
                    })
                })
                .collect();

            ContainerInfo {
                id: c.id.unwrap_or_default(),
                name: c.names.unwrap_or_default().first()
                    .map(|n| n.trim_start_matches('/').to_string())
                    .unwrap_or_default(),
                image: c.image.unwrap_or_default(),
                status: c.status.unwrap_or_default(),
                state: c.state.unwrap_or_default(),
                ports,
            }
        })
        .collect();

    log::info!("Found {} containers", result.len());
    Ok(result)
}

/// Perform action on a container
#[tauri::command]
pub async fn container_action(
    container_id: String,
    action: ContainerAction,
) -> Result<String, String> {
    log::info!("Container action {:?} on {}", action, container_id);

    let docker = get_docker()?;

    match action {
        ContainerAction::Start => {
            docker
                .start_container(&container_id, None::<StartContainerOptions<String>>)
                .await
                .map_err(|e| format!("Failed to start container: {}", e))?;
            Ok(format!("Container {} started", container_id))
        }
        ContainerAction::Stop => {
            docker
                .stop_container(&container_id, None::<StopContainerOptions>)
                .await
                .map_err(|e| format!("Failed to stop container: {}", e))?;
            Ok(format!("Container {} stopped", container_id))
        }
        ContainerAction::Restart => {
            docker
                .restart_container(&container_id, None)
                .await
                .map_err(|e| format!("Failed to restart container: {}", e))?;
            Ok(format!("Container {} restarted", container_id))
        }
        ContainerAction::Remove => {
            let options = RemoveContainerOptions {
                force: true,
                ..Default::default()
            };
            docker
                .remove_container(&container_id, Some(options))
                .await
                .map_err(|e| format!("Failed to remove container: {}", e))?;
            Ok(format!("Container {} removed", container_id))
        }
        ContainerAction::Logs => {
            let options = LogsOptions::<String> {
                stdout: true,
                stderr: true,
                tail: "100".to_string(),
                ..Default::default()
            };

            let mut logs_stream = docker.logs(&container_id, Some(options));
            let mut logs = String::new();

            while let Some(log_result) = logs_stream.next().await {
                match log_result {
                    Ok(log_output) => {
                        logs.push_str(&log_output.to_string());
                    }
                    Err(e) => {
                        return Err(format!("Failed to read logs: {}", e));
                    }
                }
            }

            Ok(logs)
        }
    }
}

/// Get Docker system info
#[tauri::command]
pub async fn docker_info() -> Result<HashMap<String, String>, String> {
    let docker = get_docker()?;

    let info = docker
        .info()
        .await
        .map_err(|e| format!("Failed to get Docker info: {}", e))?;

    let mut result = HashMap::new();
    result.insert("containers".to_string(), info.containers.unwrap_or(0).to_string());
    result.insert("images".to_string(), info.images.unwrap_or(0).to_string());
    result.insert("name".to_string(), info.name.unwrap_or_default());
    result.insert("server_version".to_string(), info.server_version.unwrap_or_default());
    result.insert("os".to_string(), info.operating_system.unwrap_or_default());

    Ok(result)
}
