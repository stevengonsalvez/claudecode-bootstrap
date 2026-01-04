// ABOUTME: Configuration management for agents-in-a-box
// Handles application config, container defaults, and MCP server definitions

#![allow(dead_code)]

use anyhow::{Context, Result};
use dirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub mod container;
pub mod mcp;
pub mod mcp_init;
pub mod presets;

pub use container::{ContainerTemplate, ContainerTemplateConfig};
pub use mcp::{McpInitStrategy, McpServerConfig};
pub use mcp_init::{McpInitResult, McpInitializer, apply_mcp_init_result};
pub use presets::{PermissionSet, PresetManager, RepositoryPreset, create_default_presets};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Application version
    #[serde(default = "default_version")]
    pub version: String,

    /// Default container template to use if none specified
    #[serde(default = "default_container_template")]
    pub default_container_template: String,

    /// Available container templates
    #[serde(default)]
    pub container_templates: HashMap<String, ContainerTemplate>,

    /// MCP server configurations
    #[serde(default)]
    pub mcp_servers: HashMap<String, McpServerConfig>,

    /// Global environment variables
    #[serde(default)]
    pub environment: HashMap<String, String>,

    /// Workspace defaults
    #[serde(default)]
    pub workspace_defaults: WorkspaceDefaults,

    /// UI preferences
    #[serde(default)]
    pub ui_preferences: UiPreferences,

    /// Docker configuration
    #[serde(default)]
    pub docker: DockerConfig,

    /// Tmux configuration
    #[serde(default)]
    pub tmux: TmuxConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceDefaults {
    /// Default branch prefix for new sessions
    #[serde(default = "default_branch_prefix")]
    pub branch_prefix: String,

    /// Whether to auto-detect workspaces on startup
    #[serde(default = "default_true")]
    pub auto_detect: bool,

    /// Paths to exclude from workspace scanning
    #[serde(default)]
    pub exclude_paths: Vec<String>,

    /// Additional paths to scan for git repositories
    /// These are added to the default paths (~/projects, ~/code, etc.)
    #[serde(default)]
    pub workspace_scan_paths: Vec<PathBuf>,

    /// Maximum number of repositories to show in search results (default: 500)
    #[serde(default = "default_max_repositories")]
    pub max_repositories: usize,
}

impl Default for WorkspaceDefaults {
    fn default() -> Self {
        Self {
            branch_prefix: default_branch_prefix(),
            auto_detect: default_true(),
            exclude_paths: Vec::new(),
            workspace_scan_paths: Vec::new(),
            max_repositories: default_max_repositories(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UiPreferences {
    /// Color theme
    #[serde(default = "default_theme")]
    pub theme: String,

    /// Whether to show container status in UI
    #[serde(default = "default_true")]
    pub show_container_status: bool,

    /// Whether to show git status in UI
    #[serde(default = "default_true")]
    pub show_git_status: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DockerConfig {
    /// Docker host connection string
    /// Examples:
    /// - unix:///var/run/docker.sock
    /// - tcp://localhost:2376
    /// - npipe:////./pipe/docker_engine
    pub host: Option<String>,

    /// Connection timeout in seconds
    #[serde(default = "default_docker_timeout")]
    pub timeout: u64,

    /// TLS configuration for TCP connections
    #[serde(default)]
    pub tls: Option<DockerTlsConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerTlsConfig {
    /// Path to CA certificate
    pub ca_cert: Option<String>,

    /// Path to client certificate
    pub client_cert: Option<String>,

    /// Path to client private key
    pub client_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TmuxConfig {
    /// Detach key combination (default: "ctrl-q")
    #[serde(default = "default_detach_key")]
    pub detach_key: String,

    /// Preview update interval in milliseconds (default: 100ms)
    #[serde(default = "default_update_interval")]
    pub preview_update_interval_ms: u64,

    /// Tmux history limit in lines (default: 10000)
    #[serde(default = "default_history_limit")]
    pub history_limit: u32,

    /// Enable mouse scrolling in tmux (default: true)
    #[serde(default = "default_mouse_scroll")]
    pub enable_mouse_scroll: bool,
}

fn default_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn default_container_template() -> String {
    "claude-dev".to_string()
}

fn default_branch_prefix() -> String {
    "agents/".to_string()
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_detach_key() -> String {
    "ctrl-q".to_string()
}

fn default_update_interval() -> u64 {
    100
}

fn default_history_limit() -> u32 {
    10000
}

fn default_mouse_scroll() -> bool {
    true
}

fn default_true() -> bool {
    true
}

fn default_docker_timeout() -> u64 {
    60
}

fn default_max_repositories() -> usize {
    500
}

impl AppConfig {
    /// Load configuration from default locations
    pub fn load() -> Result<Self> {
        // Try loading from multiple locations in order of precedence
        let config_paths = Self::get_config_paths();

        let mut config = Self::default();

        // Load each config file and merge
        for path in config_paths {
            if path.exists() {
                let content = fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read config from {}", path.display()))?;

                let file_config: AppConfig = toml::from_str(&content)
                    .with_context(|| format!("Failed to parse config from {}", path.display()))?;

                config.merge(file_config);
            }
        }

        // Load built-in container templates if none exist
        if config.container_templates.is_empty() {
            config.load_builtin_templates();
        }

        Ok(config)
    }

    /// Save configuration to user config directory
    pub fn save(&self) -> Result<()> {
        let config_dir = Self::get_user_config_dir()?;
        fs::create_dir_all(&config_dir)?;

        let config_path = config_dir.join("config.toml");
        let content = toml::to_string_pretty(self)?;
        fs::write(&config_path, content)?;

        Ok(())
    }

    /// Get configuration file paths in order of precedence
    fn get_config_paths() -> Vec<PathBuf> {
        let mut paths = vec![];

        // 1. Local project config
        if let Ok(cwd) = std::env::current_dir() {
            paths.push(cwd.join(".agents-box").join("config.toml"));
        }

        // 2. User config (~/.agents-in-a-box/config.toml)
        if let Ok(config_dir) = Self::get_user_config_dir() {
            paths.push(config_dir.join("config.toml"));
        }

        // 3. System config
        paths.push(PathBuf::from("/etc/agents-in-a-box/config.toml"));

        paths
    }

    /// Get user configuration directory
    fn get_user_config_dir() -> Result<PathBuf> {
        let home_dir = dirs::home_dir().context("Failed to get home directory")?;
        let config_dir = home_dir.join(".agents-in-a-box").join("config");
        Ok(config_dir)
    }

    /// Merge another config into this one
    fn merge(&mut self, other: AppConfig) {
        // Don't override version
        if !other.default_container_template.is_empty() {
            self.default_container_template = other.default_container_template;
        }

        // Merge maps
        self.container_templates.extend(other.container_templates);
        self.mcp_servers.extend(other.mcp_servers);
        self.environment.extend(other.environment);

        // Override workspace defaults if provided
        if other.workspace_defaults.branch_prefix != default_branch_prefix() {
            self.workspace_defaults.branch_prefix = other.workspace_defaults.branch_prefix;
        }
        self.workspace_defaults.auto_detect = other.workspace_defaults.auto_detect;
        if !other.workspace_defaults.exclude_paths.is_empty() {
            self.workspace_defaults.exclude_paths = other.workspace_defaults.exclude_paths;
        }
        if !other.workspace_defaults.workspace_scan_paths.is_empty() {
            self.workspace_defaults.workspace_scan_paths =
                other.workspace_defaults.workspace_scan_paths;
        }
        // Always take max_repositories from config if loaded from file
        self.workspace_defaults.max_repositories = other.workspace_defaults.max_repositories;

        // Override UI preferences
        if other.ui_preferences.theme != default_theme() {
            self.ui_preferences.theme = other.ui_preferences.theme;
        }
        self.ui_preferences.show_container_status = other.ui_preferences.show_container_status;
        self.ui_preferences.show_git_status = other.ui_preferences.show_git_status;
    }

    /// Load built-in container templates
    fn load_builtin_templates(&mut self) {
        // Claude development template (based on claude-docker)
        let claude_dev = ContainerTemplate::claude_dev_default();
        self.container_templates.insert("claude-dev".to_string(), claude_dev);

        // Basic templates
        let node_template = ContainerTemplate::node_default();
        self.container_templates.insert("node".to_string(), node_template);

        let python_template = ContainerTemplate::python_default();
        self.container_templates.insert("python".to_string(), python_template);

        let rust_template = ContainerTemplate::rust_default();
        self.container_templates.insert("rust".to_string(), rust_template);
    }

    /// Get a container template by name
    pub fn get_container_template(&self, name: &str) -> Option<&ContainerTemplate> {
        self.container_templates.get(name)
    }

    /// Get the default container template
    pub fn get_default_container_template(&self) -> Option<&ContainerTemplate> {
        self.container_templates.get(&self.default_container_template)
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut config = Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            default_container_template: default_container_template(),
            container_templates: HashMap::new(),
            mcp_servers: HashMap::new(),
            environment: HashMap::new(),
            workspace_defaults: WorkspaceDefaults::default(),
            ui_preferences: UiPreferences::default(),
            docker: DockerConfig::default(),
            tmux: TmuxConfig::default(),
        };

        // Load built-in templates
        config.load_builtin_templates();

        config
    }
}

/// Load configuration from environment
pub fn load_from_env() -> HashMap<String, String> {
    std::env::vars()
        .filter(|(k, _)| {
            k.starts_with("AGENTS_BOX_") || k.starts_with("CLAUDE_") || k.starts_with("ANTHROPIC_")
        })
        .collect()
}

/// Project-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// Container template to use for this project
    pub container_template: Option<String>,

    /// Custom container configuration
    pub container_config: Option<ContainerTemplateConfig>,

    /// Project-specific MCP servers
    #[serde(default)]
    pub mcp_servers: Vec<String>,

    /// Project-specific environment variables
    #[serde(default)]
    pub environment: HashMap<String, String>,

    /// Whether to mount ~/.claude directory
    #[serde(default = "default_true")]
    pub mount_claude_config: bool,

    /// Additional paths to mount from host
    #[serde(default)]
    pub additional_mounts: Vec<MountConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountConfig {
    pub host_path: String,
    pub container_path: String,
    #[serde(default)]
    pub read_only: bool,
}

impl ProjectConfig {
    /// Load project configuration from a directory
    pub fn load_from_dir(dir: &Path) -> Result<Option<Self>> {
        let config_path = dir.join(".agents-box").join("project.toml");
        if !config_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&config_path)?;
        let config: ProjectConfig = toml::from_str(&content)?;
        Ok(Some(config))
    }

    /// Save project configuration to a directory
    pub fn save_to_dir(&self, dir: &Path) -> Result<()> {
        let config_dir = dir.join(".agents-box");
        fs::create_dir_all(&config_dir)?;

        let config_path = config_dir.join("project.toml");
        let content = toml::to_string_pretty(self)?;
        fs::write(&config_path, content)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(config.default_container_template, "claude-dev");
        assert!(!config.container_templates.is_empty());
    }

    #[test]
    fn test_project_config_save_load() {
        let temp_dir = TempDir::new().unwrap();
        let project_config = ProjectConfig {
            container_template: Some("node".to_string()),
            container_config: None,
            mcp_servers: vec!["context7".to_string()],
            environment: HashMap::new(),
            mount_claude_config: true,
            additional_mounts: vec![],
        };

        project_config.save_to_dir(temp_dir.path()).unwrap();
        let loaded = ProjectConfig::load_from_dir(temp_dir.path()).unwrap().unwrap();

        assert_eq!(loaded.container_template, Some("node".to_string()));
        assert_eq!(loaded.mcp_servers, vec!["context7".to_string()]);
    }
}
