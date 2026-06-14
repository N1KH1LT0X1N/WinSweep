//! Docker API client and management
//!
//! This module provides functionality to interact with Docker daemon,
//! including API version negotiation, container management, image cleanup,
//! volume cleanup, and build cache management.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;
use tokio::process;
use tracing::{debug, warn};

/// Docker API client with version negotiation
#[derive(Clone)]
pub struct DockerClient {
    api_version: String,
    daemon_running: bool,
    docker_path: Option<PathBuf>,
}

/// Docker container information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: ContainerStatus,
    pub created: SystemTime,
    pub ports: Vec<PortMapping>,
    pub size_rw: Option<u64>,
    pub size_root_fs: Option<u64>,
}

/// Container status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerStatus {
    Running,
    Exited,
    Paused,
    Restarting,
    Removing,
    Dead,
    Created,
    Unknown,
}

/// Docker image information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    pub id: String,
    pub repository: String,
    pub tag: String,
    pub size: u64,
    pub created: SystemTime,
    pub used: bool,
    pub dangling: bool,
}

/// Port mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortMapping {
    pub container_port: u16,
    pub host_port: Option<u16>,
    pub protocol: String,
}

/// Docker volume information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeInfo {
    pub name: String,
    pub driver: String,
    pub mount_point: PathBuf,
    pub created: SystemTime,
    pub size: Option<u64>,
    pub containers: Vec<String>,
}

/// Docker network information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub scope: String,
    pub internal: bool,
    pub containers: Vec<String>,
    pub created: SystemTime,
}

/// Result of Docker cleanup operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerCleanupResult {
    pub containers_removed: u32,
    pub images_removed: u32,
    pub volumes_removed: u32,
    pub networks_removed: u32,
    pub build_cache_cleaned: bool,
    pub space_freed: u64,
    pub errors: Vec<String>,
    pub duration_ms: u64,
}

impl DockerClient {
    /// Create a new Docker client
    pub async fn new() -> Result<Self> {
        let docker_path = Self::find_docker_executable();
        let daemon_running = Self::check_daemon_status(&docker_path).await?;

        let api_version = if daemon_running {
            Self::negotiate_api_version(&docker_path).await?
        } else {
            "1.41".to_string() // Default latest stable
        };

        Ok(Self {
            api_version,
            daemon_running,
            docker_path,
        })
    }

    /// Find Docker executable
    fn find_docker_executable() -> Option<PathBuf> {
        // Check for docker.exe on Windows
        if let Ok(path) = which::which("docker.exe") {
            return Some(path);
        }

        // Check for docker (WSL/Linux)
        if let Ok(path) = which::which("docker") {
            return Some(path);
        }

        // Check common installation paths
        let common_paths = [
            r"C:\Program Files\Docker\Docker\resources\docker.exe",
            r"C:\Program Files\Docker\Docker\resources\bin\docker.exe",
        ];

        for path in &common_paths {
            if PathBuf::from(path).exists() {
                return Some(PathBuf::from(path));
            }
        }

        None
    }

    /// Check if Docker daemon is running
    async fn check_daemon_status(docker_path: &Option<PathBuf>) -> Result<bool> {
        if let Some(ref docker_path) = docker_path {
            let output = process::Command::new(docker_path)
                .arg("info")
                .output()
                .await;

            match output {
                Ok(result) => Ok(result.status.success()),
                Err(_) => Ok(false),
            }
        } else {
            Ok(false)
        }
    }

    /// Negotiate API version with Docker daemon
    async fn negotiate_api_version(docker_path: &Option<PathBuf>) -> Result<String> {
        if let Some(ref docker_path) = docker_path {
            // Get server API version
            let output = process::Command::new(docker_path)
                .args(["version", "--format", "{{.Server.APIVersion}}"])
                .output()
                .await?;

            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                debug!("Docker API version: {}", version);
                return Ok(version);
            }
        }

        // Fallback to default
        warn!("Could not determine Docker API version, using default");
        Ok("1.41".to_string())
    }

    /// Get all containers
    pub async fn get_containers(&self) -> Result<Vec<ContainerInfo>> {
        if !self.daemon_running {
            return Ok(Vec::new());
        }

        let docker_path = self
            .docker_path
            .as_ref()
            .context("Docker executable not found")?;

        // Get container list with format
        let output = process::Command::new(docker_path)
            .args([
                "ps",
                "-a",
                "--format",
                "{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.Status}}\t{{.CreatedAt}}\t{{.Ports}}\t{{.SizeRw}}\t{{.SizeRootFs}}"
            ])
            .output()
            .await?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to list containers: {}", error));
        }

        let mut containers = Vec::new();
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if line.trim().is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 4 {
                let container = ContainerInfo {
                    id: parts[0].to_string(),
                    name: parts[1].to_string(),
                    image: parts[2].to_string(),
                    status: Self::parse_container_status(parts[3]),
                    created: Self::parse_docker_time(parts[4]).unwrap_or(SystemTime::UNIX_EPOCH),
                    ports: Self::parse_ports(parts.get(5).unwrap_or(&"")),
                    size_rw: parts.get(6).and_then(|s| s.parse::<u64>().ok()),
                    size_root_fs: parts.get(7).and_then(|s| s.parse::<u64>().ok()),
                };
                containers.push(container);
            }
        }

        Ok(containers)
    }

    /// Get all images
    pub async fn get_images(&self) -> Result<Vec<ImageInfo>> {
        if !self.daemon_running {
            return Ok(Vec::new());
        }

        let docker_path = self
            .docker_path
            .as_ref()
            .context("Docker executable not found")?;

        // Get image list with format
        let output = process::Command::new(docker_path)
            .args([
                "images",
                "--format",
                "{{.ID}}\t{{.Repository}}\t{{.Tag}}\t{{.Size}}\t{{.CreatedAt}}\t{{.CreatedAtSince}}"
            ])
            .output()
            .await?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to list images: {}", error));
        }

        let mut images = Vec::new();
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if line.trim().is_empty() || line.starts_with("REPOSITORY") {
                continue;
            }

            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 {
                let size = Self::parse_size(parts.get(3).unwrap_or(&"0B")).unwrap_or(0);
                let created = Self::parse_docker_time(parts.get(4).unwrap_or(&""))
                    .unwrap_or(SystemTime::UNIX_EPOCH);

                let image = ImageInfo {
                    id: parts[0].to_string(),
                    repository: parts[1].to_string(),
                    tag: parts[2].to_string(),
                    size,
                    created,
                    used: false, // Would need additional check
                    dangling: parts[1] == "<none>",
                };
                images.push(image);
            }
        }

        Ok(images)
    }

    /// Get all volumes
    pub async fn get_volumes(&self) -> Result<Vec<VolumeInfo>> {
        if !self.daemon_running {
            return Ok(Vec::new());
        }

        let docker_path = self
            .docker_path
            .as_ref()
            .context("Docker executable not found")?;

        // Get volume list with format
        let output = process::Command::new(docker_path)
            .args([
                "volume",
                "ls",
                "--format",
                "{{.Name}}\t{{.Driver}}\t{{.Mountpoint}}\t{{.CreatedAt}}",
            ])
            .output()
            .await?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to list volumes: {}", error));
        }

        let mut volumes = Vec::new();
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if line.trim().is_empty() || line.starts_with("DRIVER") {
                continue;
            }

            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 {
                let volume = VolumeInfo {
                    name: parts[0].to_string(),
                    driver: parts[1].to_string(),
                    mount_point: PathBuf::from(parts[2]),
                    created: Self::parse_docker_time(parts.get(3).unwrap_or(&""))
                        .unwrap_or(SystemTime::UNIX_EPOCH),
                    size: None,             // Would need additional calculation
                    containers: Vec::new(), // Would need additional query
                };
                volumes.push(volume);
            }
        }

        Ok(volumes)
    }

    /// Get all networks
    pub async fn get_networks(&self) -> Result<Vec<NetworkInfo>> {
        if !self.daemon_running {
            return Ok(Vec::new());
        }

        let docker_path = self
            .docker_path
            .as_ref()
            .context("Docker executable not found")?;

        // Get network list with format
        let output = process::Command::new(docker_path)
            .args([
                "network",
                "ls",
                "--format",
                "{{.ID}}\t{{.Name}}\t{{.Driver}}\t{{.Scope}}\t{{.Internal}}",
            ])
            .output()
            .await?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to list networks: {}", error));
        }

        let mut networks = Vec::new();
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if line.trim().is_empty() || line.starts_with("NETWORK") {
                continue;
            }

            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 4 {
                let network = NetworkInfo {
                    id: parts[0].to_string(),
                    name: parts[1].to_string(),
                    driver: parts[2].to_string(),
                    scope: parts[3].to_string(),
                    internal: parts.get(4).map(|s| *s == "true").unwrap_or(false),
                    containers: Vec::new(), // Would need additional query
                    created: SystemTime::UNIX_EPOCH, // Not available in list
                };
                networks.push(network);
            }
        }

        Ok(networks)
    }

    /// Stop a container
    pub async fn stop_container(&self, container_id: &str) -> Result<()> {
        if !self.daemon_running {
            return Err(anyhow::anyhow!("Docker daemon is not running"));
        }

        let docker_path = self
            .docker_path
            .as_ref()
            .context("Docker executable not found")?;

        let output = process::Command::new(docker_path)
            .args(["stop", container_id])
            .output()
            .await?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            Err(anyhow::anyhow!("Failed to stop container: {}", error))
        } else {
            Ok(())
        }
    }

    /// Remove a container
    pub async fn remove_container(&self, container_id: &str, force: bool) -> Result<()> {
        if !self.daemon_running {
            return Err(anyhow::anyhow!("Docker daemon is not running"));
        }

        let docker_path = self
            .docker_path
            .as_ref()
            .context("Docker executable not found")?;

        let mut args = vec!["rm"];
        if force {
            args.push("-f");
        }
        args.push(container_id);

        let output = process::Command::new(docker_path)
            .args(args)
            .output()
            .await?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            Err(anyhow::anyhow!("Failed to remove container: {}", error))
        } else {
            Ok(())
        }
    }

    /// Remove an image
    pub async fn remove_image(&self, image_id: &str, force: bool) -> Result<()> {
        if !self.daemon_running {
            return Err(anyhow::anyhow!("Docker daemon is not running"));
        }

        let docker_path = self
            .docker_path
            .as_ref()
            .context("Docker executable not found")?;

        let mut args = vec!["rmi"];
        if force {
            args.push("-f");
        }
        args.push(image_id);

        let output = process::Command::new(docker_path)
            .args(args)
            .output()
            .await?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            Err(anyhow::anyhow!("Failed to remove image: {}", error))
        } else {
            Ok(())
        }
    }

    /// Remove a volume
    pub async fn remove_volume(&self, volume_name: &str, force: bool) -> Result<()> {
        if !self.daemon_running {
            return Err(anyhow::anyhow!("Docker daemon is not running"));
        }

        let docker_path = self
            .docker_path
            .as_ref()
            .context("Docker executable not found")?;

        let mut args = vec!["volume", "rm"];
        if force {
            args.push("-f");
        }
        args.push(volume_name);

        let output = process::Command::new(docker_path)
            .args(args)
            .output()
            .await?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            Err(anyhow::anyhow!("Failed to remove volume: {}", error))
        } else {
            Ok(())
        }
    }

    /// Remove a network
    pub async fn remove_network(&self, network_name: &str) -> Result<()> {
        if !self.daemon_running {
            return Err(anyhow::anyhow!("Docker daemon is not running"));
        }

        let docker_path = self
            .docker_path
            .as_ref()
            .context("Docker executable not found")?;

        let output = process::Command::new(docker_path)
            .args(["network", "rm", network_name])
            .output()
            .await?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            Err(anyhow::anyhow!("Failed to remove network: {}", error))
        } else {
            Ok(())
        }
    }

    /// Clean build cache
    pub async fn clean_build_cache(&self) -> Result<()> {
        if !self.daemon_running {
            return Err(anyhow::anyhow!("Docker daemon is not running"));
        }

        let docker_path = self
            .docker_path
            .as_ref()
            .context("Docker executable not found")?;

        let output = process::Command::new(docker_path)
            .args(["builder", "prune", "-f"])
            .output()
            .await?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            Err(anyhow::anyhow!("Failed to clean build cache: {}", error))
        } else {
            Ok(())
        }
    }

    /// Perform comprehensive cleanup
    pub async fn cleanup_all(&self, options: &CleanupOptions) -> Result<DockerCleanupResult> {
        let start_time = std::time::Instant::now();
        let mut result = DockerCleanupResult {
            containers_removed: 0,
            images_removed: 0,
            volumes_removed: 0,
            networks_removed: 0,
            build_cache_cleaned: false,
            space_freed: 0,
            errors: Vec::new(),
            duration_ms: 0,
        };

        // Stop and remove containers
        if options.remove_containers {
            let containers = self.get_containers().await?;
            for container in containers {
                if options.stop_running && matches!(container.status, ContainerStatus::Running) {
                    if let Err(e) = self.stop_container(&container.id).await {
                        result.errors.push(format!(
                            "Failed to stop container {}: {}",
                            container.name, e
                        ));
                        continue;
                    }
                }

                if let Err(e) = self.remove_container(&container.id, options.force).await {
                    result.errors.push(format!(
                        "Failed to remove container {}: {}",
                        container.name, e
                    ));
                } else {
                    result.containers_removed += 1;
                    // Add size calculation
                    if let Some(size) = container.size_rw {
                        result.space_freed += size;
                    }
                    if let Some(size) = container.size_root_fs {
                        result.space_freed += size;
                    }
                }
            }
        }

        // Remove images
        if options.remove_images {
            let images = self.get_images().await?;
            for image in images {
                if options.remove_dangling || !image.dangling {
                    if let Err(e) = self.remove_image(&image.id, options.force).await {
                        result.errors.push(format!(
                            "Failed to remove image {}: {}",
                            image.repository, e
                        ));
                    } else {
                        result.images_removed += 1;
                        result.space_freed += image.size;
                    }
                }
            }
        }

        // Remove volumes
        if options.remove_volumes {
            let volumes = self.get_volumes().await?;
            for volume in volumes {
                if let Err(e) = self.remove_volume(&volume.name, options.force).await {
                    result
                        .errors
                        .push(format!("Failed to remove volume {}: {}", volume.name, e));
                } else {
                    result.volumes_removed += 1;
                    // Would need to calculate volume size
                }
            }
        }

        // Remove networks
        if options.remove_networks {
            let networks = self.get_networks().await?;
            for network in networks {
                // Skip default networks
                if network.name == "bridge" || network.name == "host" || network.name == "none" {
                    continue;
                }

                if let Err(e) = self.remove_network(&network.name).await {
                    result
                        .errors
                        .push(format!("Failed to remove network {}: {}", network.name, e));
                } else {
                    result.networks_removed += 1;
                }
            }
        }

        // Clean build cache
        if options.clean_build_cache {
            if let Err(e) = self.clean_build_cache().await {
                result
                    .errors
                    .push(format!("Failed to clean build cache: {}", e));
            } else {
                result.build_cache_cleaned = true;
            }
        }

        result.duration_ms = start_time.elapsed().as_millis() as u64;
        Ok(result)
    }

    // Helper methods
    fn parse_container_status(status: &str) -> ContainerStatus {
        if status.starts_with("Up") {
            ContainerStatus::Running
        } else if status.starts_with("Exited") {
            ContainerStatus::Exited
        } else if status.starts_with("Paused") {
            ContainerStatus::Paused
        } else if status.starts_with("Restarting") {
            ContainerStatus::Restarting
        } else if status.starts_with("Removing") {
            ContainerStatus::Removing
        } else if status.starts_with("Dead") {
            ContainerStatus::Dead
        } else if status.starts_with("Created") {
            ContainerStatus::Created
        } else {
            ContainerStatus::Unknown
        }
    }

    fn parse_docker_time(time_str: &str) -> Option<SystemTime> {
        // Docker uses RFC3339 format
        chrono::DateTime::parse_from_rfc3339(time_str)
            .ok()
            .map(|dt| dt.with_timezone(&chrono::Utc).into())
    }

    fn parse_ports(ports_str: &str) -> Vec<PortMapping> {
        if ports_str.is_empty() {
            return Vec::new();
        }

        let mut ports = Vec::new();
        for port_part in ports_str.split(',') {
            let parts: Vec<&str> = port_part.trim().split(':').collect();
            if parts.len() >= 2 {
                if let Some(container_part) = parts.last() {
                    if let Some(slash_pos) = container_part.find('/') {
                        let container_port = match container_part[..slash_pos].parse::<u16>() {
                            Ok(p) => p,
                            Err(_) => continue,
                        };
                        let protocol = container_part[slash_pos + 1..].to_string();

                        let host_port = if parts.len() == 3 {
                            parts[1].parse::<u16>().ok()
                        } else {
                            None
                        };

                        ports.push(PortMapping {
                            container_port,
                            host_port,
                            protocol,
                        });
                    }
                }
            }
        }

        ports
    }

    fn parse_size(size_str: &str) -> Option<u64> {
        let size_str = size_str.trim();
        if size_str.is_empty() {
            return Some(0);
        }

        // Try 2-char unit first (kB, MB, GB), fall back to 1-char (B)
        let (num, unit) = if size_str.len() >= 2 {
            let (n, u) = size_str.split_at(size_str.len() - 2);
            if u == "kB" || u == "MB" || u == "GB" {
                (n, u)
            } else {
                size_str.split_at(size_str.len() - 1)
            }
        } else {
            size_str.split_at(size_str.len().saturating_sub(1))
        };

        let num: f64 = num.parse().ok()?;

        match unit {
            "B" => Some(num as u64),
            "kB" => Some((num * 1024.0) as u64),
            "MB" => Some((num * 1024.0 * 1024.0) as u64),
            "GB" => Some((num * 1024.0 * 1024.0 * 1024.0) as u64),
            _ => None,
        }
    }

    /// Get daemon status
    pub fn is_daemon_running(&self) -> bool {
        self.daemon_running
    }

    /// Get API version
    pub fn api_version(&self) -> &str {
        &self.api_version
    }
}

/// Cleanup options for Docker
#[derive(Debug, Clone)]
pub struct CleanupOptions {
    pub remove_containers: bool,
    pub remove_images: bool,
    pub remove_volumes: bool,
    pub remove_networks: bool,
    pub clean_build_cache: bool,
    pub stop_running: bool,
    pub remove_dangling: bool,
    pub force: bool,
}

impl Default for CleanupOptions {
    fn default() -> Self {
        Self {
            remove_containers: true,
            remove_images: true,
            remove_volumes: false, // Be careful with volumes
            remove_networks: true,
            clean_build_cache: true,
            stop_running: true,
            remove_dangling: true,
            force: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_docker_client_creation() {
        let client = DockerClient::new().await;
        assert!(client.is_ok());
    }

    #[test]
    fn test_parse_container_status() {
        assert_eq!(
            DockerClient::parse_container_status("Up 2 hours"),
            ContainerStatus::Running
        );
        assert_eq!(
            DockerClient::parse_container_status("Exited (0) 2 hours ago"),
            ContainerStatus::Exited
        );
        assert_eq!(
            DockerClient::parse_container_status("Paused"),
            ContainerStatus::Paused
        );
    }

    #[test]
    fn test_parse_size() {
        assert_eq!(DockerClient::parse_size("0B"), Some(0));
        assert_eq!(DockerClient::parse_size("1024B"), Some(1024));
        assert_eq!(DockerClient::parse_size("1MB"), Some(1024 * 1024));
    }
}
