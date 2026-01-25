// ABOUTME: MCP (Model Context Protocol) server configuration
// Manages MCP server definitions and installation

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Server name
    pub name: String,

    /// Server description
    pub description: String,

    /// Installation method
    pub installation: McpInstallation,

    /// Server definition for claude/gemini
    pub definition: McpServerDefinition,

    /// Required environment variables
    #[serde(default)]
    pub required_env: Vec<String>,

    /// Whether this server is enabled by default
    #[serde(default = "default_true")]
    pub enabled_by_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpInstallation {
    /// NPM package
    Npm {
        package: String,
        version: Option<String>,
    },

    /// Python package
    Python {
        package: String,
        version: Option<String>,
    },

    /// Git repository
    Git {
        url: String,
        branch: Option<String>,
        install_command: Option<String>,
    },

    /// Pre-installed (no action needed)
    PreInstalled,

    /// Custom installation script
    Custom { script: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpServerDefinition {
    /// Simple command-based server
    Command {
        command: String,
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
    },

    /// JSON-based configuration (for complex servers)
    Json { config: serde_json::Value },
}

fn default_true() -> bool {
    true
}

impl McpServerConfig {
    /// Create Serena MCP server config
    pub fn serena() -> Self {
        Self {
            name: "serena".to_string(),
            description: "AI coding agent that can work alongside you".to_string(),
            installation: McpInstallation::Npm {
                package: "@ambergristle/serena".to_string(),
                version: None,
            },
            definition: McpServerDefinition::Command {
                command: "node".to_string(),
                args: vec![
                    "/home/claude-user/.npm-global/lib/node_modules/@ambergristle/serena/out/index.js".to_string(),
                ],
                env: HashMap::new(),
            },
            required_env: vec![],
            enabled_by_default: true,
        }
    }

    /// Create Context7 MCP server config
    pub fn context7() -> Self {
        Self {
            name: "context7".to_string(),
            description: "Provides library documentation and code examples".to_string(),
            installation: McpInstallation::Npm {
                package: "context7".to_string(),
                version: None,
            },
            definition: McpServerDefinition::Command {
                command: "node".to_string(),
                args: vec![
                    "/home/claude-user/.npm-global/lib/node_modules/context7/index.js".to_string(),
                ],
                env: HashMap::new(),
            },
            required_env: vec![],
            enabled_by_default: true,
        }
    }

    /// Create Twilio MCP server config
    pub fn twilio() -> Self {
        Self {
            name: "twilio".to_string(),
            description: "Send SMS messages via Twilio".to_string(),
            installation: McpInstallation::Npm {
                package: "@twilio-labs/mcp-server-twilio".to_string(),
                version: None,
            },
            definition: McpServerDefinition::Json {
                config: serde_json::json!({
                    "command": "node",
                    "args": ["/home/claude-user/.npm-global/lib/node_modules/@twilio-labs/mcp-server-twilio/bin/run"],
                    "env": {
                        "TWILIO_AUTH_TOKEN": "${TWILIO_AUTH_TOKEN}",
                        "TWILIO_ACCOUNT_SID": "${TWILIO_ACCOUNT_SID}",
                        "TWILIO_FROM_PHONE": "${TWILIO_FROM_PHONE}"
                    }
                }),
            },
            required_env: vec![
                "TWILIO_AUTH_TOKEN".to_string(),
                "TWILIO_ACCOUNT_SID".to_string(),
                "TWILIO_FROM_PHONE".to_string(),
            ],
            enabled_by_default: false,
        }
    }

    /// Get default MCP server configurations
    pub fn defaults() -> HashMap<String, Self> {
        let mut servers = HashMap::new();

        let serena = Self::serena();
        servers.insert(serena.name.clone(), serena);

        let context7 = Self::context7();
        servers.insert(context7.name.clone(), context7);

        let twilio = Self::twilio();
        servers.insert(twilio.name.clone(), twilio);

        servers
    }

    /// Check if required environment variables are set
    pub fn check_env(&self) -> Result<(), Vec<String>> {
        let missing: Vec<String> = self
            .required_env
            .iter()
            .filter(|var| std::env::var(var).is_err())
            .cloned()
            .collect();

        if missing.is_empty() {
            Ok(())
        } else {
            Err(missing)
        }
    }

    /// Generate installation script for this server
    pub fn installation_script(&self) -> String {
        match &self.installation {
            McpInstallation::Npm { package, version } => {
                if let Some(v) = version {
                    format!("npm install -g {}@{}", package, v)
                } else {
                    format!("npm install -g {}", package)
                }
            }
            McpInstallation::Python { package, version } => {
                if let Some(v) = version {
                    format!("pip install {}=={}", package, v)
                } else {
                    format!("pip install {}", package)
                }
            }
            McpInstallation::Git {
                url,
                branch,
                install_command,
            } => {
                let mut script = format!("git clone {}", url);
                if let Some(b) = branch {
                    script.push_str(&format!(" -b {}", b));
                }
                if let Some(cmd) = install_command {
                    script.push_str(&format!(" && {}", cmd));
                }
                script
            }
            McpInstallation::PreInstalled => "# Pre-installed".to_string(),
            McpInstallation::Custom { script } => script.clone(),
        }
    }

    /// Convert to MCP configuration format for Claude/Gemini
    pub fn to_mcp_config(&self) -> serde_json::Value {
        match &self.definition {
            McpServerDefinition::Command { command, args, env } => {
                let mut config = serde_json::json!({
                    "command": command,
                    "args": args,
                });

                if !env.is_empty() {
                    config["env"] = serde_json::json!(env);
                }

                config
            }
            McpServerDefinition::Json { config } => config.clone(),
        }
    }
}

/// Generate MCP servers configuration file content
pub fn generate_mcp_config_json(servers: &[McpServerConfig]) -> serde_json::Value {
    let mut mcp_servers = serde_json::Map::new();

    for server in servers {
        if server.enabled_by_default {
            mcp_servers.insert(server.name.clone(), server.to_mcp_config());
        }
    }

    serde_json::json!({
        "mcpServers": mcp_servers
    })
}

/// MCP initialization strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum McpInitStrategy {
    /// Initialize MCP servers inside each container
    PerContainer,

    /// Mount central MCP configuration from host
    CentralMount {
        /// Path to mount from (e.g., ~/.claude)
        host_path: String,
    },

    /// Hybrid: install servers in container but use central config
    Hybrid {
        /// Config path to mount
        config_path: String,
        /// Whether to merge with container config
        merge_configs: bool,
    },
}

impl Default for McpInitStrategy {
    fn default() -> Self {
        // Default to hybrid approach for flexibility
        Self::Hybrid {
            config_path: "~/.claude".to_string(),
            merge_configs: true,
        }
    }
}

// =============================================================================
// MCP Socket Pooling Types
// =============================================================================

/// MCP Definition for pooling configuration
///
/// This struct defines an MCP server that can be shared across sessions
/// via socket pooling. It contains the command, args, and pooling settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpDefinition {
    /// Command to execute (e.g., "npx", "node", "python")
    pub command: String,

    /// Arguments for the command
    #[serde(default)]
    pub args: Vec<String>,

    /// Environment variables to set
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Description of the MCP server
    #[serde(default)]
    pub description: String,

    /// Whether this MCP is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Whether to use socket pooling for this MCP
    /// If true, the MCP process is shared across sessions via Unix socket
    #[serde(default = "default_true")]
    pub pool: bool,

    /// Required environment variables (validation)
    #[serde(default)]
    pub required_env: Vec<String>,
}

impl McpDefinition {
    /// Create a new MCP definition
    pub fn new(command: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            command: command.into(),
            args,
            env: HashMap::new(),
            description: String::new(),
            enabled: true,
            pool: true,
            required_env: vec![],
        }
    }

    /// Create with pooling disabled (stdio mode)
    pub fn new_stdio(command: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            command: command.into(),
            args,
            env: HashMap::new(),
            description: String::new(),
            enabled: true,
            pool: false,
            required_env: vec![],
        }
    }

    /// Convert from Claude settings.json format
    pub fn from_claude_settings(name: &str, config: &serde_json::Value) -> Option<Self> {
        let command = config.get("command")?.as_str()?.to_string();
        let args: Vec<String> = config
            .get("args")
            .and_then(|a| a.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        let env: HashMap<String, String> = config
            .get("env")
            .and_then(|e| e.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        Some(Self {
            command,
            args,
            env,
            description: format!("Imported from Claude settings: {}", name),
            enabled: true,
            pool: true,
            required_env: vec![],
        })
    }
}

/// MCP Importer - Auto-imports MCP definitions from Claude settings
pub struct McpImporter;

impl McpImporter {
    /// Auto-import MCPs from Claude settings files
    ///
    /// Checks the following locations:
    /// 1. ~/.claude/settings.json (Claude Code CLI)
    /// 2. ~/Library/Application Support/Claude/claude_desktop_config.json (Claude Desktop - macOS)
    /// 3. ~/.config/claude/claude_desktop_config.json (Claude Desktop - Linux)
    pub fn auto_import() -> HashMap<String, McpDefinition> {
        let mut mcps = HashMap::new();

        if let Some(home) = dirs::home_dir() {
            // Claude Code CLI settings
            let claude_cli_settings = home.join(".claude").join("settings.json");
            if claude_cli_settings.exists() {
                if let Ok(imported) = Self::import_from_claude_settings(&claude_cli_settings) {
                    mcps.extend(imported);
                }
            }

            // Claude Desktop (macOS)
            #[cfg(target_os = "macos")]
            {
                let claude_desktop = home
                    .join("Library")
                    .join("Application Support")
                    .join("Claude")
                    .join("claude_desktop_config.json");
                if claude_desktop.exists() {
                    if let Ok(imported) = Self::import_from_claude_desktop(&claude_desktop) {
                        mcps.extend(imported);
                    }
                }
            }

            // Claude Desktop (Linux)
            #[cfg(target_os = "linux")]
            {
                let claude_desktop = home
                    .join(".config")
                    .join("claude")
                    .join("claude_desktop_config.json");
                if claude_desktop.exists() {
                    if let Ok(imported) = Self::import_from_claude_desktop(&claude_desktop) {
                        mcps.extend(imported);
                    }
                }
            }
        }

        mcps
    }

    /// Import MCPs from Claude CLI settings.json
    fn import_from_claude_settings(path: &std::path::Path) -> std::io::Result<HashMap<String, McpDefinition>> {
        let content = std::fs::read_to_string(path)?;
        let settings: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let mut mcps = HashMap::new();

        if let Some(mcp_servers) = settings.get("mcpServers").and_then(|s| s.as_object()) {
            for (name, config) in mcp_servers {
                if let Some(def) = McpDefinition::from_claude_settings(name, config) {
                    mcps.insert(name.clone(), def);
                }
            }
        }

        Ok(mcps)
    }

    /// Import MCPs from Claude Desktop config
    fn import_from_claude_desktop(path: &std::path::Path) -> std::io::Result<HashMap<String, McpDefinition>> {
        let content = std::fs::read_to_string(path)?;
        let config: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let mut mcps = HashMap::new();

        if let Some(mcp_servers) = config.get("mcpServers").and_then(|s| s.as_object()) {
            for (name, server_config) in mcp_servers {
                if let Some(def) = McpDefinition::from_claude_settings(name, server_config) {
                    mcps.insert(name.clone(), def);
                }
            }
        }

        Ok(mcps)
    }
}

/// MCP Catalog - Generates session-specific .mcp.json files
///
/// When socket pooling is enabled, MCPs are accessed via Unix sockets
/// instead of spawning new processes. This catalog generates the
/// appropriate .mcp.json configuration for each session.
pub struct McpCatalog;

impl McpCatalog {
    /// Generate session configuration for a worktree
    ///
    /// For pooled MCPs, generates `nc -U <socket_path>` commands.
    /// For non-pooled MCPs, falls back to original stdio configuration.
    pub fn generate_session_config(
        mcps: &HashMap<String, McpDefinition>,
        socket_dir: Option<&std::path::Path>,
    ) -> serde_json::Value {
        let mut mcp_servers = serde_json::Map::new();

        for (name, def) in mcps {
            if !def.enabled {
                continue;
            }

            let config = if def.pool {
                // Pooled MCP: use nc to connect to socket
                if let Some(dir) = socket_dir {
                    let socket_path = dir.join(format!("mcp-{}.sock", name));
                    serde_json::json!({
                        "command": "nc",
                        "args": ["-U", socket_path.to_string_lossy()],
                    })
                } else {
                    // No socket dir available, fall back to stdio
                    Self::create_stdio_config(def)
                }
            } else {
                // Non-pooled MCP: use stdio
                Self::create_stdio_config(def)
            };

            mcp_servers.insert(name.clone(), config);
        }

        serde_json::json!({
            "mcpServers": mcp_servers
        })
    }

    /// Create stdio configuration for an MCP
    fn create_stdio_config(def: &McpDefinition) -> serde_json::Value {
        let mut config = serde_json::json!({
            "command": def.command,
            "args": def.args,
        });

        if !def.env.is_empty() {
            config["env"] = serde_json::json!(def.env);
        }

        config
    }

    /// Write session configuration to a worktree
    ///
    /// Reads any existing .mcp.json and merges with the generated config,
    /// preserving project-specific MCP configurations.
    pub fn write_session_config(
        worktree_path: &std::path::Path,
        mcps: &HashMap<String, McpDefinition>,
        socket_dir: Option<&std::path::Path>,
    ) -> std::io::Result<()> {
        let mcp_json_path = worktree_path.join(".mcp.json");

        // Read existing config if present
        let mut existing_config: serde_json::Value = if mcp_json_path.exists() {
            let content = std::fs::read_to_string(&mcp_json_path)?;
            serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
        } else {
            serde_json::json!({})
        };

        // Generate pooled config
        let generated = Self::generate_session_config(mcps, socket_dir);

        // Merge: generated config takes precedence, but preserve project-specific MCPs
        if let (Some(existing_servers), Some(generated_servers)) = (
            existing_config.get_mut("mcpServers").and_then(|s| s.as_object_mut()),
            generated.get("mcpServers").and_then(|s| s.as_object()),
        ) {
            // Add generated servers, overwriting any with same name
            for (name, config) in generated_servers {
                existing_servers.insert(name.clone(), config.clone());
            }
        } else if let Some(generated_servers) = generated.get("mcpServers") {
            existing_config["mcpServers"] = generated_servers.clone();
        }

        // Write merged config
        let content = serde_json::to_string_pretty(&existing_config)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(&mcp_json_path, content)?;

        tracing::info!("Wrote MCP session config to {}", mcp_json_path.display());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serena_config() {
        let serena = McpServerConfig::serena();
        assert_eq!(serena.name, "serena");
        assert!(serena.enabled_by_default);
        assert!(serena.required_env.is_empty());
    }

    #[test]
    fn test_twilio_config() {
        let twilio = McpServerConfig::twilio();
        assert_eq!(twilio.name, "twilio");
        assert!(!twilio.enabled_by_default);
        assert_eq!(twilio.required_env.len(), 3);
    }

    #[test]
    fn test_installation_script() {
        let context7 = McpServerConfig::context7();
        let script = context7.installation_script();
        assert_eq!(script, "npm install -g context7");
    }

    #[test]
    fn test_mcp_config_generation() {
        let servers = vec![McpServerConfig::serena(), McpServerConfig::context7()];

        let config = generate_mcp_config_json(&servers);
        let mcp_servers = config["mcpServers"].as_object().unwrap();

        assert!(mcp_servers.contains_key("serena"));
        assert!(mcp_servers.contains_key("context7"));
    }
}
