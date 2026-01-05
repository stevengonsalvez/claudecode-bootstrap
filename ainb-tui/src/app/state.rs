// ABOUTME: Application state management and view switching logic for agents-in-a-box TUI

#![allow(dead_code)]

use crate::app::SessionLoader;
use crate::claude::client::ClaudeChatManager;
use crate::claude::types::ClaudeStreamingEvent;
use crate::claude::{ClaudeApiClient, ClaudeMessage};
use crate::components::fuzzy_file_finder::FuzzyFileFinderState;
use crate::components::live_logs_stream::LogEntry;
use crate::config::AppConfig;
use crate::credentials;
use crate::docker::LogStreamingCoordinator;
use crate::models::{Session, Workspace};
use std::collections::HashMap;
use std::time::{Duration, Instant};

use chrono;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Text editor with cursor support for boss mode prompts
#[derive(Debug, Clone)]
pub struct TextEditor {
    lines: Vec<String>,
    cursor_line: usize,
    cursor_col: usize,
}

impl TextEditor {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor_line: 0,
            cursor_col: 0,
        }
    }

    pub fn from_string(text: &str) -> Self {
        let lines: Vec<String> = if text.is_empty() {
            vec![String::new()]
        } else {
            text.lines().map(|s| s.to_string()).collect()
        };

        Self {
            lines,
            cursor_line: 0,
            cursor_col: 0,
        }
    }

    pub fn to_string(&self) -> String {
        self.lines.join("\n")
    }

    pub fn is_empty(&self) -> bool {
        self.lines.len() == 1 && self.lines[0].is_empty()
    }

    pub fn insert_char(&mut self, ch: char) {
        if ch == '\n' {
            self.insert_newline();
        } else {
            let line = &mut self.lines[self.cursor_line];
            line.insert(self.cursor_col, ch);
            self.cursor_col += 1;
        }
    }

    pub fn insert_newline(&mut self) {
        let current_line = self.lines[self.cursor_line].clone();
        let (left, right) = current_line.split_at(self.cursor_col);

        self.lines[self.cursor_line] = left.to_string();
        self.lines.insert(self.cursor_line + 1, right.to_string());

        self.cursor_line += 1;
        self.cursor_col = 0;
    }

    pub fn backspace(&mut self) {
        if self.cursor_col > 0 {
            // Delete character before cursor
            self.lines[self.cursor_line].remove(self.cursor_col - 1);
            self.cursor_col -= 1;
        } else if self.cursor_line > 0 {
            // Join with previous line
            let current_line = self.lines.remove(self.cursor_line);
            self.cursor_line -= 1;
            self.cursor_col = self.lines[self.cursor_line].len();
            self.lines[self.cursor_line].push_str(&current_line);
        }
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_col = self.lines[self.cursor_line].len();
        }
    }

    pub fn move_cursor_right(&mut self) {
        if self.cursor_col < self.lines[self.cursor_line].len() {
            self.cursor_col += 1;
        } else if self.cursor_line < self.lines.len() - 1 {
            self.cursor_line += 1;
            self.cursor_col = 0;
        }
    }

    pub fn move_cursor_up(&mut self) {
        if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_col = self.cursor_col.min(self.lines[self.cursor_line].len());
        }
    }

    pub fn move_cursor_down(&mut self) {
        if self.cursor_line < self.lines.len() - 1 {
            self.cursor_line += 1;
            self.cursor_col = self.cursor_col.min(self.lines[self.cursor_line].len());
        }
    }

    pub fn move_to_line_start(&mut self) {
        self.cursor_col = 0;
    }

    pub fn move_to_line_end(&mut self) {
        self.cursor_col = self.lines[self.cursor_line].len();
    }

    pub fn insert_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        let mut lines = text.lines();

        // Insert first line of text at current cursor position
        if let Some(first_line) = lines.next() {
            self.lines[self.cursor_line].insert_str(self.cursor_col, first_line);
            self.cursor_col += first_line.len();
        }

        // Insert newlines and subsequent lines
        for line in lines {
            self.insert_newline();
            self.lines[self.cursor_line].insert_str(self.cursor_col, line);
            self.cursor_col += line.len();
        }
    }

    pub fn get_cursor_position(&self) -> (usize, usize) {
        (self.cursor_line, self.cursor_col)
    }

    pub fn get_lines(&self) -> &Vec<String> {
        &self.lines
    }

    pub fn move_cursor_to_end(&mut self) {
        if !self.lines.is_empty() {
            self.cursor_line = self.lines.len() - 1;
            self.cursor_col = self.lines[self.cursor_line].len();
        }
    }

    pub fn set_cursor_position(&mut self, line: usize, col: usize) {
        if line < self.lines.len() {
            self.cursor_line = line;
            self.cursor_col = col.min(self.lines[line].len());
        }
    }

    // Word movement methods
    pub fn move_cursor_word_forward(&mut self) {
        let current_line = &self.lines[self.cursor_line];

        // If at end of line, move to next line
        if self.cursor_col >= current_line.len() {
            if self.cursor_line < self.lines.len() - 1 {
                self.cursor_line += 1;
                self.cursor_col = 0;
                // Find first non-whitespace character
                let next_line = &self.lines[self.cursor_line];
                while self.cursor_col < next_line.len()
                    && next_line.chars().nth(self.cursor_col).unwrap().is_whitespace()
                {
                    self.cursor_col += 1;
                }
            }
            return;
        }

        let chars: Vec<char> = current_line.chars().collect();
        let mut pos = self.cursor_col;

        // Skip current word
        while pos < chars.len()
            && !chars[pos].is_whitespace()
            && chars[pos] != '.'
            && chars[pos] != ','
        {
            pos += 1;
        }

        // Skip whitespace
        while pos < chars.len() && chars[pos].is_whitespace() {
            pos += 1;
        }

        self.cursor_col = pos;
    }

    pub fn move_cursor_word_backward(&mut self) {
        // If at beginning of line, move to end of previous line
        if self.cursor_col == 0 {
            if self.cursor_line > 0 {
                self.cursor_line -= 1;
                self.cursor_col = self.lines[self.cursor_line].len();
            }
            return;
        }

        let current_line = &self.lines[self.cursor_line];
        let chars: Vec<char> = current_line.chars().collect();
        let mut pos = self.cursor_col.saturating_sub(1);

        // Skip whitespace backwards
        while pos > 0 && chars[pos].is_whitespace() {
            pos = pos.saturating_sub(1);
        }

        // Skip word backwards
        while pos > 0 && !chars[pos].is_whitespace() && chars[pos] != '.' && chars[pos] != ',' {
            pos = pos.saturating_sub(1);
        }

        // If we stopped on whitespace or punctuation, move forward one
        if pos > 0 && (chars[pos].is_whitespace() || chars[pos] == '.' || chars[pos] == ',') {
            pos += 1;
        }

        self.cursor_col = pos;
    }

    // Word deletion methods
    pub fn delete_word_forward(&mut self) {
        let current_line_text = self.lines[self.cursor_line].clone();
        let chars: Vec<char> = current_line_text.chars().collect();
        let start_pos = self.cursor_col;

        if start_pos >= chars.len() {
            return;
        }

        let mut end_pos = start_pos;

        // Skip current word
        while end_pos < chars.len()
            && !chars[end_pos].is_whitespace()
            && chars[end_pos] != '.'
            && chars[end_pos] != ','
        {
            end_pos += 1;
        }

        // Skip following whitespace
        while end_pos < chars.len() && chars[end_pos].is_whitespace() {
            end_pos += 1;
        }

        // Remove the text
        let before: String = chars[..start_pos].iter().collect();
        let after: String = chars[end_pos..].iter().collect();
        self.lines[self.cursor_line] = format!("{}{}", before, after);
    }

    pub fn delete_word_backward(&mut self) {
        if self.cursor_col == 0 {
            return;
        }

        let current_line_text = self.lines[self.cursor_line].clone();
        let chars: Vec<char> = current_line_text.chars().collect();
        let end_pos = self.cursor_col;
        let mut start_pos = end_pos.saturating_sub(1);

        // Skip whitespace backwards
        while start_pos > 0 && chars[start_pos].is_whitespace() {
            start_pos = start_pos.saturating_sub(1);
        }

        // Skip word backwards
        while start_pos > 0
            && !chars[start_pos].is_whitespace()
            && chars[start_pos] != '.'
            && chars[start_pos] != ','
        {
            start_pos = start_pos.saturating_sub(1);
        }

        // If we stopped on whitespace or punctuation, move forward one
        if start_pos > 0
            && (chars[start_pos].is_whitespace()
                || chars[start_pos] == '.'
                || chars[start_pos] == ',')
        {
            start_pos += 1;
        }

        // Remove the text
        let before: String = chars[..start_pos].iter().collect();
        let after: String = chars[end_pos..].iter().collect();
        self.lines[self.cursor_line] = format!("{}{}", before, after);
        self.cursor_col = start_pos;
    }
}

/// Notification system for TUI messages
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationType {
    Success,
    Error,
    Info,
    Warning,
}

#[derive(Debug, Clone)]
pub struct Notification {
    pub message: String,
    pub notification_type: NotificationType,
    pub created_at: Instant,
    pub duration: Duration,
}

impl Notification {
    pub fn success(message: String) -> Self {
        Self {
            message,
            notification_type: NotificationType::Success,
            created_at: Instant::now(),
            duration: Duration::from_secs(3),
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            message,
            notification_type: NotificationType::Error,
            created_at: Instant::now(),
            duration: Duration::from_secs(5),
        }
    }

    pub fn info(message: String) -> Self {
        Self {
            message,
            notification_type: NotificationType::Info,
            created_at: Instant::now(),
            duration: Duration::from_secs(3),
        }
    }

    pub fn warning(message: String) -> Self {
        Self {
            message,
            notification_type: NotificationType::Warning,
            created_at: Instant::now(),
            duration: Duration::from_secs(4),
        }
    }

    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.duration
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusedPane {
    Sessions, // Left pane - workspace/session list
    LiveLogs, // Right pane - live logs
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    HomeScreen,      // Default landing page with tile navigation
    AgentSelection,  // Choose agent provider and model
    Config,          // Settings and configuration
    Catalog,         // Browse marketplace/catalog
    Analytics,       // Usage statistics and cost tracking
    SessionList,
    Logs,
    Terminal,
    Help,
    NewSession,
    SearchWorkspace,
    NonGitNotification,
    AttachedTerminal,
    AuthSetup,  // New view for authentication setup
    ClaudeChat, // Claude chat popup overlay
    GitView,    // Git status and diff view
}

#[derive(Debug, Clone)]
pub struct ConfirmationDialog {
    pub title: String,
    pub message: String,
    pub confirm_action: ConfirmAction,
    pub selected_option: bool, // true = Yes, false = No
}

#[derive(Debug, Clone)]
pub enum ConfirmAction {
    DeleteSession(Uuid),
    KillOtherTmux(String), // Kill a non-agents-in-a-box tmux session by name
}

// ============================================================================
// Home Screen State
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeTile {
    Agents,    // Agent selection
    Catalog,   // Browse catalog/marketplace
    Config,    // Settings & presets
    Sessions,  // Session manager
    Stats,     // Analytics & usage
    Help,      // Docs & guides
}

impl HomeTile {
    pub fn all() -> Vec<HomeTile> {
        vec![
            HomeTile::Agents,
            HomeTile::Catalog,
            HomeTile::Config,
            HomeTile::Sessions,
            HomeTile::Stats,
            HomeTile::Help,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            HomeTile::Agents => "Agents",
            HomeTile::Catalog => "Catalog",
            HomeTile::Config => "Config",
            HomeTile::Sessions => "Sessions",
            HomeTile::Stats => "Stats",
            HomeTile::Help => "Help",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            HomeTile::Agents => "Select & Configure",
            HomeTile::Catalog => "Browse & Bootstrap",
            HomeTile::Config => "Settings & Presets",
            HomeTile::Sessions => "Manage Active",
            HomeTile::Stats => "Usage & Analytics",
            HomeTile::Help => "Docs & Guides",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            HomeTile::Agents => "🤖",
            HomeTile::Catalog => "📦",
            HomeTile::Config => "⚙️",
            HomeTile::Sessions => "🚀",
            HomeTile::Stats => "📊",
            HomeTile::Help => "❓",
        }
    }
}

#[derive(Debug, Clone)]
pub struct HomeScreenState {
    pub selected_tile: usize,
    pub tiles: Vec<HomeTile>,
}

impl Default for HomeScreenState {
    fn default() -> Self {
        Self {
            selected_tile: 0,
            tiles: HomeTile::all(),
        }
    }
}

impl HomeScreenState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn selected(&self) -> Option<&HomeTile> {
        self.tiles.get(self.selected_tile)
    }

    pub fn select_next(&mut self) {
        if !self.tiles.is_empty() {
            self.selected_tile = (self.selected_tile + 1) % self.tiles.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.tiles.is_empty() {
            self.selected_tile = if self.selected_tile == 0 {
                self.tiles.len() - 1
            } else {
                self.selected_tile - 1
            };
        }
    }

    pub fn select_right(&mut self) {
        // 2x3 grid: move right wraps within row
        let col = self.selected_tile % 3;
        let row = self.selected_tile / 3;
        let new_col = (col + 1) % 3;
        self.selected_tile = row * 3 + new_col;
    }

    pub fn select_left(&mut self) {
        // 2x3 grid: move left wraps within row
        let col = self.selected_tile % 3;
        let row = self.selected_tile / 3;
        let new_col = if col == 0 { 2 } else { col - 1 };
        self.selected_tile = row * 3 + new_col;
    }

    pub fn select_down(&mut self) {
        // 2x3 grid: move down wraps to top
        let col = self.selected_tile % 3;
        let row = self.selected_tile / 3;
        let new_row = (row + 1) % 2;
        self.selected_tile = new_row * 3 + col;
    }

    pub fn select_up(&mut self) {
        // 2x3 grid: move up wraps to bottom
        let col = self.selected_tile % 3;
        let row = self.selected_tile / 3;
        let new_row = if row == 0 { 1 } else { 0 };
        self.selected_tile = new_row * 3 + col;
    }
}

// ============================================================================
// Agent Selection State
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderStatus {
    Available,
    ComingSoon,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostTier {
    Low,
    Medium,
    High,
    Premium,
}

#[derive(Debug, Clone)]
pub struct AgentModel {
    pub name: String,
    pub description: String,
    pub cost_tier: CostTier,
    pub is_recommended: bool,
}

impl AgentModel {
    pub fn new(name: &str, description: &str, cost_tier: CostTier, is_recommended: bool) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            cost_tier,
            is_recommended,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AgentProvider {
    pub name: String,
    pub vendor: String,
    pub models: Vec<AgentModel>,
    pub status: ProviderStatus,
}

impl AgentProvider {
    pub fn claude() -> Self {
        Self {
            name: "Claude Code".to_string(),
            vendor: "Anthropic".to_string(),
            models: vec![
                AgentModel::new("Opus 4.5", "Best reasoning, complex tasks", CostTier::Premium, false),
                AgentModel::new("Sonnet 4.5", "Balanced (Recommended)", CostTier::High, true),
                AgentModel::new("Haiku 4.5", "Fast, lightweight", CostTier::Medium, false),
            ],
            status: ProviderStatus::Available,
        }
    }

    pub fn codex() -> Self {
        Self {
            name: "Codex CLI".to_string(),
            vendor: "OpenAI".to_string(),
            models: vec![
                AgentModel::new("GPT-5.2-Codex", "Latest Codex model", CostTier::High, true),
                AgentModel::new("o3-mini", "Reasoning focused", CostTier::Medium, false),
            ],
            status: ProviderStatus::ComingSoon,
        }
    }

    pub fn gemini() -> Self {
        Self {
            name: "Gemini CLI".to_string(),
            vendor: "Google".to_string(),
            models: vec![
                AgentModel::new("Gemini 2.0 Pro", "1M context window", CostTier::High, true),
            ],
            status: ProviderStatus::ComingSoon,
        }
    }

    pub fn local() -> Self {
        Self {
            name: "Local Models".to_string(),
            vendor: "Ollama".to_string(),
            models: vec![
                AgentModel::new("Configurable", "Self-hosted models", CostTier::Low, true),
            ],
            status: ProviderStatus::ComingSoon,
        }
    }

    pub fn all() -> Vec<AgentProvider> {
        vec![
            Self::claude(),
            Self::codex(),
            Self::gemini(),
            Self::local(),
        ]
    }
}

#[derive(Debug, Clone)]
pub struct AgentSelectionState {
    pub selected_provider: usize,
    pub selected_model: usize,
    pub providers: Vec<AgentProvider>,
    pub expanded_provider: Option<usize>, // Which provider is expanded to show models
}

impl Default for AgentSelectionState {
    fn default() -> Self {
        Self {
            selected_provider: 0,
            selected_model: 0,
            providers: AgentProvider::all(),
            expanded_provider: Some(0), // Claude expanded by default
        }
    }
}

impl AgentSelectionState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current_provider(&self) -> Option<&AgentProvider> {
        self.providers.get(self.selected_provider)
    }

    pub fn current_model(&self) -> Option<&AgentModel> {
        self.current_provider()
            .and_then(|p| p.models.get(self.selected_model))
    }

    pub fn select_next_provider(&mut self) {
        if !self.providers.is_empty() {
            self.selected_provider = (self.selected_provider + 1) % self.providers.len();
            self.selected_model = 0;
            self.expanded_provider = Some(self.selected_provider);
        }
    }

    pub fn select_prev_provider(&mut self) {
        if !self.providers.is_empty() {
            self.selected_provider = if self.selected_provider == 0 {
                self.providers.len() - 1
            } else {
                self.selected_provider - 1
            };
            self.selected_model = 0;
            self.expanded_provider = Some(self.selected_provider);
        }
    }

    pub fn select_next_model(&mut self) {
        if let Some(provider) = self.current_provider() {
            if !provider.models.is_empty() {
                self.selected_model = (self.selected_model + 1) % provider.models.len();
            }
        }
    }

    pub fn select_prev_model(&mut self) {
        if let Some(provider) = self.current_provider() {
            if !provider.models.is_empty() {
                self.selected_model = if self.selected_model == 0 {
                    provider.models.len() - 1
                } else {
                    self.selected_model - 1
                };
            }
        }
    }

    pub fn toggle_expand(&mut self) {
        if self.expanded_provider == Some(self.selected_provider) {
            self.expanded_provider = None;
        } else {
            self.expanded_provider = Some(self.selected_provider);
        }
    }

    pub fn is_current_available(&self) -> bool {
        self.current_provider()
            .map(|p| p.status == ProviderStatus::Available)
            .unwrap_or(false)
    }
}

// ============================================================================
// Configuration Screen State
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfigCategory {
    Authentication,
    Workspace,
    AgentDefaults,
    Plugins,
    Permissions,
    Appearance,
    Analytics,
}

impl ConfigCategory {
    pub fn all() -> Vec<ConfigCategory> {
        vec![
            ConfigCategory::Authentication,
            ConfigCategory::Workspace,
            ConfigCategory::AgentDefaults,
            ConfigCategory::Plugins,
            ConfigCategory::Permissions,
            ConfigCategory::Appearance,
            ConfigCategory::Analytics,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            ConfigCategory::Authentication => "Authentication",
            ConfigCategory::Workspace => "Workspace",
            ConfigCategory::AgentDefaults => "Agent Defaults",
            ConfigCategory::Plugins => "Plugins",
            ConfigCategory::Permissions => "Permissions",
            ConfigCategory::Appearance => "Appearance",
            ConfigCategory::Analytics => "Analytics",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            ConfigCategory::Authentication => "🔐",
            ConfigCategory::Workspace => "📁",
            ConfigCategory::AgentDefaults => "🤖",
            ConfigCategory::Plugins => "🔌",
            ConfigCategory::Permissions => "🛡️",
            ConfigCategory::Appearance => "🎨",
            ConfigCategory::Analytics => "📊",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ConfigCategory::Authentication => "API keys, OAuth, GitHub credentials",
            ConfigCategory::Workspace => "Default paths, git settings",
            ConfigCategory::AgentDefaults => "Model, temperature, max tokens",
            ConfigCategory::Plugins => "Installed plugins, enable/disable",
            ConfigCategory::Permissions => "File write, shell, git approval",
            ConfigCategory::Appearance => "Theme, colors, layout",
            ConfigCategory::Analytics => "Usage tracking, cost alerts",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfigSetting {
    pub key: String,
    pub label: String,
    pub value: ConfigValue,
    pub description: String,
}

#[derive(Debug, Clone)]
pub enum ConfigValue {
    Text(String),
    Secret(String),      // Masked display
    Bool(bool),
    Choice(Vec<String>, usize), // Options and selected index
    Number(i64),
}

impl ConfigValue {
    pub fn display(&self) -> String {
        match self {
            ConfigValue::Text(s) => s.clone(),
            ConfigValue::Secret(s) => {
                if s.is_empty() {
                    "Not configured".to_string()
                } else {
                    format!("{}••••••••", &s[..std::cmp::min(8, s.len())])
                }
            }
            ConfigValue::Bool(b) => if *b { "✓ Enabled" } else { "✗ Disabled" }.to_string(),
            ConfigValue::Choice(options, idx) => options.get(*idx).cloned().unwrap_or_default(),
            ConfigValue::Number(n) => n.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfigScreenState {
    pub selected_category: usize,
    pub selected_setting: usize,
    pub categories: Vec<ConfigCategory>,
    pub settings: std::collections::HashMap<ConfigCategory, Vec<ConfigSetting>>,
    pub editing: bool,
    pub edit_buffer: String,
    /// True when entering API key (special handling - saves to keychain)
    pub api_key_input_mode: bool,
}

impl Default for ConfigScreenState {
    fn default() -> Self {
        let mut settings = std::collections::HashMap::new();

        // Authentication settings
        // Determine current auth status for display
        let auth_status = match credentials::get_anthropic_api_key() {
            Ok(Some(key)) => {
                let masked = if key.len() > 12 {
                    format!("{}••••••••", &key[..12])
                } else {
                    "••••••••".to_string()
                };
                format!("API Key ({})", masked)
            }
            _ => "System Auth (Pro/Max Plan)".to_string(),
        };

        settings.insert(ConfigCategory::Authentication, vec![
            ConfigSetting {
                key: "claude_auth".to_string(),
                label: "Claude Authentication".to_string(),
                value: ConfigValue::Text(auth_status),
                description: "Press Enter to configure authentication provider".to_string(),
            },
            ConfigSetting {
                key: "github_auth".to_string(),
                label: "GitHub Credentials".to_string(),
                value: ConfigValue::Text("System Default".to_string()),
                description: "Uses git credential helper. PAT support coming soon.".to_string(),
            },
        ]);

        // Workspace settings
        settings.insert(ConfigCategory::Workspace, vec![
            ConfigSetting {
                key: "default_workspace".to_string(),
                label: "Default Workspace".to_string(),
                value: ConfigValue::Text("~/projects".to_string()),
                description: "Default directory for new sessions".to_string(),
            },
            ConfigSetting {
                key: "scan_depth".to_string(),
                label: "Scan Depth".to_string(),
                value: ConfigValue::Number(3),
                description: "How deep to scan for git repositories".to_string(),
            },
        ]);

        // Agent defaults
        settings.insert(ConfigCategory::AgentDefaults, vec![
            ConfigSetting {
                key: "default_model".to_string(),
                label: "Default Model".to_string(),
                value: ConfigValue::Choice(
                    vec!["Opus 4.5".to_string(), "Sonnet 4.5".to_string(), "Haiku 4.5".to_string()],
                    1, // Sonnet default
                ),
                description: "Default Claude model for new sessions".to_string(),
            },
            ConfigSetting {
                key: "auto_approve".to_string(),
                label: "Auto-Approve Actions".to_string(),
                value: ConfigValue::Bool(false),
                description: "Automatically approve file writes and commands".to_string(),
            },
        ]);

        // Permissions
        settings.insert(ConfigCategory::Permissions, vec![
            ConfigSetting {
                key: "allow_file_write".to_string(),
                label: "Allow File Write".to_string(),
                value: ConfigValue::Bool(true),
                description: "Allow agents to write files".to_string(),
            },
            ConfigSetting {
                key: "allow_shell".to_string(),
                label: "Allow Shell Commands".to_string(),
                value: ConfigValue::Bool(true),
                description: "Allow agents to run shell commands".to_string(),
            },
            ConfigSetting {
                key: "allow_git".to_string(),
                label: "Allow Git Operations".to_string(),
                value: ConfigValue::Bool(true),
                description: "Allow agents to perform git operations".to_string(),
            },
        ]);

        // Appearance
        settings.insert(ConfigCategory::Appearance, vec![
            ConfigSetting {
                key: "theme".to_string(),
                label: "Theme".to_string(),
                value: ConfigValue::Choice(
                    vec!["Dark".to_string(), "Light".to_string(), "System".to_string()],
                    0,
                ),
                description: "Color theme for the TUI".to_string(),
            },
        ]);

        // Plugins (empty for now)
        settings.insert(ConfigCategory::Plugins, vec![
            ConfigSetting {
                key: "installed_plugins".to_string(),
                label: "Installed Plugins".to_string(),
                value: ConfigValue::Text("None installed".to_string()),
                description: "Manage installed plugins from the Catalog".to_string(),
            },
        ]);

        // Analytics
        settings.insert(ConfigCategory::Analytics, vec![
            ConfigSetting {
                key: "track_usage".to_string(),
                label: "Track Usage".to_string(),
                value: ConfigValue::Bool(true),
                description: "Track session duration and token usage".to_string(),
            },
            ConfigSetting {
                key: "cost_alerts".to_string(),
                label: "Cost Alerts".to_string(),
                value: ConfigValue::Bool(false),
                description: "Alert when spending exceeds threshold".to_string(),
            },
        ]);

        Self {
            selected_category: 0,
            selected_setting: 0,
            categories: ConfigCategory::all(),
            settings,
            editing: false,
            edit_buffer: String::new(),
            api_key_input_mode: false,
        }
    }
}

impl ConfigScreenState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current_category(&self) -> Option<&ConfigCategory> {
        self.categories.get(self.selected_category)
    }

    pub fn current_settings(&self) -> Vec<&ConfigSetting> {
        self.current_category()
            .and_then(|cat| self.settings.get(cat))
            .map(|s| s.iter().collect())
            .unwrap_or_default()
    }

    pub fn current_setting(&self) -> Option<&ConfigSetting> {
        self.current_settings().get(self.selected_setting).copied()
    }

    pub fn select_next_category(&mut self) {
        if !self.categories.is_empty() {
            self.selected_category = (self.selected_category + 1) % self.categories.len();
            self.selected_setting = 0;
        }
    }

    pub fn select_prev_category(&mut self) {
        if !self.categories.is_empty() {
            self.selected_category = if self.selected_category == 0 {
                self.categories.len() - 1
            } else {
                self.selected_category - 1
            };
            self.selected_setting = 0;
        }
    }

    pub fn select_next_setting(&mut self) {
        let settings_count = self.current_settings().len();
        if settings_count > 0 {
            self.selected_setting = (self.selected_setting + 1) % settings_count;
        }
    }

    pub fn select_prev_setting(&mut self) {
        let settings_count = self.current_settings().len();
        if settings_count > 0 {
            self.selected_setting = if self.selected_setting == 0 {
                settings_count - 1
            } else {
                self.selected_setting - 1
            };
        }
    }

    pub fn toggle_current_setting(&mut self) {
        if let Some(category) = self.current_category().cloned() {
            if let Some(settings) = self.settings.get_mut(&category) {
                if let Some(setting) = settings.get_mut(self.selected_setting) {
                    match &mut setting.value {
                        ConfigValue::Bool(ref mut b) => *b = !*b,
                        ConfigValue::Choice(options, ref mut idx) => {
                            *idx = (*idx + 1) % options.len();
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// Create ConfigScreenState from AppConfig (loads persisted settings)
    pub fn from_app_config(config: &AppConfig) -> Self {
        let mut state = Self::default();

        // Update Authentication settings from config
        if let Some(settings) = state.settings.get_mut(&ConfigCategory::Authentication) {
            for setting in settings.iter_mut() {
                if setting.key == "claude_auth" {
                    // Build status text based on provider and API key presence
                    use crate::config::ClaudeAuthProvider;
                    let status = match &config.authentication.claude_provider {
                        ClaudeAuthProvider::ApiKey => {
                            let masked = credentials::get_anthropic_api_key_masked();
                            if masked == "Not configured" {
                                "API Key (Not configured)".to_string()
                            } else {
                                format!("API Key ({})", masked)
                            }
                        }
                        ClaudeAuthProvider::SystemAuth => "System Auth (Pro/Max Plan)".to_string(),
                        ClaudeAuthProvider::AmazonBedrock => "Amazon Bedrock [Coming Soon]".to_string(),
                        ClaudeAuthProvider::GoogleVertex => "Google Vertex [Coming Soon]".to_string(),
                        ClaudeAuthProvider::AzureFoundry => "Azure Foundry [Coming Soon]".to_string(),
                        ClaudeAuthProvider::GlmZai => "GLM on ZAI [Coming Soon]".to_string(),
                        ClaudeAuthProvider::LlmGateway => "LLM Gateway [Coming Soon]".to_string(),
                    };
                    setting.value = ConfigValue::Text(status);
                }
            }
        }

        // Update Workspace settings from config
        if let Some(settings) = state.settings.get_mut(&ConfigCategory::Workspace) {
            for setting in settings.iter_mut() {
                match setting.key.as_str() {
                    "default_workspace" => {
                        // Use first scan path or default
                        let path = config.workspace_defaults.workspace_scan_paths
                            .first()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "~/projects".to_string());
                        setting.value = ConfigValue::Text(path);
                    }
                    "scan_depth" => {
                        setting.value = ConfigValue::Number(3); // Not in AppConfig yet
                    }
                    _ => {}
                }
            }
        }

        // Update Agent Defaults from config
        if let Some(settings) = state.settings.get_mut(&ConfigCategory::AgentDefaults) {
            for setting in settings.iter_mut() {
                match setting.key.as_str() {
                    "auto_approve" => {
                        // Will be added to AppConfig
                        setting.value = ConfigValue::Bool(false);
                    }
                    _ => {}
                }
            }
        }

        // Update Appearance from config
        if let Some(settings) = state.settings.get_mut(&ConfigCategory::Appearance) {
            for setting in settings.iter_mut() {
                if setting.key == "theme" {
                    let theme_idx = match config.ui_preferences.theme.as_str() {
                        "dark" => 0,
                        "light" => 1,
                        "system" => 2,
                        _ => 0,
                    };
                    setting.value = ConfigValue::Choice(
                        vec!["Dark".to_string(), "Light".to_string(), "System".to_string()],
                        theme_idx,
                    );
                }
            }
        }

        // Update Analytics from config
        if let Some(settings) = state.settings.get_mut(&ConfigCategory::Analytics) {
            for setting in settings.iter_mut() {
                match setting.key.as_str() {
                    "track_usage" => {
                        setting.value = ConfigValue::Bool(true); // Default, not in AppConfig yet
                    }
                    "cost_alerts" => {
                        setting.value = ConfigValue::Bool(false); // Default, not in AppConfig yet
                    }
                    _ => {}
                }
            }
        }

        state
    }

    /// Convert ConfigScreenState back to AppConfig for saving
    pub fn apply_to_app_config(&self, config: &mut AppConfig) {
        // Apply Workspace settings
        if let Some(settings) = self.settings.get(&ConfigCategory::Workspace) {
            for setting in settings {
                match setting.key.as_str() {
                    "default_workspace" => {
                        if let ConfigValue::Text(path) = &setting.value {
                            let expanded = if path.starts_with("~/") {
                                dirs::home_dir()
                                    .map(|h| h.join(&path[2..]))
                                    .unwrap_or_else(|| std::path::PathBuf::from(path))
                            } else {
                                std::path::PathBuf::from(path)
                            };
                            // Add to scan paths if not already present
                            if !config.workspace_defaults.workspace_scan_paths.contains(&expanded) {
                                config.workspace_defaults.workspace_scan_paths.push(expanded);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // Apply Appearance settings
        if let Some(settings) = self.settings.get(&ConfigCategory::Appearance) {
            for setting in settings {
                if setting.key == "theme" {
                    if let ConfigValue::Choice(options, idx) = &setting.value {
                        if let Some(theme) = options.get(*idx) {
                            config.ui_preferences.theme = theme.to_lowercase();
                        }
                    }
                }
            }
        }

        // Apply Permissions settings
        if let Some(settings) = self.settings.get(&ConfigCategory::Permissions) {
            for setting in settings {
                match setting.key.as_str() {
                    "allow_file_write" | "allow_shell" | "allow_git" => {
                        // These would be added to AppConfig in future
                    }
                    _ => {}
                }
            }
        }
    }
}

// Auth provider option for the popup
#[derive(Debug, Clone)]
pub struct AuthProviderOption {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub description: String,
    pub available: bool,
    pub is_current: bool,
}

impl AuthProviderOption {
    pub fn new(id: &str, name: &str, icon: &str, desc: &str, available: bool) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            icon: icon.to_string(),
            description: desc.to_string(),
            available,
            is_current: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuthProviderPopupState {
    pub providers: Vec<AuthProviderOption>,
    pub selected_index: usize,
    pub is_entering_key: bool,
    pub api_key_input: String,
    pub show_popup: bool,
}

impl Default for AuthProviderPopupState {
    fn default() -> Self {
        // Check current API key status to mark current provider
        let has_api_key = credentials::get_anthropic_api_key()
            .map(|opt| opt.is_some())
            .unwrap_or(false);

        let mut providers = vec![
            AuthProviderOption::new(
                "system",
                "System Auth (Pro/Max Plan)",
                "",
                "Uses 'claude auth' - for Anthropic Pro/Max subscribers",
                true,
            ),
            AuthProviderOption::new(
                "api_key",
                "API Key (Pay-as-you-go)",
                "",
                "Set ANTHROPIC_API_KEY environment variable for pay-per-use",
                true,
            ),
            AuthProviderOption::new(
                "bedrock",
                "Amazon Bedrock",
                "",
                "Use Claude via AWS Bedrock service",
                false, // Coming soon
            ),
            AuthProviderOption::new(
                "vertex",
                "Google Vertex AI",
                "",
                "Use Claude via Google Cloud Vertex AI",
                false, // Coming soon
            ),
            AuthProviderOption::new(
                "azure",
                "Microsoft Azure Foundry",
                "",
                "Use Claude via Azure AI services",
                false, // Coming soon
            ),
            AuthProviderOption::new(
                "glm",
                "GLM on ZAI",
                "",
                "Use GLM models via ZAI platform",
                false, // Coming soon
            ),
            AuthProviderOption::new(
                "gateway",
                "LLM Gateway",
                "",
                "Use custom LLM gateway endpoint",
                false, // Coming soon
            ),
        ];

        // Mark current provider
        if has_api_key {
            if let Some(p) = providers.iter_mut().find(|p| p.id == "api_key") {
                p.is_current = true;
            }
        } else {
            if let Some(p) = providers.iter_mut().find(|p| p.id == "system") {
                p.is_current = true;
            }
        }

        Self {
            providers,
            selected_index: 0,
            is_entering_key: false,
            api_key_input: String::new(),
            show_popup: false,
        }
    }
}

impl AuthProviderPopupState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn select_next(&mut self) {
        if !self.providers.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.providers.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.providers.is_empty() {
            self.selected_index = if self.selected_index == 0 {
                self.providers.len() - 1
            } else {
                self.selected_index - 1
            };
        }
    }

    pub fn current_provider(&self) -> Option<&AuthProviderOption> {
        self.providers.get(self.selected_index)
    }

    pub fn is_api_key_selected(&self) -> bool {
        self.current_provider().map(|p| p.id == "api_key").unwrap_or(false)
    }

    pub fn start_key_input(&mut self) {
        self.is_entering_key = true;
        self.api_key_input.clear();
    }

    pub fn cancel_key_input(&mut self) {
        self.is_entering_key = false;
        self.api_key_input.clear();
    }

    /// Create AuthProviderPopupState with current provider marked based on config
    pub fn from_app_config(config: &crate::config::AppConfig) -> Self {
        use crate::config::ClaudeAuthProvider;

        let mut state = Self::default();

        // Clear any auto-detected current flags
        for provider in &mut state.providers {
            provider.is_current = false;
        }

        // Mark the provider from config as current
        let provider_id = match &config.authentication.claude_provider {
            ClaudeAuthProvider::SystemAuth => "system",
            ClaudeAuthProvider::ApiKey => "api_key",
            ClaudeAuthProvider::AmazonBedrock => "amazon_bedrock",
            ClaudeAuthProvider::GoogleVertex => "google_vertex",
            ClaudeAuthProvider::AzureFoundry => "azure_foundry",
            ClaudeAuthProvider::GlmZai => "glm_zai",
            ClaudeAuthProvider::LlmGateway => "llm_gateway",
        };

        if let Some(p) = state.providers.iter_mut().find(|p| p.id == provider_id) {
            p.is_current = true;
        }

        state
    }

    /// Get the current provider ID (the one marked as is_current)
    pub fn get_current_provider_id(&self) -> Option<&str> {
        self.providers.iter()
            .find(|p| p.is_current)
            .map(|p| p.id.as_str())
    }

    pub fn refresh_providers(&mut self) {
        let has_api_key = credentials::get_anthropic_api_key()
            .map(|opt| opt.is_some())
            .unwrap_or(false);

        for p in &mut self.providers {
            p.is_current = false;
        }

        if has_api_key {
            if let Some(p) = self.providers.iter_mut().find(|p| p.id == "api_key") {
                p.is_current = true;
            }
        } else {
            if let Some(p) = self.providers.iter_mut().find(|p| p.id == "system") {
                p.is_current = true;
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthMethod {
    OAuth,
    ApiKey,
    Skip,
}

#[derive(Debug, Clone)]
pub struct AuthSetupState {
    pub selected_method: AuthMethod,
    pub api_key_input: String,
    pub is_processing: bool,
    pub error_message: Option<String>,
    pub show_cursor: bool,
}

#[derive(Debug, Clone)]
pub struct ClaudeChatState {
    pub messages: Vec<ClaudeMessage>,
    pub input_buffer: String,
    pub is_streaming: bool,
    pub current_streaming_response: Option<String>,
    pub associated_session_id: Option<Uuid>,
    pub total_tokens_used: u32,
    pub last_activity: chrono::DateTime<chrono::Utc>,
}

impl ClaudeChatState {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            input_buffer: String::new(),
            is_streaming: false,
            current_streaming_response: None,
            associated_session_id: None,
            total_tokens_used: 0,
            last_activity: chrono::Utc::now(),
        }
    }

    pub fn add_message(&mut self, message: ClaudeMessage) {
        self.messages.push(message);
        self.last_activity = chrono::Utc::now();
    }

    pub fn start_streaming(&mut self, user_message: String) {
        self.add_message(ClaudeMessage::user(user_message));
        self.is_streaming = true;
        self.current_streaming_response = Some(String::new());
        self.input_buffer.clear();
        self.last_activity = chrono::Utc::now();
    }

    pub fn append_streaming_response(&mut self, text: &str) {
        if let Some(ref mut response) = self.current_streaming_response {
            response.push_str(text);
        }
        self.last_activity = chrono::Utc::now();
    }

    pub fn finish_streaming(&mut self) {
        if let Some(response) = self.current_streaming_response.take() {
            self.add_message(ClaudeMessage::assistant(response));
        }
        self.is_streaming = false;
    }

    pub fn clear_input(&mut self) {
        self.input_buffer.clear();
    }

    pub fn add_char_to_input(&mut self, ch: char) {
        if !self.is_streaming {
            self.input_buffer.push(ch);
        }
    }

    pub fn backspace_input(&mut self) {
        if !self.is_streaming {
            self.input_buffer.pop();
        }
    }
}

#[derive(Debug)]
pub struct AppState {
    pub workspaces: Vec<Workspace>,
    pub selected_workspace_index: Option<usize>,
    pub selected_session_index: Option<usize>,
    pub expand_all_workspaces: bool, // When true, show all sessions across all workspaces
    pub current_view: View,
    pub should_quit: bool,
    pub logs: HashMap<Uuid, Vec<String>>,
    pub help_visible: bool,
    // New session creation state
    pub new_session_state: Option<NewSessionState>,
    // Async action processing
    pub pending_async_action: Option<AsyncAction>,
    // Flag to track if user cancelled during async operation
    pub async_operation_cancelled: bool,
    // Confirmation dialog state
    pub confirmation_dialog: Option<ConfirmationDialog>,
    // Flag to force UI refresh after workspace changes
    pub ui_needs_refresh: bool,

    // Claude chat visibility toggle
    pub claude_chat_visible: bool,

    // Focus management for panes
    pub focused_pane: FocusedPane,
    // Track if current directory is a git repository
    pub is_current_dir_git_repo: bool,
    // Track which session logs were last fetched to avoid unnecessary refetches
    pub last_logs_session_id: Option<Uuid>,
    // Track attached terminal state
    pub attached_session_id: Option<Uuid>,
    // Auth setup state
    pub auth_setup_state: Option<AuthSetupState>,
    // Track when logs were last updated for each session
    pub log_last_updated: HashMap<Uuid, std::time::Instant>,
    // Track the last time we checked for log updates globally
    pub last_log_check: Option<std::time::Instant>,
    // Track the last time we checked for OAuth token refresh
    pub last_token_refresh_check: Option<std::time::Instant>,
    // Claude chat integration
    pub claude_chat_state: Option<ClaudeChatState>,
    // Live logs from Docker containers
    pub live_logs: HashMap<Uuid, Vec<LogEntry>>,
    // Claude API client manager (when initialized)
    pub claude_manager: Option<ClaudeChatManager>,
    // Docker log streaming coordinator
    pub log_streaming_coordinator: Option<LogStreamingCoordinator>,
    // Channel sender for log streaming
    pub log_sender: Option<mpsc::UnboundedSender<(Uuid, LogEntry)>>,
    // Git view state
    pub git_view_state: Option<crate::components::GitViewState>,
    // Notification system
    pub notifications: Vec<Notification>,
    // Pending event to be processed in next loop iteration
    pub pending_event: Option<crate::app::events::AppEvent>,

    // Quick commit dialog state
    pub quick_commit_message: Option<String>, // None = not in quick commit mode, Some = message being entered
    pub quick_commit_cursor: usize,           // Cursor position in quick commit message

    // Tmux integration
    pub tmux_sessions: HashMap<Uuid, crate::tmux::TmuxSession>,
    pub preview_update_task: Option<tokio::task::JoinHandle<()>>,

    // Other tmux sessions (not managed by agents-in-a-box)
    pub other_tmux_sessions: Vec<crate::models::OtherTmuxSession>,
    pub other_tmux_expanded: bool,
    pub selected_other_tmux_index: Option<usize>,

    // AINB 2.0: Home screen and agent selection
    pub home_screen_state: HomeScreenState,
    pub agent_selection_state: AgentSelectionState,
    pub config_screen_state: ConfigScreenState,
    pub auth_provider_popup_state: AuthProviderPopupState,

    // Persistent configuration (saved to ~/.agents-in-a-box/config/config.toml)
    pub app_config: AppConfig,
}

#[derive(Debug)]
pub struct NewSessionState {
    pub available_repos: Vec<std::path::PathBuf>,
    pub filtered_repos: Vec<(usize, std::path::PathBuf)>, // (original_index, path)
    pub selected_repo_index: Option<usize>,
    pub branch_name: String,
    pub step: NewSessionStep,
    pub filter_text: String,
    pub is_current_dir_mode: bool, // true if creating session in current dir
    pub skip_permissions: bool,    // true to use --dangerously-skip-permissions flag
    pub mode: crate::models::SessionMode, // Interactive or Boss mode
    pub boss_prompt: TextEditor,   // The prompt text editor for boss mode execution
    pub file_finder: FuzzyFileFinderState, // Fuzzy file finder for @ symbol
    pub restart_session_id: Option<Uuid>, // If set, this is a restart operation
}

impl Default for NewSessionState {
    fn default() -> Self {
        Self {
            available_repos: vec![],
            filtered_repos: vec![],
            selected_repo_index: None,
            branch_name: String::new(),
            step: NewSessionStep::SelectRepo,
            filter_text: String::new(),
            is_current_dir_mode: false,
            skip_permissions: false,
            mode: crate::models::SessionMode::Interactive,
            boss_prompt: TextEditor::new(),
            file_finder: FuzzyFileFinderState::new(),
            restart_session_id: None,
        }
    }
}

impl NewSessionState {
    pub fn apply_filter(&mut self) {
        self.filtered_repos.clear();
        let filter_lower = self.filter_text.to_lowercase();

        for (idx, repo) in self.available_repos.iter().enumerate() {
            if let Some(folder_name) = repo.file_name() {
                if let Some(name_str) = folder_name.to_str() {
                    if name_str.to_lowercase().contains(&filter_lower) {
                        self.filtered_repos.push((idx, repo.clone()));
                    }
                }
            }
        }

        // Reset selection if current selection is out of bounds
        if let Some(idx) = self.selected_repo_index {
            if idx >= self.filtered_repos.len() {
                self.selected_repo_index = if self.filtered_repos.is_empty() {
                    None
                } else {
                    Some(0)
                };
            }
        } else if !self.filtered_repos.is_empty() {
            self.selected_repo_index = Some(0);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewSessionStep {
    SelectRepo,
    InputBranch,
    SelectMode,  // Choose between Interactive and Boss mode
    InputPrompt, // Enter prompt for Boss mode
    ConfigurePermissions,
    Creating,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AsyncAction {
    StartNewSession,        // Old - will be removed
    StartWorkspaceSearch,   // New - search all workspaces
    NewSessionInCurrentDir, // New - create session in current directory
    NewSessionNormal,       // New - create normal new session with mode selection
    CreateNewSession,
    DeleteSession(Uuid),       // New - delete session with container cleanup
    RefreshWorkspaces,         // Manual refresh of workspace data
    FetchContainerLogs(Uuid),  // Fetch container logs for a session
    AttachToContainer(Uuid),   // Attach to a container session
    AttachToTmuxSession(Uuid), // Attach to a tmux session
    KillContainer(Uuid),       // Kill container for a session
    AuthSetupOAuth,            // Run OAuth authentication setup
    AuthSetupApiKey,           // Save API key authentication
    ReauthenticateCredentials, // Re-authenticate Claude credentials
    RestartSession(Uuid),      // Restart a stopped session with new container
    CleanupOrphaned,           // Clean up orphaned containers without worktrees
    AttachToOtherTmux(String), // Attach to a non-agents-in-a-box tmux session by name
    KillOtherTmux(String),     // Kill a non-agents-in-a-box tmux session by name
}

impl Default for AppState {
    fn default() -> Self {
        // Load persistent configuration
        let app_config = AppConfig::load().unwrap_or_else(|e| {
            warn!("Failed to load config, using defaults: {}", e);
            AppConfig::default()
        });

        Self {
            workspaces: Vec::new(),
            selected_workspace_index: None,
            selected_session_index: None,
            expand_all_workspaces: true, // Default to expanded view
            current_view: View::HomeScreen,
            should_quit: false,
            logs: HashMap::new(),
            help_visible: false,
            new_session_state: None,
            pending_async_action: None,
            async_operation_cancelled: false,
            confirmation_dialog: None,
            ui_needs_refresh: false,
            claude_chat_visible: false,
            focused_pane: FocusedPane::Sessions,
            is_current_dir_git_repo: false,
            last_logs_session_id: None,
            attached_session_id: None,
            auth_setup_state: None,
            log_last_updated: HashMap::new(),
            last_log_check: None,
            last_token_refresh_check: None,
            claude_chat_state: None,
            live_logs: HashMap::new(),
            claude_manager: None,
            log_streaming_coordinator: None,
            log_sender: None,
            git_view_state: None,
            notifications: Vec::new(),
            pending_event: None,

            // Initialize quick commit state
            quick_commit_message: None,
            quick_commit_cursor: 0,

            // Initialize tmux integration
            tmux_sessions: HashMap::new(),
            preview_update_task: None,

            // Initialize other tmux sessions
            other_tmux_sessions: Vec::new(),
            other_tmux_expanded: true, // Default to expanded
            selected_other_tmux_index: None,

            // AINB 2.0: Home screen and agent selection
            home_screen_state: HomeScreenState::default(),
            agent_selection_state: AgentSelectionState::default(),
            config_screen_state: ConfigScreenState::from_app_config(&app_config),
            auth_provider_popup_state: AuthProviderPopupState::from_app_config(&app_config),

            // Persistent configuration
            app_config,
        }
    }
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Initialize Claude integration if authentication is available
    pub async fn init_claude_integration(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        match ClaudeApiClient::load_auth_from_config() {
            Ok(auth) => {
                info!("Initializing Claude API integration");
                match ClaudeApiClient::with_auth(auth) {
                    Ok(client) => {
                        // Test connection
                        match client.test_connection().await {
                            Ok(()) => {
                                let mut manager = ClaudeChatManager::new(client);
                                manager.create_session(None);
                                self.claude_manager = Some(manager);
                                self.claude_chat_state = Some(ClaudeChatState::new());
                                info!("Claude integration initialized successfully");
                                Ok(())
                            }
                            Err(e) => {
                                warn!("Claude API connection test failed: {}", e);
                                Err(format!("Claude API connection failed: {}", e).into())
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to create Claude API client: {}", e);
                        Err(e.into())
                    }
                }
            }
            Err(e) => {
                info!("Claude authentication not configured: {}", e);
                // This is OK - user can set up auth later
                Ok(())
            }
        }
    }

    /// Send a message to Claude
    pub async fn send_claude_message(
        &mut self,
        message: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let (Some(chat_state), Some(manager)) =
            (&mut self.claude_chat_state, &mut self.claude_manager)
        {
            chat_state.start_streaming(message.clone());

            // Start streaming response
            match manager.stream_message(&message).await {
                Ok(mut stream) => {
                    // Handle streaming response
                    while let Some(event) = stream.next().await {
                        match event {
                            Ok(ClaudeStreamingEvent::ContentBlockDelta { delta, .. }) => {
                                chat_state.append_streaming_response(&delta.text);
                                self.ui_needs_refresh = true;
                            }
                            Ok(ClaudeStreamingEvent::MessageStop) => {
                                chat_state.finish_streaming();
                                self.ui_needs_refresh = true;
                                break;
                            }
                            Ok(ClaudeStreamingEvent::Error { error }) => {
                                error!("Claude API error: {}", error.message);
                                chat_state.finish_streaming();
                                return Err(format!("Claude error: {}", error.message).into());
                            }
                            Ok(_) => {
                                // Other events - continue
                            }
                            Err(e) => {
                                error!("Streaming error: {}", e);
                                chat_state.finish_streaming();
                                return Err(e.into());
                            }
                        }
                    }
                    Ok(())
                }
                Err(e) => {
                    chat_state.is_streaming = false;
                    Err(e.into())
                }
            }
        } else {
            Err("Claude integration not initialized".into())
        }
    }

    /// Add a log entry to live logs
    pub fn add_live_log(&mut self, session_id: Uuid, log_entry: LogEntry) {
        self.live_logs.entry(session_id).or_insert_with(Vec::new).push(log_entry);

        // Limit log entries to prevent memory issues (keep last 1000)
        if let Some(logs) = self.live_logs.get_mut(&session_id) {
            if logs.len() > 1000 {
                logs.drain(0..logs.len() - 1000);
            }
        }

        self.ui_needs_refresh = true;
    }

    /// Start log streaming for a session when it becomes active
    pub async fn start_log_streaming_for_session(
        &mut self,
        session_id: Uuid,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(coordinator) = &mut self.log_streaming_coordinator {
            // Find the session to get container info
            let session_info = self
                .workspaces
                .iter()
                .flat_map(|w| &w.sessions)
                .find(|s| s.id == session_id)
                .and_then(|s| {
                    s.container_id.clone().map(|container_id| {
                        (
                            container_id,
                            format!("{}-{}", s.name, s.branch_name),
                            s.mode.clone(),
                        )
                    })
                });

            if let Some((container_id, container_name, session_mode)) = session_info {
                info!(
                    "Starting log streaming for session {} (container: {})",
                    session_id, container_id
                );
                coordinator
                    .start_streaming(session_id, container_id, container_name, session_mode)
                    .await?;
            }
        }
        Ok(())
    }

    /// Stop log streaming for a session when it becomes inactive
    pub async fn stop_log_streaming_for_session(
        &mut self,
        session_id: Uuid,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(coordinator) = &mut self.log_streaming_coordinator {
            info!("Stopping log streaming for session {}", session_id);
            coordinator.stop_streaming(session_id).await?;
        }
        Ok(())
    }

    /// Clear live logs for a session
    pub fn clear_live_logs(&mut self, session_id: Uuid) {
        self.live_logs.remove(&session_id);
        self.ui_needs_refresh = true;
    }

    /// Get total live log count across all sessions
    pub fn total_live_log_count(&self) -> usize {
        self.live_logs.values().map(|logs| logs.len()).sum()
    }

    /// Check if this is first time setup (no auth configured)
    pub fn is_first_time_setup() -> bool {
        let home_dir = match dirs::home_dir() {
            Some(dir) => dir,
            None => return false,
        };

        let auth_dir = home_dir.join(".agents-in-a-box/auth");

        let has_credentials = auth_dir.join(".credentials.json").exists();
        let has_claude_json = auth_dir.join(".claude.json").exists();
        let has_api_key = std::env::var("ANTHROPIC_API_KEY").is_ok();
        let has_env_file = home_dir.join(".agents-in-a-box/.env").exists();

        // Load .env file if it exists to check for API key
        let has_env_api_key = if has_env_file {
            std::fs::read_to_string(home_dir.join(".agents-in-a-box/.env"))
                .map(|contents| contents.contains("ANTHROPIC_API_KEY="))
                .unwrap_or(false)
        } else {
            false
        };

        // For OAuth authentication, we need BOTH .credentials.json AND .claude.json
        // If we have a refresh token, we can refresh expired access tokens, so it's not "first time setup"
        let has_valid_oauth = if has_credentials && has_claude_json {
            // Check if we have OAuth credentials (either valid token OR refresh token to get new one)
            let credentials_path = auth_dir.join(".credentials.json");
            std::fs::read_to_string(&credentials_path)
                .ok()
                .and_then(|contents| serde_json::from_str::<serde_json::Value>(&contents).ok())
                .and_then(|json| json.get("claudeAiOauth").cloned())
                .map(|oauth| {
                    // If we have a refresh token, we can refresh even if access token is expired
                    oauth.get("refreshToken").is_some()
                        || Self::is_oauth_token_valid(&credentials_path)
                })
                .unwrap_or(false)
        } else {
            false
        };

        // Show auth screen if we don't have valid OAuth setup AND no API key alternatives
        !has_valid_oauth && !has_api_key && !has_env_api_key
    }

    /// Check if OAuth token in credentials file is still valid (not expired)
    fn is_oauth_token_valid(credentials_path: &std::path::Path) -> bool {
        use std::fs;

        if let Ok(contents) = fs::read_to_string(credentials_path) {
            // Parse the JSON to extract OAuth token info
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&contents) {
                if let Some(oauth) = json.get("claudeAiOauth") {
                    if let Some(expires_at) = oauth.get("expiresAt").and_then(|v| v.as_u64()) {
                        // Check if current time is before expiration time
                        let current_time = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;

                        if current_time < expires_at {
                            info!(
                                "OAuth token is valid, expires at: {}",
                                chrono::DateTime::from_timestamp_millis(expires_at as i64)
                                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                                    .unwrap_or_else(|| "unknown".to_string())
                            );
                            return true;
                        } else {
                            warn!(
                                "OAuth token has expired at: {}",
                                chrono::DateTime::from_timestamp_millis(expires_at as i64)
                                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                                    .unwrap_or_else(|| "unknown".to_string())
                            );
                            return false;
                        }
                    }
                }
            }
        }

        // If we can't parse or find expiration info, assume invalid
        warn!("Could not validate OAuth token from credentials file");
        false
    }

    /// Check if OAuth token needs refresh (expires within 30 minutes)
    fn oauth_token_needs_refresh(credentials_path: &std::path::Path) -> bool {
        use std::fs;

        if let Ok(contents) = fs::read_to_string(credentials_path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&contents) {
                if let Some(oauth) = json.get("claudeAiOauth") {
                    // Check if we have a refresh token
                    if oauth.get("refreshToken").is_none() {
                        info!("No refresh token available");
                        return false;
                    }

                    if let Some(expires_at) = oauth.get("expiresAt").and_then(|v| v.as_u64()) {
                        let current_time = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;

                        // Refresh if token expires in less than 30 minutes
                        let buffer_time = 30 * 60 * 1000; // 30 minutes in milliseconds

                        if current_time >= (expires_at.saturating_sub(buffer_time)) {
                            info!(
                                "OAuth token needs refresh, expires at: {}",
                                chrono::DateTime::from_timestamp_millis(expires_at as i64)
                                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                                    .unwrap_or_else(|| "unknown".to_string())
                            );
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    /// Refresh OAuth tokens using the refresh token
    pub async fn refresh_oauth_tokens(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Attempting to refresh OAuth tokens");

        let home_dir = dirs::home_dir().ok_or("Could not determine home directory")?;
        let auth_dir = home_dir.join(".agents-in-a-box").join("auth");
        let credentials_path = auth_dir.join(".credentials.json");

        // Check if tokens actually need refresh
        if !Self::oauth_token_needs_refresh(&credentials_path) {
            info!("OAuth tokens do not need refresh yet");
            return Ok(());
        }

        // Build the Docker image if needed
        let image_name = "agents-box:agents-dev";
        let image_check = tokio::process::Command::new("docker")
            .args(["image", "inspect", image_name])
            .output()
            .await?;

        if !image_check.status.success() {
            info!("Building agents-dev image for token refresh...");
            let build_status = tokio::process::Command::new("docker")
                .args(["build", "-t", image_name, "docker/agents-dev"])
                .status()
                .await?;

            if !build_status.success() {
                return Err("Failed to build image for token refresh".into());
            }
        }

        // Run the oauth-refresh.js script in a container (with retries built-in)
        info!("Running OAuth token refresh in container");

        // Create the volume mount string that will live long enough
        let volume_mount = format!("{}:/home/claude-user/.claude", auth_dir.display());

        // Build args based on debug mode
        let mut args = vec![
            "run",
            "--rm",
            "-v",
            &volume_mount,
            "-e",
            "PATH=/home/claude-user/.npm-global/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
            "-e",
            "HOME=/home/claude-user",
        ];

        // Add debug env if needed
        // Check if we're in debug mode by checking RUST_LOG env var
        if std::env::var("RUST_LOG").unwrap_or_default().contains("debug") {
            args.push("-e");
            args.push("DEBUG=1");
        }

        args.extend([
            "-w",
            "/home/claude-user",
            "--user",
            "claude-user",
            "--entrypoint",
            "node",
            image_name,
            "/app/scripts/oauth-refresh.js",
        ]);

        let output = tokio::process::Command::new("docker").args(&args).output().await?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            info!("OAuth token refresh successful: {}", stdout.trim());

            // Verify the new token is valid
            if Self::is_oauth_token_valid(&credentials_path) {
                info!("New OAuth token verified as valid");
                Ok(())
            } else {
                Err("Token refresh succeeded but new token is invalid".into())
            }
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            warn!("OAuth token refresh failed");
            warn!("Stderr: {}", stderr.trim());
            warn!("Stdout: {}", stdout.trim());
            Err(format!("Token refresh failed: {}", stderr.trim()).into())
        }
    }

    pub fn check_current_directory_status(&mut self) {
        use crate::git::workspace_scanner::WorkspaceScanner;
        use std::env;

        if let Ok(current_dir) = env::current_dir() {
            self.is_current_dir_git_repo =
                WorkspaceScanner::validate_workspace(&current_dir).unwrap_or(false);

            if !self.is_current_dir_git_repo {
                info!(
                    "Current directory is not a git repository: {:?} - auto-triggering workspace search",
                    current_dir
                );
                // Auto-trigger workspace search when not in a git repo
                self.pending_async_action = Some(AsyncAction::StartWorkspaceSearch);
                self.async_operation_cancelled = false;
            } else {
                info!(
                    "Current directory is a valid git repository: {:?}",
                    current_dir
                );
            }
        } else {
            warn!("Could not determine current directory - auto-triggering workspace search");
            self.is_current_dir_git_repo = false;
            // Auto-trigger workspace search
            self.pending_async_action = Some(AsyncAction::StartWorkspaceSearch);
            self.async_operation_cancelled = false;
        }
    }

    pub async fn load_real_workspaces(&mut self) {
        info!("Loading active sessions (both Docker and Interactive)");

        // Clear existing workspaces before loading to prevent duplicates
        self.workspaces.clear();

        // Check and refresh OAuth tokens if needed (only if Docker is available)
        let home_dir = dirs::home_dir();
        if let Some(home) = home_dir {
            let credentials_path =
                home.join(".agents-in-a-box").join("auth").join(".credentials.json");

            // Only attempt refresh if we have OAuth credentials AND Docker is available
            if credentials_path.exists() && Self::oauth_token_needs_refresh(&credentials_path) {
                if self.is_docker_available().await {
                    info!("Docker available - attempting OAuth token refresh");
                    match self.refresh_oauth_tokens().await {
                        Ok(()) => info!("OAuth tokens refreshed successfully"),
                        Err(e) => warn!("Failed to refresh OAuth tokens: {}", e),
                    }
                } else {
                    info!("Docker not available - skipping OAuth token refresh");
                }
            }
        }

        // Load Boss mode sessions (Docker-based) if Docker is available
        if self.is_docker_available().await {
            info!("Docker available - loading Boss mode sessions");
            self.load_boss_mode_sessions().await;
        } else {
            info!("Docker not available - skipping Boss mode session loading");
        }

        // Load Interactive mode sessions (always attempt, no Docker needed)
        info!("Loading Interactive mode sessions");
        self.load_interactive_mode_sessions().await;

        // Load other tmux sessions (not managed by agents-in-a-box)
        info!("Loading other tmux sessions");
        self.load_other_tmux_sessions().await;

        // Set initial selection
        if !self.workspaces.is_empty() {
            self.selected_workspace_index = Some(0);
            if !self.workspaces[0].sessions.is_empty() {
                self.selected_session_index = Some(0);
            }
        } else {
            info!("No active sessions found. Use 'n' to create a new session.");
            self.selected_workspace_index = None;
            self.selected_session_index = None;
        }

        // Queue logs fetch for the currently selected session if any
        self.queue_logs_fetch();
    }

    /// Load Boss mode sessions from Docker containers
    async fn load_boss_mode_sessions(&mut self) {
        // Try to load active Docker sessions
        match SessionLoader::new().await {
            Ok(loader) => {
                match loader.load_active_sessions().await {
                    Ok(mut workspaces) => {
                        // Append to existing workspaces instead of replacing
                        self.workspaces.append(&mut workspaces);
                        info!(
                            "Loaded {} Boss mode workspaces (total: {})",
                            workspaces.len(),
                            self.workspaces.len()
                        );
                    }
                    Err(e) => {
                        warn!("Failed to load Boss mode sessions: {}", e);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to create session loader for Boss mode: {}", e);
            }
        }
    }

    /// Load Interactive mode sessions from tmux
    async fn load_interactive_mode_sessions(&mut self) {
        use crate::interactive::InteractiveSessionManager;

        // Create Interactive session manager (no Docker needed)
        let mut manager = match InteractiveSessionManager::new() {
            Ok(m) => m,
            Err(e) => {
                warn!("Failed to create Interactive session manager: {}", e);
                return;
            }
        };

        // Discover Interactive sessions from tmux
        match manager.list_sessions().await {
            Ok(sessions) => {
                info!("Discovered {} Interactive sessions from tmux", sessions.len());

                // Convert to Session models and add to workspaces
                for interactive_session in sessions {
                    let session = interactive_session.to_session_model();

                    // Find or create workspace for this session
                    // Use source_repository (the original git repo) not worktree_path parent
                    let workspace_path = &interactive_session.source_repository;

                    // Remove any stale entries for this session (e.g., added by Boss-mode loader)
                    for workspace in &mut self.workspaces {
                        workspace.sessions.retain(|s| s.id != interactive_session.session_id);
                    }

                    if let Some(workspace) = self.workspaces.iter_mut().find(|w| {
                        std::path::Path::new(&w.path).canonicalize().ok()
                            == workspace_path.canonicalize().ok()
                    }) {
                        // Add to existing workspace
                        workspace.sessions.push(session);
                    } else {
                        // Create new workspace
                        let mut workspace = crate::models::Workspace::new(
                            interactive_session.workspace_name.clone(),
                            workspace_path.to_path_buf(),
                        );
                        workspace.sessions.push(session);
                        self.workspaces.push(workspace);
                    }

                    // Store tmux session for attach operations
                    // Pass branch name (NOT tmux-prefixed name) to TmuxSession::new()
                    // because TmuxSession::sanitize_name() will add the tmux_ prefix
                    let tmux_session = crate::tmux::TmuxSession::new(
                        interactive_session.branch_name.clone(),
                        "claude".to_string(),
                    );
                    self.tmux_sessions.insert(interactive_session.session_id, tmux_session);
                }
            }
            Err(e) => {
                warn!("Failed to discover Interactive sessions: {}", e);
            }
        }
    }

    /// Discover tmux sessions that are NOT managed by agents-in-a-box
    /// These are sessions without the "tmux_" prefix
    pub async fn load_other_tmux_sessions(&mut self) {
        use tokio::process::Command;
        use crate::models::OtherTmuxSession;

        info!("Discovering other tmux sessions");

        // Get all tmux sessions with format: name:attached:windows
        let output = match Command::new("tmux")
            .args(["list-sessions", "-F", "#{session_name}:#{session_attached}:#{session_windows}"])
            .output()
            .await
        {
            Ok(o) => o,
            Err(e) => {
                debug!("Failed to list tmux sessions: {} (tmux might not be running)", e);
                self.other_tmux_sessions.clear();
                return;
            }
        };

        if !output.status.success() {
            debug!("No tmux sessions found (tmux might not be running)");
            self.other_tmux_sessions.clear();
            return;
        }

        let sessions_output = String::from_utf8_lossy(&output.stdout);
        let mut other_sessions = Vec::new();

        for line in sessions_output.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 3 {
                // Session name may contain colons, so reconstruct from all parts except last two
                let name = parts[..parts.len() - 2].join(":");

                // Skip agents-in-a-box managed sessions (tmux_ prefix)
                if name.starts_with("tmux_") {
                    continue;
                }

                let attached = parts[parts.len() - 2] == "1";
                let windows = parts[parts.len() - 1].parse().unwrap_or_else(|e| {
                    warn!("Failed to parse window count for tmux session '{}': {}. Defaulting to 1.", name, e);
                    1
                });

                other_sessions.push(OtherTmuxSession::new(name, attached, windows));
            }
        }

        info!("Discovered {} other tmux sessions", other_sessions.len());
        self.other_tmux_sessions = other_sessions;
    }

    pub fn load_mock_data(&mut self) {
        let mut workspace1 = Workspace::new(
            "project1".to_string(),
            "/Users/user/projects/project1".into(),
        );

        let mut session1 = Session::new(
            "fix-auth".to_string(),
            workspace1.path.to_string_lossy().to_string(),
        );
        session1.set_status(crate::models::SessionStatus::Running);
        session1.git_changes.added = 42;
        session1.git_changes.deleted = 13;

        let mut session2 = Session::new(
            "add-feature".to_string(),
            workspace1.path.to_string_lossy().to_string(),
        );
        session2.set_status(crate::models::SessionStatus::Stopped);

        let mut session3 = Session::new(
            "debug-issue".to_string(),
            workspace1.path.to_string_lossy().to_string(),
        );
        session3.set_status(crate::models::SessionStatus::Error(
            "Container failed to start".to_string(),
        ));

        workspace1.add_session(session1);
        workspace1.add_session(session2);
        workspace1.add_session(session3);

        let mut workspace2 = Workspace::new(
            "project2".to_string(),
            "/Users/user/projects/project2".into(),
        );

        let mut session4 = Session::new(
            "refactor-api".to_string(),
            workspace2.path.to_string_lossy().to_string(),
        );
        session4.set_status(crate::models::SessionStatus::Running);
        session4.git_changes.modified = 7;

        workspace2.add_session(session4);

        self.workspaces.push(workspace1);
        self.workspaces.push(workspace2);

        if !self.workspaces.is_empty() {
            self.selected_workspace_index = Some(0);
            if !self.workspaces[0].sessions.is_empty() {
                self.selected_session_index = Some(0);
            }
        }
    }

    /// Load a large dataset to simulate the 353 repository scenario
    pub fn load_large_mock_data(&mut self) {
        // Load normal mock data first
        self.load_mock_data();

        // Add many more workspaces to simulate large dataset
        for i in 3..=200 {
            let workspace = Workspace::new(
                format!("test-project-{:03}", i),
                format!("/Users/user/projects/test-project-{:03}", i).into(),
            );
            self.workspaces.push(workspace);
        }

        info!(
            "Loaded large mock dataset with {} workspaces",
            self.workspaces.len()
        );
    }

    pub fn selected_session(&self) -> Option<&Session> {
        let workspace_idx = self.selected_workspace_index?;
        let session_idx = self.selected_session_index?;
        self.workspaces.get(workspace_idx)?.sessions.get(session_idx)
    }

    pub fn selected_workspace(&self) -> Option<&Workspace> {
        let workspace_idx = self.selected_workspace_index?;
        self.workspaces.get(workspace_idx)
    }

    pub fn next_session(&mut self) {
        // Check if we're in the "Other tmux" section
        if self.selected_other_tmux_index.is_some() {
            // Navigate within other tmux sessions
            let current = self.selected_other_tmux_index.unwrap_or(0);
            if current + 1 < self.other_tmux_sessions.len() {
                self.selected_other_tmux_index = Some(current + 1);
            }
            // At the end - stay at last item (no wrap)
            return;
        }

        if let Some(workspace_idx) = self.selected_workspace_index {
            if let Some(workspace) = self.workspaces.get(workspace_idx) {
                if !workspace.sessions.is_empty() {
                    let current = self.selected_session_index.unwrap_or(0);
                    if current + 1 < workspace.sessions.len() {
                        // Move to next session in this workspace
                        self.selected_session_index = Some(current + 1);
                        self.queue_logs_fetch();
                    } else if !self.other_tmux_sessions.is_empty() {
                        // At last session - move to "Other tmux" section
                        self.selected_workspace_index = None;
                        self.selected_session_index = None;
                        self.selected_other_tmux_index = Some(0);
                    }
                    // Else: stay at last session
                }
            }
        }
    }

    pub fn previous_session(&mut self) {
        // Check if we're in the "Other tmux" section
        if let Some(other_idx) = self.selected_other_tmux_index {
            if other_idx > 0 {
                // Move up within other tmux sessions
                self.selected_other_tmux_index = Some(other_idx - 1);
            } else {
                // At first other_tmux session - move back to workspaces
                if !self.workspaces.is_empty() {
                    let last_workspace_idx = self.workspaces.len() - 1;
                    self.selected_workspace_index = Some(last_workspace_idx);
                    let last_session_idx = self.workspaces[last_workspace_idx].sessions.len().saturating_sub(1);
                    self.selected_session_index = if self.workspaces[last_workspace_idx].sessions.is_empty() {
                        None
                    } else {
                        Some(last_session_idx)
                    };
                    self.selected_other_tmux_index = None;
                    self.queue_logs_fetch();
                }
            }
            return;
        }

        if let Some(workspace_idx) = self.selected_workspace_index {
            if let Some(workspace) = self.workspaces.get(workspace_idx) {
                if !workspace.sessions.is_empty() {
                    let current = self.selected_session_index.unwrap_or(0);
                    if current > 0 {
                        self.selected_session_index = Some(current - 1);
                    }
                    // At first session - stay (no wrap to other tmux from top)
                    self.queue_logs_fetch();
                }
            }
        }
    }

    pub fn next_workspace(&mut self) {
        if !self.workspaces.is_empty() {
            let current = self.selected_workspace_index.unwrap_or(0);
            self.selected_workspace_index = Some((current + 1) % self.workspaces.len());
            self.selected_session_index =
                if !self.workspaces[self.selected_workspace_index.unwrap()].sessions.is_empty() {
                    Some(0)
                } else {
                    None
                };
            // Queue container logs fetch for the newly selected session
            self.queue_logs_fetch();
        }
    }

    pub fn previous_workspace(&mut self) {
        if !self.workspaces.is_empty() {
            let current = self.selected_workspace_index.unwrap_or(0);
            self.selected_workspace_index = Some(if current == 0 {
                self.workspaces.len() - 1
            } else {
                current - 1
            });
            self.selected_session_index =
                if !self.workspaces[self.selected_workspace_index.unwrap()].sessions.is_empty() {
                    Some(0)
                } else {
                    None
                };
            // Queue container logs fetch for the newly selected session
            self.queue_logs_fetch();
        }
    }

    pub fn toggle_help(&mut self) {
        self.help_visible = !self.help_visible;
    }

    pub fn toggle_expand_all_workspaces(&mut self) {
        self.expand_all_workspaces = !self.expand_all_workspaces;
    }

    /// Toggle the expand/collapse state of the "Other tmux" section
    pub fn toggle_other_tmux_expanded(&mut self) {
        self.other_tmux_expanded = !self.other_tmux_expanded;
    }

    /// Get the currently selected other tmux session, if any
    pub fn selected_other_tmux_session(&self) -> Option<&crate::models::OtherTmuxSession> {
        self.selected_other_tmux_index
            .and_then(|idx| self.other_tmux_sessions.get(idx))
    }

    /// Check if the selection is in the "Other tmux" section
    pub fn is_other_tmux_selected(&self) -> bool {
        self.selected_other_tmux_index.is_some() && self.selected_workspace_index.is_none()
    }

    pub fn toggle_claude_chat(&mut self) {
        if self.current_view == View::ClaudeChat {
            // Close Claude chat popup and return to main view
            self.current_view = View::SessionList;
            self.claude_chat_visible = false;
        } else {
            // Open Claude chat popup
            self.current_view = View::ClaudeChat;
            self.claude_chat_visible = true;
        }
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn show_delete_confirmation(&mut self, session_id: Uuid) {
        info!("!!! SHOWING DELETE CONFIRMATION DIALOG for session: {}", session_id);
        self.confirmation_dialog = Some(ConfirmationDialog {
            title: "Delete Session".to_string(),
            message: "Are you sure you want to delete this session? This will stop the container and remove the git worktree.".to_string(),
            confirm_action: ConfirmAction::DeleteSession(session_id),
            selected_option: false, // Default to "No"
        });
    }

    /// Show confirmation dialog for killing an "other" tmux session
    pub fn show_kill_other_tmux_confirmation(&mut self, session_name: String) {
        info!("Showing kill confirmation for other tmux session: {}", session_name);
        self.confirmation_dialog = Some(ConfirmationDialog {
            title: "Kill tmux Session".to_string(),
            message: format!("Are you sure you want to kill tmux session '{}'?", session_name),
            confirm_action: ConfirmAction::KillOtherTmux(session_name),
            selected_option: false, // Default to "No"
        });
    }

    /// Queue fetching container logs for the currently selected session if needed
    fn queue_logs_fetch(&mut self) {
        // Get session ID without borrowing self
        if let Some(session_id) = self.get_selected_session_id() {
            // Only fetch if we haven't already fetched logs for this session
            if self.last_logs_session_id != Some(session_id) {
                self.pending_async_action = Some(AsyncAction::FetchContainerLogs(session_id));
                self.last_logs_session_id = Some(session_id);
            }
        }
    }

    /// Get the ID of the currently selected session without borrowing self
    pub fn get_selected_session_id(&self) -> Option<Uuid> {
        let workspace_idx = self.selected_workspace_index?;
        let session_idx = self.selected_session_index?;
        self.workspaces.get(workspace_idx)?.sessions.get(session_idx).map(|s| s.id)
    }

    /// Get a reference to the currently selected session
    pub fn get_selected_session(&self) -> Option<&crate::models::Session> {
        let workspace_idx = self.selected_workspace_index?;
        let session_idx = self.selected_session_index?;

        self.workspaces.get(workspace_idx)?.sessions.get(session_idx)
    }

    /// Attach to a container session using docker exec with proper terminal handling
    pub async fn attach_to_container(
        &mut self,
        session_id: Uuid,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crate::docker::ContainerManager;

        // Find the session to get container ID
        let container_id = self
            .workspaces
            .iter()
            .flat_map(|w| &w.sessions)
            .find(|s| s.id == session_id)
            .and_then(|s| s.container_id.as_ref())
            .cloned();

        if let Some(container_id) = container_id {
            info!(
                "Attaching to container {} for session {}",
                container_id, session_id
            );

            // Check if container is running
            let container_manager = ContainerManager::new().await?;
            let status = container_manager.get_container_status(&container_id).await?;

            match status {
                crate::docker::ContainerStatus::Running => {
                    // Start an interactive bash shell instead of Claude CLI directly
                    // This gives users more flexibility to run claude when needed
                    // Force bash to read .bashrc to load custom session environment
                    let exec_command = vec![
                        "/bin/bash".to_string(),
                        "-l".to_string(), // Login shell to read .bash_profile/.bashrc
                        "-i".to_string(), // Interactive shell
                    ];

                    match container_manager
                        .exec_interactive_blocking(&container_id, exec_command)
                        .await
                    {
                        Ok(_exit_status) => {
                            info!(
                                "Successfully detached from container {} for session {}",
                                container_id, session_id
                            );
                            // The container session has ended, stay in current view
                            Ok(())
                        }
                        Err(e) => {
                            error!("Failed to exec into container {}: {}", container_id, e);
                            Err(format!("Failed to attach to container: {}", e).into())
                        }
                    }
                }
                _ => {
                    warn!(
                        "Cannot attach to container {} - it is not running (status: {:?})",
                        container_id, status
                    );
                    Err(format!("Container is not running (status: {:?})", status).into())
                }
            }
        } else {
            warn!(
                "Cannot attach to session {} - no container ID found",
                session_id
            );
            Err("No container associated with this session".into())
        }
    }

    /// Kill the container for a session (force stop and cleanup)
    pub async fn kill_container(
        &mut self,
        session_id: Uuid,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crate::docker::ContainerManager;

        // Find the session to get container ID
        let container_id = self
            .workspaces
            .iter()
            .flat_map(|w| &w.sessions)
            .find(|s| s.id == session_id)
            .and_then(|s| s.container_id.as_ref())
            .cloned();

        if let Some(container_id) = container_id {
            info!(
                "Killing container {} for session {}",
                container_id, session_id
            );

            // Clear attached session if we're currently attached to this session
            if self.attached_session_id == Some(session_id) {
                self.attached_session_id = None;
                self.current_view = crate::app::state::View::SessionList;
                self.ui_needs_refresh = true;
            }

            let container_manager = ContainerManager::new().await?;

            // Force stop the container
            if let Some(mut session_container) = self.find_session_container_mut(session_id) {
                if let Err(e) = container_manager.stop_container(&mut session_container).await {
                    warn!("Failed to stop container gracefully: {}", e);
                }

                // Force remove the container
                if let Err(e) = container_manager.remove_container(&mut session_container).await {
                    error!("Failed to remove container: {}", e);
                    return Err(format!("Failed to remove container: {}", e).into());
                }

                info!(
                    "Successfully killed and removed container {} for session {}",
                    container_id, session_id
                );
            }

            Ok(())
        } else {
            warn!(
                "Cannot kill container for session {} - no container ID found",
                session_id
            );
            Err("No container associated with this session".into())
        }
    }

    /// Helper method to find a session container by session ID
    fn find_session_container_mut(
        &mut self,
        _session_id: Uuid,
    ) -> Option<&mut crate::docker::SessionContainer> {
        // This is a simplified approach - in a real implementation you'd need to track
        // SessionContainer objects separately or modify the Session model to include them
        None // Placeholder - would need container tracking
    }

    /// Fetch container logs for a session
    pub async fn fetch_container_logs(
        &mut self,
        session_id: Uuid,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        use crate::docker::ContainerManager;

        // Find the session to get container ID
        let container_id = self
            .workspaces
            .iter()
            .flat_map(|w| &w.sessions)
            .find(|s| s.id == session_id)
            .and_then(|s| s.container_id.as_ref())
            .cloned();

        if let Some(container_id) = container_id {
            let container_manager = ContainerManager::new().await?;
            let logs = container_manager.get_container_logs(&container_id, Some(50)).await?;

            // Update the logs cache
            self.logs.insert(session_id, logs.clone());

            Ok(logs)
        } else {
            // No container ID - return session creation logs if available
            Ok(self
                .logs
                .get(&session_id)
                .cloned()
                .unwrap_or_else(|| vec!["No container associated with this session".to_string()]))
        }
    }

    /// Fetch Claude-specific logs from the container
    pub async fn fetch_claude_logs(
        &mut self,
        session_id: Uuid,
    ) -> Result<String, Box<dyn std::error::Error>> {
        use crate::docker::ContainerManager;

        // Find the session to get container ID and update recent_logs
        let container_id = self
            .workspaces
            .iter_mut()
            .flat_map(|w| &mut w.sessions)
            .find(|s| s.id == session_id)
            .and_then(|s| {
                let id = s.container_id.clone();
                // We'll update recent_logs after fetching
                id
            });

        if let Some(container_id) = container_id {
            let container_manager = ContainerManager::new().await?;
            let logs = container_manager.tail_logs(&container_id, 20).await?;

            // Update the session's recent_logs field
            if let Some(session) = self
                .workspaces
                .iter_mut()
                .flat_map(|w| &mut w.sessions)
                .find(|s| s.id == session_id)
            {
                session.recent_logs = Some(logs.clone());
            }

            Ok(logs)
        } else {
            Ok("No container associated with this session".to_string())
        }
    }

    pub async fn new_session_normal(&mut self) {
        use crate::git::WorkspaceScanner;
        use std::env;

        info!("=== new_session_normal called ===");

        // REMOVED: Auth check moved to Boss mode selection only
        // Interactive mode works without Docker authentication (uses host ~/.claude)
        // Boss mode will check auth when selected during session creation
        info!("Proceeding with session creation (auth deferred to Boss mode selection)");

        // Check if current directory is a git repository
        let current_dir = match env::current_dir() {
            Ok(dir) => {
                info!("Current directory: {:?}", dir);
                dir
            }
            Err(e) => {
                warn!("Failed to get current directory: {}", e);
                return;
            }
        };

        match WorkspaceScanner::validate_workspace(&current_dir) {
            Ok(true) => {
                info!(
                    "Current directory is a valid git repository: {:?}",
                    current_dir
                );
            }
            Ok(false) => {
                warn!(
                    "Current directory is not a git repository: {:?}",
                    current_dir
                );
                info!("Falling back to workspace search");
                // Fall back to workspace search since current directory is not a git repository
                self.start_workspace_search().await;
                return;
            }
            Err(e) => {
                error!("Failed to validate workspace: {}", e);
                info!("Falling back to workspace search due to validation error");
                // Fall back to workspace search on validation error
                self.start_workspace_search().await;
                return;
            }
        }

        // Generate branch name with UUID
        let branch_base = format!(
            "agents-in-a-box/{}",
            uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("session")
        );

        // Create new session state for normal new session (NOT current directory mode)
        self.new_session_state = Some(NewSessionState {
            available_repos: vec![current_dir.clone()],
            filtered_repos: vec![(0, current_dir.clone())],
            selected_repo_index: Some(0),
            branch_name: branch_base.clone(),
            step: NewSessionStep::InputBranch,
            ..Default::default()
        });

        self.current_view = View::NewSession;

        info!(
            "Successfully created normal new session state with branch: {}",
            branch_base
        );
    }

    pub async fn new_session_in_current_dir(&mut self) {
        use crate::git::WorkspaceScanner;
        use std::env;

        info!("Starting new session in current directory");

        // Check if authentication is set up first
        if Self::is_first_time_setup() {
            info!("Authentication not set up, switching to auth setup view");
            self.current_view = View::AuthSetup;
            self.auth_setup_state = Some(AuthSetupState {
                selected_method: AuthMethod::OAuth,
                api_key_input: String::new(),
                is_processing: false,
                error_message: Some("Authentication required before creating sessions.\n\nPlease set up Claude authentication to continue.".to_string()),
                show_cursor: false,
            });
            return;
        }

        // Check if current directory is a git repository
        let current_dir = match env::current_dir() {
            Ok(dir) => {
                info!("Current directory: {:?}", dir);
                dir
            }
            Err(e) => {
                warn!("Failed to get current directory: {}", e);
                return;
            }
        };

        match WorkspaceScanner::validate_workspace(&current_dir) {
            Ok(true) => {
                info!(
                    "Current directory is a valid git repository: {:?}",
                    current_dir
                );
            }
            Ok(false) => {
                warn!(
                    "Current directory is not a git repository: {:?}",
                    current_dir
                );
                info!("Falling back to workspace search");
                // Fall back to workspace search since current directory is not a git repository
                self.start_workspace_search().await;
                return;
            }
            Err(e) => {
                error!("Failed to validate workspace: {}", e);
                info!("Falling back to workspace search due to validation error");
                // Fall back to workspace search on validation error
                self.start_workspace_search().await;
                return;
            }
        }

        // Generate branch name with UUID
        let branch_base = format!(
            "agents-in-a-box/{}",
            uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("session")
        );

        // Create new session state for current directory
        self.new_session_state = Some(NewSessionState {
            available_repos: vec![current_dir.clone()],
            filtered_repos: vec![(0, current_dir.clone())],
            selected_repo_index: Some(0),
            branch_name: branch_base.clone(),
            step: NewSessionStep::InputBranch,
            is_current_dir_mode: true,
            ..Default::default()
        });

        self.current_view = View::NewSession;

        info!(
            "Successfully created new session state with branch: {}",
            branch_base
        );
    }

    pub async fn start_workspace_search(&mut self) {
        info!("Starting workspace search from NonGitNotification view");

        // Always transition to SessionList first to get out of NonGitNotification
        self.current_view = View::SessionList;

        match SessionLoader::new().await {
            Ok(loader) => {
                match loader.get_available_repositories().await {
                    Ok(repos) => {
                        if repos.is_empty() {
                            warn!("No repositories found in default search paths");
                            // Even with no repos, show the search interface with empty list
                            // User can type to search or we can show helpful message
                            info!("Showing empty search interface - user can type to add paths");
                        }

                        // Generate branch name with UUID
                        let branch_base = format!(
                            "agents-in-a-box/{}",
                            uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("session")
                        );

                        // Initialize filtered repos with all repos (even if empty)
                        let filtered_repos: Vec<(usize, std::path::PathBuf)> = repos
                            .iter()
                            .enumerate()
                            .map(|(idx, path)| (idx, path.clone()))
                            .collect();

                        // Check if user has already cancelled (e.g., pressed escape while loading)
                        if self.async_operation_cancelled {
                            info!("Operation was cancelled by user");
                            return;
                        }

                        let has_repos = !filtered_repos.is_empty();
                        self.new_session_state = Some(NewSessionState {
                            available_repos: repos,
                            filtered_repos,
                            selected_repo_index: if has_repos { Some(0) } else { None },
                            branch_name: branch_base,
                            ..Default::default()
                        });

                        self.current_view = View::SearchWorkspace;
                        info!("Successfully transitioned to SearchWorkspace view");
                    }
                    Err(e) => {
                        warn!("Failed to load repositories: {}", e);
                        // Still transition to search view with empty state
                        self.new_session_state = Some(NewSessionState {
                            branch_name: format!(
                                "agents-in-a-box/{}",
                                uuid::Uuid::new_v4()
                                    .to_string()
                                    .split('-')
                                    .next()
                                    .unwrap_or("session")
                            ),
                            ..Default::default()
                        });
                        self.current_view = View::SearchWorkspace;
                        info!("Transitioned to SearchWorkspace view with empty state due to error");
                    }
                }
            }
            Err(e) => {
                warn!("Failed to create session loader: {}", e);
                // Still transition to search view with empty state
                self.new_session_state = Some(NewSessionState {
                    branch_name: format!(
                        "agents-in-a-box/{}",
                        uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("session")
                    ),
                    ..Default::default()
                });
                self.current_view = View::SearchWorkspace;
                info!("Transitioned to SearchWorkspace view with empty state due to loader error");
            }
        }
    }

    pub async fn start_new_session(&mut self) {
        info!("Starting new session creation");

        // Get available repositories
        match SessionLoader::new().await {
            Ok(loader) => match loader.get_available_repositories().await {
                Ok(repos) => {
                    let has_repos = !repos.is_empty();
                    let filtered_repos: Vec<(usize, std::path::PathBuf)> =
                        repos.iter().enumerate().map(|(idx, path)| (idx, path.clone())).collect();

                    self.new_session_state = Some(NewSessionState {
                        available_repos: repos,
                        filtered_repos,
                        selected_repo_index: if has_repos { Some(0) } else { None },
                        ..Default::default()
                    });
                    self.current_view = View::NewSession;
                }
                Err(e) => {
                    warn!("Failed to get available repositories: {}", e);
                }
            },
            Err(e) => {
                warn!("Failed to create session loader: {}", e);
            }
        }
    }

    pub fn cancel_new_session(&mut self) {
        self.new_session_state = None;
        self.current_view = View::SessionList;
        // Also clear any pending async actions to prevent race conditions
        self.pending_async_action = None;
        // Set cancellation flag to prevent race conditions
        self.async_operation_cancelled = true;
    }

    pub fn new_session_next_repo(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if !state.filtered_repos.is_empty() {
                let current = state.selected_repo_index.unwrap_or(0);
                state.selected_repo_index = Some((current + 1) % state.filtered_repos.len());
            }
        }
    }

    pub fn new_session_prev_repo(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if !state.filtered_repos.is_empty() {
                let current = state.selected_repo_index.unwrap_or(0);
                state.selected_repo_index = Some(if current == 0 {
                    state.filtered_repos.len() - 1
                } else {
                    current - 1
                });
            }
        }
    }

    pub fn new_session_confirm_repo(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.selected_repo_index.is_some() {
                tracing::info!(
                    "Confirming repository selection - selected_repo_index: {:?}",
                    state.selected_repo_index
                );
                tracing::info!(
                    "Available repos count: {}, Filtered repos count: {}",
                    state.available_repos.len(),
                    state.filtered_repos.len()
                );

                if let Some(repo_index) = state.selected_repo_index {
                    if let Some((_, repo_path)) = state.filtered_repos.get(repo_index) {
                        tracing::info!("Selected repository path: {:?}", repo_path);
                    } else {
                        tracing::error!(
                            "Failed to get repository at index {} from filtered_repos",
                            repo_index
                        );
                        return;
                    }
                }

                state.step = NewSessionStep::InputBranch;
                let uuid_str = uuid::Uuid::new_v4().to_string();
                state.branch_name = format!("agents-session-{}", &uuid_str[..8]);

                // Change view from SearchWorkspace to NewSession to show branch input
                self.current_view = View::NewSession;
                tracing::info!(
                    "Repository confirmed, transitioning to branch input step with branch: {}",
                    state.branch_name
                );
            }
        }
    }

    pub fn new_session_update_branch(&mut self, ch: char) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputBranch {
                state.branch_name.push(ch);
            }
        }
    }

    pub fn new_session_backspace(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputBranch {
                state.branch_name.pop();
            }
        }
    }

    pub fn new_session_proceed_to_mode_selection(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputBranch {
                tracing::info!(
                    "Proceeding from InputBranch to SelectMode with branch: {}",
                    state.branch_name
                );
                state.step = NewSessionStep::SelectMode;
            }
        }
    }

    pub fn new_session_proceed_from_mode(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::SelectMode {
                tracing::info!(
                    "Proceeding from SelectMode to next step with mode: {:?}",
                    state.mode
                );
                match state.mode {
                    crate::models::SessionMode::Interactive => {
                        // Interactive mode: go directly to permissions
                        state.step = NewSessionStep::ConfigurePermissions;
                        tracing::info!("Interactive mode selected, going to ConfigurePermissions");
                    }
                    crate::models::SessionMode::Boss => {
                        // Boss mode: go to prompt input first
                        state.step = NewSessionStep::InputPrompt;
                        tracing::info!("Boss mode selected, going to InputPrompt");
                    }
                }
            }
        }
    }

    pub fn new_session_proceed_to_permissions(&mut self) {
        tracing::info!("new_session_proceed_to_permissions called");
        if let Some(ref mut state) = self.new_session_state {
            tracing::debug!("Current session state step: {:?}", state.step);
            if state.step == NewSessionStep::InputPrompt {
                tracing::info!("Advancing from InputPrompt to ConfigurePermissions");
                state.step = NewSessionStep::ConfigurePermissions;
                self.ui_needs_refresh = true;
            } else {
                tracing::warn!(
                    "Cannot proceed to permissions - not in InputPrompt step (current: {:?})",
                    state.step
                );
            }
        } else {
            tracing::error!("Cannot proceed to permissions - no session state found");
        }
    }

    pub fn new_session_toggle_mode(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::SelectMode {
                state.mode = match state.mode {
                    crate::models::SessionMode::Interactive => crate::models::SessionMode::Boss,
                    crate::models::SessionMode::Boss => crate::models::SessionMode::Interactive,
                };
            }
        }
    }

    pub fn new_session_add_char_to_prompt(&mut self, ch: char) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputPrompt {
                if ch == '@' {
                    // Activate fuzzy file finder (supports multiple @ references)
                    let workspace_root = if let Some(selected_idx) = state.selected_repo_index {
                        state.filtered_repos.get(selected_idx).map(|(_, path)| path.clone())
                    } else {
                        None
                    };
                    // If already active, deactivate current search and start new one
                    if state.file_finder.is_active {
                        state.file_finder.deactivate();
                    }
                    state.file_finder.activate(state.boss_prompt.to_string().len(), workspace_root);
                    state.boss_prompt.insert_char(ch);
                } else if state.file_finder.is_active {
                    // File finder is active, handle character input for filtering
                    if ch == ' ' || ch == '\t' || ch == '\n' {
                        // Whitespace deactivates file finder
                        state.file_finder.deactivate();
                        state.boss_prompt.insert_char(ch);
                    } else {
                        state.file_finder.add_char_to_query(ch);
                    }
                } else {
                    // Normal character input
                    state.boss_prompt.insert_char(ch);
                }
            }
        }
    }

    pub fn new_session_backspace_prompt(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputPrompt {
                if state.file_finder.is_active {
                    if !state.file_finder.query.is_empty() {
                        // Remove character from file finder query
                        state.file_finder.backspace_query();
                    } else {
                        // Query is empty, deactivate file finder and remove @ symbol
                        state.file_finder.deactivate();
                        state.boss_prompt.backspace();
                    }
                } else {
                    // Normal backspace
                    state.boss_prompt.backspace();
                }
            }
        }
    }

    pub fn new_session_move_cursor_left(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputPrompt && !state.file_finder.is_active {
                state.boss_prompt.move_cursor_left();
            }
        }
    }

    pub fn new_session_move_cursor_right(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputPrompt && !state.file_finder.is_active {
                state.boss_prompt.move_cursor_right();
            }
        }
    }

    pub fn new_session_move_cursor_up(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputPrompt && !state.file_finder.is_active {
                state.boss_prompt.move_cursor_up();
            }
        }
    }

    pub fn new_session_move_cursor_down(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputPrompt && !state.file_finder.is_active {
                state.boss_prompt.move_cursor_down();
            }
        }
    }

    pub fn new_session_move_to_line_start(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputPrompt && !state.file_finder.is_active {
                state.boss_prompt.move_to_line_start();
            }
        }
    }

    pub fn new_session_move_to_line_end(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputPrompt && !state.file_finder.is_active {
                state.boss_prompt.move_to_line_end();
            }
        }
    }

    pub fn new_session_move_cursor_word_left(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputPrompt && !state.file_finder.is_active {
                state.boss_prompt.move_cursor_word_backward();
            }
        }
    }

    pub fn new_session_move_cursor_word_right(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputPrompt && !state.file_finder.is_active {
                state.boss_prompt.move_cursor_word_forward();
            }
        }
    }

    pub fn new_session_delete_word_forward(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputPrompt && !state.file_finder.is_active {
                state.boss_prompt.delete_word_forward();
            }
        }
    }

    pub fn new_session_delete_word_backward(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputPrompt && !state.file_finder.is_active {
                state.boss_prompt.delete_word_backward();
            }
        }
    }

    pub fn new_session_insert_newline(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputPrompt && !state.file_finder.is_active {
                state.boss_prompt.insert_newline();
            }
        }
    }

    pub fn new_session_paste_text(&mut self, text: String) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::InputPrompt && !state.file_finder.is_active {
                // Insert the pasted text at the current cursor position
                state.boss_prompt.insert_text(&text);
            }
        }
    }

    pub fn new_session_toggle_permissions(&mut self) {
        if let Some(ref mut state) = self.new_session_state {
            if state.step == NewSessionStep::ConfigurePermissions {
                state.skip_permissions = !state.skip_permissions;
            }
        }
    }

    pub async fn new_session_create(&mut self) {
        // Check session mode FIRST to determine if auth is needed
        let session_mode = if let Some(ref state) = self.new_session_state {
            state.mode.clone()
        } else {
            tracing::error!("new_session_create called but new_session_state is None");
            return;
        };

        // ONLY check authentication for Boss mode (Docker-based sessions)
        // Interactive mode uses host ~/.claude and doesn't need Docker auth
        if session_mode == crate::models::SessionMode::Boss {
            // First check if Docker is available (Boss mode requires Docker)
            if !self.is_docker_available().await {
                error!("Boss mode requires Docker but Docker is not running");
                self.add_error_notification(
                    "Boss mode requires Docker.\n\nPlease start Docker and try again, or use Interactive mode instead.".to_string()
                );
                // Stay in current view so user can go back and select Interactive mode
                return;
            }

            // Check if tokens need refresh (Docker is available at this point)
            if let Some(home) = dirs::home_dir() {
                let credentials_path = home.join(".agents-in-a-box/auth/.credentials.json");
                if credentials_path.exists() && Self::oauth_token_needs_refresh(&credentials_path) {
                    info!("Boss mode selected - OAuth tokens need refresh, attempting refresh");
                    match self.refresh_oauth_tokens().await {
                        Ok(()) => info!("OAuth tokens refreshed successfully for Boss mode"),
                        Err(e) => {
                            error!("Failed to refresh OAuth tokens for Boss mode: {}", e);
                            self.add_error_notification(
                                format!("Failed to refresh OAuth tokens: {}\n\nPlease check Docker and try again.", e)
                            );
                            return;
                        }
                    }
                }
            }

            // Then check if authentication is set up
            if Self::is_first_time_setup() {
                info!("Boss mode selected but authentication not set up, switching to auth setup view");
                self.current_view = View::AuthSetup;
                self.auth_setup_state = Some(AuthSetupState {
                    selected_method: AuthMethod::OAuth,
                    api_key_input: String::new(),
                    is_processing: false,
                    error_message: Some("Boss mode requires Docker authentication.\n\nPlease set up Claude authentication to continue.".to_string()),
                    show_cursor: false,
                });
                // Clear new session state
                self.new_session_state = None;
                return;
            }
        } else {
            info!("Interactive mode selected - skipping Docker auth check (will use host ~/.claude)");
        }

        let (
            repo_path,
            branch_name,
            session_id,
            skip_permissions,
            mode,
            boss_prompt,
            restart_session_id,
        ) = {
            if let Some(ref mut state) = self.new_session_state {
                tracing::info!("new_session_create called with step: {:?}", state.step);

                // Handle both ConfigurePermissions step (normal flow) and InputBranch step (current dir mode)
                let can_create = match state.step {
                    NewSessionStep::ConfigurePermissions => true,
                    NewSessionStep::InputBranch if state.is_current_dir_mode => {
                        // For current directory mode, skip to permissions step with defaults
                        state.step = NewSessionStep::ConfigurePermissions;
                        state.skip_permissions = false; // Default to safe permissions
                        state.mode = crate::models::SessionMode::Interactive; // Default mode
                        true
                    }
                    _ => false,
                };

                if can_create {
                    if let Some(repo_index) = state.selected_repo_index {
                        if let Some((_, repo_path)) = state.filtered_repos.get(repo_index) {
                            tracing::info!(
                                "Creating session for repository: {:?}, branch: {}",
                                repo_path,
                                state.branch_name
                            );
                            state.step = NewSessionStep::Creating;

                            // Use existing session ID for restart, or generate new one
                            let session_id =
                                state.restart_session_id.unwrap_or_else(|| uuid::Uuid::new_v4());

                            (
                                repo_path.clone(),
                                state.branch_name.clone(),
                                session_id,
                                state.skip_permissions,
                                state.mode.clone(),
                                if state.mode == crate::models::SessionMode::Boss {
                                    Some(state.boss_prompt.to_string())
                                } else {
                                    None
                                },
                                state.restart_session_id, // Pass restart session ID
                            )
                        } else {
                            tracing::error!(
                                "Failed to get repository path from filtered_repos at index: {}",
                                repo_index
                            );
                            return;
                        }
                    } else {
                        tracing::error!("No repository selected (selected_repo_index is None)");
                        return;
                    }
                } else {
                    tracing::warn!(
                        "new_session_create called but step is not valid for creation, current step: {:?}, is_current_dir_mode: {}",
                        state.step,
                        state.is_current_dir_mode
                    );
                    return;
                }
            } else {
                tracing::error!("new_session_create called but new_session_state is None");
                return;
            }
        };

        // Create the session with log streaming
        tracing::info!(
            "Calling create_session_with_logs for session {} (mode: {:?}, restart: {})",
            session_id,
            mode,
            restart_session_id.is_some()
        );

        let result = if let Some(restart_id) = restart_session_id {
            // This is a restart - try to reuse existing worktree
            info!(
                "Restarting session {} with potentially updated configuration",
                restart_id
            );
            self.create_restart_session_with_logs(
                &repo_path,
                &branch_name,
                session_id,
                skip_permissions,
                mode,
                boss_prompt,
            )
            .await
        } else {
            // Normal new session creation
            self.create_session_with_logs(
                &repo_path,
                &branch_name,
                session_id,
                skip_permissions,
                mode,
                boss_prompt,
            )
            .await
        };

        match result {
            Ok(()) => {
                info!("Session created successfully");
                // Reload workspaces BEFORE switching view to ensure UI shows new session immediately
                self.load_real_workspaces().await;

                // Start log streaming for the newly created session
                if let Err(e) = self.start_log_streaming_for_session(session_id).await {
                    warn!(
                        "Failed to start log streaming for session {}: {}",
                        session_id, e
                    );
                }

                // Force UI refresh to show new session immediately
                self.ui_needs_refresh = true;
                self.cancel_new_session();
            }
            Err(e) => {
                error!("Failed to create session: {}", e);
                self.cancel_new_session();
            }
        }
    }

    async fn create_restart_session_with_logs(
        &mut self,
        repo_path: &std::path::Path,
        branch_name: &str,
        session_id: Uuid,
        skip_permissions: bool,
        mode: crate::models::SessionMode,
        boss_prompt: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crate::docker::session_lifecycle::{SessionLifecycleManager, SessionRequest};
        use std::path::PathBuf;

        info!(
            "Creating restart session {} with updated configuration",
            session_id
        );

        // Create a channel for build logs
        let (log_sender, mut log_receiver) = mpsc::unbounded_channel::<String>();

        // Initialize logs for this session
        self.logs.insert(
            session_id,
            vec!["Restarting session with updated configuration...".to_string()],
        );

        // Create a shared vector for logs
        let session_logs = Arc::new(Mutex::new(Vec::new()));
        let logs_clone = session_logs.clone();

        // Spawn a task to collect logs
        let session_id_clone = session_id;
        tokio::spawn(async move {
            while let Some(log_message) = log_receiver.recv().await {
                if let Ok(mut logs) = logs_clone.lock() {
                    logs.push(log_message.clone());
                }
                info!(
                    "Restart log for session {}: {}",
                    session_id_clone, log_message
                );
            }
        });

        let workspace_name =
            repo_path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_string();

        // Clone mode so we can use it later for tmux check
        let mode_clone = mode.clone();

        let request = SessionRequest {
            session_id,
            workspace_name,
            workspace_path: repo_path.to_path_buf(),
            branch_name: branch_name.to_string(),
            base_branch: None,
            container_config: None,
            skip_permissions,
            mode,
            boss_prompt,
        };

        // Add initial log message
        if let Some(session_logs) = self.logs.get_mut(&session_id) {
            session_logs.push("Checking for existing worktree...".to_string());
        }

        let mut manager = SessionLifecycleManager::new().await?;

        // Check if worktree exists from the previous session
        let existing_worktree_path = self
            .workspaces
            .iter()
            .flat_map(|w| &w.sessions)
            .find(|s| s.id == session_id)
            .map(|s| PathBuf::from(&s.workspace_path));

        let result = if let Some(worktree_path) = existing_worktree_path {
            if worktree_path.exists() {
                info!(
                    "Found existing worktree at {}, reusing it",
                    worktree_path.display()
                );

                if let Some(logs) = self.logs.get_mut(&session_id) {
                    logs.push(format!(
                        "Reusing existing worktree at {}",
                        worktree_path.display()
                    ));
                }

                let worktree_info = crate::git::WorktreeInfo {
                    id: session_id, // Use session ID as worktree ID
                    path: worktree_path.clone(),
                    session_path: worktree_path.clone(), // Same as path for existing worktrees
                    branch_name: branch_name.to_string(),
                    source_repository: repo_path.to_path_buf(),
                    commit_hash: None, // We don't track this for existing worktrees
                };

                manager.create_session_with_existing_worktree(request, worktree_info).await
            } else {
                info!("Worktree path no longer exists, creating fresh session");

                if let Some(logs) = self.logs.get_mut(&session_id) {
                    logs.push("Worktree not found, creating fresh session...".to_string());
                }

                manager.create_session_with_logs(request, Some(log_sender.clone())).await
            }
        } else {
            info!("No existing worktree info found, creating fresh session");

            if let Some(logs) = self.logs.get_mut(&session_id) {
                logs.push("Creating fresh session...".to_string());
            }

            manager.create_session_with_logs(request, Some(log_sender.clone())).await
        };

        // Wait a moment for logs to be collected
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Transfer collected logs to our main logs HashMap
        if let Ok(collected_logs) = session_logs.lock() {
            if let Some(logs) = self.logs.get_mut(&session_id) {
                logs.extend(collected_logs.clone());
            }
        }

        // Add completion log based on result
        if let Some(logs) = self.logs.get_mut(&session_id) {
            match &result {
                Ok(_) => logs
                    .push("Session restarted successfully with updated configuration!".to_string()),
                Err(e) => logs.push(format!("Session restart failed: {}", e)),
            }
        }

        // If Docker session creation succeeded AND this is Interactive mode, create corresponding tmux session
        // Boss mode sessions should NOT have tmux integration
        if let Ok(ref session_state) = result {
            if mode_clone == crate::models::SessionMode::Interactive {
                if let Some(ref worktree_info) = session_state.worktree_info {
                    info!("Creating tmux session for restarted Interactive mode session {}", session_id);

                    // Send log message about tmux session creation
                    let _ = log_sender.send("Creating tmux session for interactive mode...".to_string());

                    // Create tmux session name from session info
                    let tmux_name = format!("tmux_{}", branch_name.replace('/', "_").replace(' ', "_"));

                    let mut tmux_session = crate::tmux::TmuxSession::new(
                        tmux_name.clone(),
                        "claude".to_string()
                    );
                    let tmux_session_name = tmux_session.name().to_string();

                    // Start tmux session in the worktree directory
                    match tmux_session.start(&worktree_info.path).await {
                        Ok(_) => {
                            info!("Successfully started tmux session: {}", tmux_session_name);

                            // Store tmux session name in the actual session model
                            if let Some(session) = self.find_session_mut(session_id) {
                                session.set_tmux_session_name(tmux_session_name.clone());
                            }

                            // Store tmux session in our map
                            self.tmux_sessions.insert(session_id, tmux_session);

                            let _ = log_sender.send("Tmux session created successfully!".to_string());
                        }
                        Err(e) => {
                            warn!("Failed to start tmux session: {}", e);
                            let _ = log_sender.send(format!("Warning: Failed to create tmux session: {}", e));
                            // Don't fail the whole session creation if tmux fails
                        }
                    }
                } else {
                    warn!("Session state has no worktree info, skipping tmux creation");
                }
            } else {
                info!("Skipping tmux creation for Boss mode session {}", session_id);
            }
        }

        result.map(|_| ())?;
        Ok(())
    }

    async fn create_session_with_logs(
        &mut self,
        repo_path: &std::path::Path,
        branch_name: &str,
        session_id: Uuid,
        skip_permissions: bool,
        mode: crate::models::SessionMode,
        boss_prompt: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Branch based on session mode
        match mode {
            crate::models::SessionMode::Interactive => {
                self.create_interactive_session(
                    repo_path,
                    branch_name,
                    session_id,
                    skip_permissions,
                )
                .await
            }
            crate::models::SessionMode::Boss => {
                self.create_boss_session(
                    repo_path,
                    branch_name,
                    session_id,
                    skip_permissions,
                    boss_prompt,
                )
                .await
            }
        }
    }

    /// Create an Interactive mode session (host-based, no Docker)
    async fn create_interactive_session(
        &mut self,
        repo_path: &std::path::Path,
        branch_name: &str,
        session_id: Uuid,
        skip_permissions: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crate::interactive::InteractiveSessionManager;

        info!(
            "Creating Interactive mode session {} for branch '{}' (skip_permissions={})",
            session_id, branch_name, skip_permissions
        );

        // Create a channel for logs
        let (log_sender, mut log_receiver) = mpsc::unbounded_channel::<String>();

        // Initialize logs for this session
        self.logs.insert(session_id, vec!["Starting Interactive session creation...".to_string()]);

        // Create a shared vector for logs
        let session_logs = Arc::new(Mutex::new(Vec::new()));
        let logs_clone = session_logs.clone();

        // Spawn a task to collect logs
        let session_id_clone = session_id;
        tokio::spawn(async move {
            while let Some(log_message) = log_receiver.recv().await {
                if let Ok(mut logs) = logs_clone.lock() {
                    logs.push(log_message.clone());
                }
                info!("Interactive session log for {}: {}", session_id_clone, log_message);
            }
        });

        let workspace_name = repo_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Send log updates
        let _ = log_sender.send("Creating git worktree...".to_string());

        // Create Interactive session manager (NO Docker dependency)
        let mut manager = InteractiveSessionManager::new()?;

        // Create the session
        let result = manager
            .create_session(
                session_id,
                workspace_name.clone(),
                repo_path.to_path_buf(),
                branch_name.to_string(),
                None, // base_branch
                skip_permissions,
            )
            .await;

        // Wait for logs to be collected
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Transfer collected logs
        if let Ok(collected_logs) = session_logs.lock() {
            if let Some(logs) = self.logs.get_mut(&session_id) {
                logs.extend(collected_logs.clone());
            }
        }

        match result {
            Ok(interactive_session) => {
                // Send success log
                if let Some(logs) = self.logs.get_mut(&session_id) {
                    logs.push("Interactive session created successfully!".to_string());
                }

                // Convert to Session model and add to workspaces
                let session = interactive_session.to_session_model();

                // Find or create workspace for this repo
                if let Some(workspace) = self.workspaces.iter_mut().find(|w| {
                    std::path::Path::new(&w.path).canonicalize().ok()
                        == repo_path.canonicalize().ok()
                }) {
                    workspace.sessions.push(session);
                } else {
                    // Create new workspace
                    let mut workspace = crate::models::Workspace::new(
                        workspace_name,
                        repo_path.to_path_buf(),
                    );
                    workspace.sessions.push(session);
                    self.workspaces.push(workspace);
                }

                // Store tmux session for attach operations
                // Pass branch name (NOT tmux-prefixed name) to TmuxSession::new()
                // because TmuxSession::sanitize_name() will add the tmux_ prefix
                let tmux_session = crate::tmux::TmuxSession::new(
                    interactive_session.branch_name.clone(),
                    "claude".to_string(),
                );
                self.tmux_sessions.insert(session_id, tmux_session);

                info!("Successfully created Interactive session {}", session_id);
                Ok(())
            }
            Err(e) => {
                error!("Failed to create Interactive session: {}", e);
                if let Some(logs) = self.logs.get_mut(&session_id) {
                    logs.push(format!("Session creation failed: {}", e));
                }
                Err(Box::new(e))
            }
        }
    }

    /// Create a Boss mode session (Docker-based)
    async fn create_boss_session(
        &mut self,
        repo_path: &std::path::Path,
        branch_name: &str,
        session_id: Uuid,
        skip_permissions: bool,
        boss_prompt: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crate::docker::session_lifecycle::{SessionLifecycleManager, SessionRequest};

        info!(
            "Creating Boss mode session {} for branch '{}'",
            session_id, branch_name
        );

        // Create a channel for build logs
        let (log_sender, mut log_receiver) = mpsc::unbounded_channel::<String>();

        // Initialize logs for this session
        self.logs.insert(session_id, vec!["Starting Boss session creation...".to_string()]);

        // Create a shared vector for logs
        let session_logs = Arc::new(Mutex::new(Vec::new()));
        let logs_clone = session_logs.clone();

        // Spawn a task to collect logs
        let session_id_clone = session_id;
        tokio::spawn(async move {
            while let Some(log_message) = log_receiver.recv().await {
                if let Ok(mut logs) = logs_clone.lock() {
                    logs.push(log_message.clone());
                }
                info!(
                    "Build log for session {}: {}",
                    session_id_clone, log_message
                );
            }
        });

        let workspace_name =
            repo_path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_string();

        let request = SessionRequest {
            session_id,
            workspace_name,
            workspace_path: repo_path.to_path_buf(),
            branch_name: branch_name.to_string(),
            base_branch: None,
            container_config: None,
            skip_permissions,
            mode: crate::models::SessionMode::Boss,
            boss_prompt,
        };

        // Add initial log message
        if let Some(session_logs) = self.logs.get_mut(&session_id) {
            session_logs.push("Creating worktree...".to_string());
        }

        // Create Docker-based session manager
        let mut manager = SessionLifecycleManager::new().await?;

        // Pass the log sender to the session lifecycle manager
        let result = manager.create_session_with_logs(request, Some(log_sender)).await;

        // Wait a moment for logs to be collected
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Transfer collected logs to our main logs HashMap
        if let Ok(collected_logs) = session_logs.lock() {
            if let Some(logs) = self.logs.get_mut(&session_id) {
                logs.extend(collected_logs.clone());
            }
        }

        // Add completion log based on result
        if let Some(logs) = self.logs.get_mut(&session_id) {
            match &result {
                Ok(_) => logs.push("Boss session created successfully!".to_string()),
                Err(e) => logs.push(format!("Session creation failed: {}", e)),
            }
        }

        result.map(|_| ())?;
        Ok(())
    }

    /// Clean up orphaned containers (containers without worktrees) AND orphaned session state
    pub async fn cleanup_orphaned_containers(&mut self) -> anyhow::Result<usize> {
        use crate::docker::ContainerManager;

        info!("Starting cleanup of orphaned containers and state entries");

        let container_manager = ContainerManager::new().await?;
        let containers = container_manager.list_agents_containers().await?;

        let mut cleaned_up = 0;

        // Step 1: Clean up orphaned containers (containers without worktrees)
        for container in containers {
            if let Some(session_id_str) =
                container.labels.as_ref().and_then(|labels| labels.get("agents-session-id"))
            {
                if let Ok(session_id) = uuid::Uuid::parse_str(session_id_str) {
                    // Check if worktree exists for this session
                    let worktree_manager = crate::git::WorktreeManager::new()?;
                    match worktree_manager.get_worktree_info(session_id) {
                        Ok(_) => {
                            // Worktree exists, container is not orphaned
                            continue;
                        }
                        Err(_) => {
                            // Worktree missing, this is an orphaned container
                            info!(
                                "Found orphaned container for session {}, removing it",
                                session_id
                            );

                            if let Some(container_id) = &container.id {
                                // Remove the orphaned container (this will stop it first)
                                if let Err(e) =
                                    container_manager.remove_container_by_id(container_id).await
                                {
                                    warn!(
                                        "Failed to remove orphaned container {}: {}",
                                        container_id, e
                                    );
                                } else {
                                    cleaned_up += 1;
                                    info!(
                                        "Successfully removed orphaned container {}",
                                        container_id
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        // Step 2: Clean up orphaned session state (sessions in workspace list without worktrees)
        let worktree_manager = crate::git::WorktreeManager::new()?;
        let mut orphaned_sessions = Vec::new();

        // Collect all session IDs from all workspaces
        for workspace in &self.workspaces {
            for session in &workspace.sessions {
                // Check if this session's name starts with "orphaned-"
                if session.name.starts_with("orphaned-") {
                    orphaned_sessions.push(session.id);
                } else {
                    // Also check if the worktree actually exists
                    if let Err(_) = worktree_manager.get_worktree_info(session.id) {
                        info!("Found session without worktree: {} ({})", session.name, session.id);
                        orphaned_sessions.push(session.id);
                    }
                }
            }
        }

        // Remove orphaned session state entries
        for session_id in &orphaned_sessions {
            info!("Removing orphaned session state: {}", session_id);

            // Remove from workspaces
            for workspace in &mut self.workspaces {
                workspace.sessions.retain(|s| s.id != *session_id);
            }

            // Clean up any remaining state
            self.live_logs.remove(session_id);

            cleaned_up += 1;
        }

        // Step 3: Prune git worktrees (removes stale git references for deleted worktrees)
        info!("Pruning git worktrees to remove stale references");
        use tokio::process::Command;
        match Command::new("git")
            .arg("worktree")
            .arg("prune")
            .arg("-v")
            .output()
            .await
        {
            Ok(output) => {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if !stdout.trim().is_empty() {
                        info!("Git worktree prune output: {}", stdout.trim());
                        // Count lines that start with "Removing" to track pruned worktrees
                        let pruned_count = stdout.lines().filter(|line| line.contains("Removing")).count();
                        if pruned_count > 0 {
                            info!("Pruned {} stale git worktree references", pruned_count);
                            cleaned_up += pruned_count;
                        }
                    } else {
                        info!("No stale git worktree references to prune");
                    }
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    warn!("Git worktree prune failed: {}", stderr);
                }
            }
            Err(e) => {
                warn!("Failed to run git worktree prune: {}", e);
            }
        }

        if cleaned_up > 0 {
            info!("Cleaned up {} orphaned items (containers + state + git refs)", cleaned_up);
            self.add_success_notification(format!(
                "🧹 Cleaned up {} orphaned items",
                cleaned_up
            ));

            // Reload workspaces to reflect changes
            self.load_real_workspaces().await;
            self.ui_needs_refresh = true;
        } else {
            info!("No orphaned containers or sessions found");
            self.add_info_notification("✅ No orphaned items found".to_string());
        }

        Ok(cleaned_up)
    }

    async fn delete_session(&mut self, session_id: Uuid) -> anyhow::Result<()> {
        info!("Deleting session: {}", session_id);

        // Determine session mode by finding the session
        let session_mode = self.find_session(session_id)
            .map(|s| s.mode.clone());

        if let Some(mode) = session_mode {
            match mode {
                crate::models::SessionMode::Interactive => {
                    self.delete_interactive_session(session_id).await?;
                }
                crate::models::SessionMode::Boss => {
                    self.delete_boss_session(session_id).await?;
                }
            }
        } else {
            // Session not found in UI, try both cleanup methods
            warn!("Session {} not found in UI, attempting cleanup anyway", session_id);

            // Try Interactive cleanup first (no Docker needed)
            if let Err(e) = self.delete_interactive_session(session_id).await {
                debug!("Interactive cleanup failed (expected if Boss mode): {}", e);
            }

            // Try Boss cleanup if Docker is available
            if self.is_docker_available().await {
                if let Err(e) = self.delete_boss_session(session_id).await {
                    debug!("Boss cleanup failed (expected if Interactive mode): {}", e);
                }
            }
        }

        // Reload workspaces to ensure UI reflects the actual state
        self.load_real_workspaces().await;
        // Force UI refresh to show updated session list immediately
        self.ui_needs_refresh = true;

        info!("Successfully deleted session: {}", session_id);
        Ok(())
    }

    /// Delete an Interactive mode session
    async fn delete_interactive_session(&mut self, session_id: Uuid) -> anyhow::Result<()> {
        use crate::interactive::InteractiveSessionManager;

        info!("=== DELETE INTERACTIVE SESSION START: {} ===", session_id);

        // Cleanup tmux session if it exists
        if let Some(mut tmux_session) = self.tmux_sessions.remove(&session_id) {
            info!("Found tmux session in state, cleaning up: {}", session_id);
            if let Err(e) = tmux_session.cleanup().await {
                warn!("Failed to cleanup tmux session from state: {}", e);
            }
        } else {
            info!("No tmux session found in state for: {}", session_id);
        }

        // Use Interactive session manager to remove session
        info!("Creating InteractiveSessionManager for session: {}", session_id);
        let mut manager = InteractiveSessionManager::new()?;
        info!("Calling manager.remove_session() for: {}", session_id);
        match manager.remove_session(session_id).await {
            Ok(()) => info!("manager.remove_session() succeeded for: {}", session_id),
            Err(e) => {
                error!("manager.remove_session() failed for {}: {}", session_id, e);
                return Err(e.into());
            }
        }

        info!("=== DELETE INTERACTIVE SESSION COMPLETE: {} ===", session_id);
        Ok(())
    }

    /// Delete a Boss mode session
    async fn delete_boss_session(&mut self, session_id: Uuid) -> anyhow::Result<()> {
        use crate::docker::{ContainerManager, SessionLifecycleManager};
        use crate::git::WorktreeManager;

        info!("Deleting Boss mode session: {}", session_id);

        // Cleanup tmux session if it exists (Boss mode might have tmux for attach)
        if let Some(mut tmux_session) = self.tmux_sessions.remove(&session_id) {
            info!("Cleaning up tmux session for Boss session {}", session_id);
            if let Err(e) = tmux_session.cleanup().await {
                warn!("Failed to cleanup tmux session: {}", e);
            }
        }

        // First, try to find and remove the container directly
        let container_name = format!("agents-session-{}", session_id);
        let container_manager = ContainerManager::new().await?;

        info!("Looking for container: {}", container_name);
        if let Ok(containers) = container_manager.list_agents_containers().await {
            for container in containers {
                if let Some(names) = &container.names {
                    if names.iter().any(|n| n.trim_start_matches('/') == container_name) {
                        info!("Found container for session {}, removing it", session_id);
                        if let Some(container_id) = &container.id {
                            match container_manager.remove_container_by_id(container_id).await {
                                Ok(_) => info!("Successfully removed container {}", container_id),
                                Err(e) => {
                                    warn!("Failed to remove container {}: {}", container_id, e)
                                }
                            }
                        }
                        break;
                    }
                }
            }
        }

        // Create session lifecycle manager
        let mut manager = SessionLifecycleManager::new().await?;

        // Try to remove the session through lifecycle manager (this will handle worktree)
        match manager.remove_session(session_id).await {
            Ok(_) => {
                info!("Session removed through lifecycle manager");
            }
            Err(e) => {
                warn!("Session not found in lifecycle manager: {}", e);
                info!("Attempting to remove orphaned worktree directly");

                // Remove the worktree directly
                let worktree_manager = WorktreeManager::new()?;
                if let Err(worktree_err) = worktree_manager.remove_worktree(session_id) {
                    warn!("Failed to remove worktree: {}", worktree_err);
                } else {
                    info!("Successfully removed orphaned worktree");
                }
            }
        }

        info!("Successfully deleted Boss session: {}", session_id);
        Ok(())
    }

    pub async fn process_async_action(&mut self) -> anyhow::Result<()> {
        if let Some(action) = self.pending_async_action.take() {
            info!(">>> process_async_action() called with action: {:?}", action);
            match action {
                AsyncAction::StartNewSession => {
                    self.start_new_session().await;
                }
                AsyncAction::StartWorkspaceSearch => {
                    // Add timeout to prevent hanging
                    use tokio::time::{Duration, timeout};
                    match timeout(Duration::from_secs(10), self.start_workspace_search()).await {
                        Ok(_) => {}
                        Err(_) => {
                            warn!("Workspace search timed out after 10 seconds");
                            // Return to safe state
                            self.new_session_state = None;
                            self.current_view = View::SessionList;
                            return Err(anyhow::anyhow!("Workspace search timed out"));
                        }
                    }
                }
                AsyncAction::NewSessionInCurrentDir => {
                    self.new_session_in_current_dir().await;
                }
                AsyncAction::NewSessionNormal => {
                    self.new_session_normal().await;
                }
                AsyncAction::CreateNewSession => {
                    self.new_session_create().await;
                }
                AsyncAction::DeleteSession(session_id) => {
                    if let Err(e) = self.delete_session(session_id).await {
                        error!("Failed to delete session {}: {}", session_id, e);
                    }
                }
                AsyncAction::RefreshWorkspaces => {
                    info!("Manual refresh triggered");
                    // Reload workspace data and force UI refresh
                    self.load_real_workspaces().await;
                    self.ui_needs_refresh = true;
                }
                AsyncAction::FetchContainerLogs(session_id) => {
                    info!("Fetching container logs for session {}", session_id);
                    if let Err(e) = self.fetch_container_logs(session_id).await {
                        warn!(
                            "Failed to fetch container logs for session {}: {}",
                            session_id, e
                        );
                    }
                    self.ui_needs_refresh = true;
                }
                AsyncAction::AttachToContainer(session_id) => {
                    info!("Attaching to container for session {}", session_id);
                    if let Err(e) = self.attach_to_container(session_id).await {
                        error!(
                            "Failed to attach to container for session {}: {}",
                            session_id, e
                        );
                    }
                    self.ui_needs_refresh = true;
                }
                AsyncAction::AttachToTmuxSession(_session_id) => {
                    // NOTE: This action must be handled in main.rs where terminal access is available
                    // The terminal handle is needed to call attach_to_tmux_session
                    warn!("AttachToTmuxSession action should be handled in main loop, not here");
                    self.ui_needs_refresh = true;
                }
                AsyncAction::KillContainer(session_id) => {
                    info!("Killing container for session {}", session_id);
                    if let Err(e) = self.kill_container(session_id).await {
                        error!("Failed to kill container for session {}: {}", session_id, e);
                    }
                    self.ui_needs_refresh = true;
                }
                AsyncAction::AuthSetupOAuth => {
                    info!("Starting OAuth authentication setup");
                    if let Err(e) = self.run_oauth_setup().await {
                        error!("Failed to setup OAuth authentication: {}", e);
                        if let Some(ref mut auth_state) = self.auth_setup_state {
                            auth_state.error_message = Some(format!("OAuth setup failed: {}", e));
                            auth_state.is_processing = false;
                        }
                    }
                }
                AsyncAction::AuthSetupApiKey => {
                    info!("Saving API key authentication");
                    if let Err(e) = self.save_api_key().await {
                        error!("Failed to save API key: {}", e);
                        if let Some(ref mut auth_state) = self.auth_setup_state {
                            auth_state.error_message =
                                Some(format!("Failed to save API key: {}", e));
                            auth_state.is_processing = false;
                        }
                    }
                }
                AsyncAction::ReauthenticateCredentials => {
                    info!("Starting re-authentication process");
                    if let Err(e) = self.handle_reauthenticate().await {
                        error!("Failed to re-authenticate: {}", e);
                    }
                }
                AsyncAction::RestartSession(session_id) => {
                    info!("Starting session restart for session {}", session_id);
                    if let Err(e) = self.handle_restart_session(session_id).await {
                        error!("Failed to restart session: {}", e);
                    }
                }
                AsyncAction::CleanupOrphaned => {
                    info!("Starting cleanup of orphaned containers");
                    if let Err(e) = self.cleanup_orphaned_containers().await {
                        error!("Failed to cleanup orphaned containers: {}", e);
                        self.add_error_notification(format!(
                            "❌ Failed to cleanup orphaned containers: {}",
                            e
                        ));
                    }
                }
                AsyncAction::AttachToOtherTmux(_session_name) => {
                    // NOTE: This action must be handled in main.rs where terminal access is available
                    warn!("AttachToOtherTmux action should be handled in main loop, not here");
                    self.ui_needs_refresh = true;
                }
                AsyncAction::KillOtherTmux(_session_name) => {
                    // NOTE: This action must be handled in main.rs where terminal access is available
                    warn!("KillOtherTmux action should be handled in main loop, not here");
                    self.ui_needs_refresh = true;
                }
            }
        }
        Ok(())
    }

    /// Run OAuth authentication setup
    async fn run_oauth_setup(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        use crossterm::{
            execute,
            terminal::{LeaveAlternateScreen, disable_raw_mode},
        };

        // Create auth directory
        let home_dir = dirs::home_dir().ok_or("Could not determine home directory")?;
        let auth_dir = home_dir.join(".agents-in-a-box/auth");

        info!("Creating auth directory: {}", auth_dir.display());
        std::fs::create_dir_all(&auth_dir)?;

        // Update UI state to show we're starting
        if let Some(ref mut auth_state) = self.auth_setup_state {
            auth_state.is_processing = true;
            auth_state.error_message = Some("Preparing authentication setup...".to_string());
        }

        // First check if Docker is available
        if !self.is_docker_available().await {
            warn!("Docker is not available or not running");
            if let Some(ref mut auth_state) = self.auth_setup_state {
                auth_state.error_message = Some(
                    "❌ Docker is not available\n\n\
                     Please start Docker and try again."
                        .to_string(),
                );
                auth_state.is_processing = false;
            }
            return Err("Docker not available".into());
        }

        // Check if image exists
        let image_name = "agents-box:agents-dev";
        let image_check = std::process::Command::new("docker")
            .args(["image", "inspect", image_name])
            .output()?;

        if !image_check.status.success() {
            info!("Building agents-dev image...");
            let build_status = std::process::Command::new("docker")
                .args(["build", "-t", image_name, "docker/agents-dev"])
                .status()?;

            if !build_status.success() {
                if let Some(ref mut auth_state) = self.auth_setup_state {
                    auth_state.error_message = Some(
                        "❌ Failed to build claude-dev image\n\n\
                         Please check Docker and try again."
                            .to_string(),
                    );
                    auth_state.is_processing = false;
                }
                return Err("Failed to build image".into());
            }
        }

        // Temporarily exit TUI to run interactive container
        info!("Exiting TUI to run interactive authentication");

        // Disable raw mode and restore terminal
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen);

        println!("\n🔐 Claude Authentication Setup\n");
        println!("This will guide you through the OAuth authentication process.");
        println!("You'll be prompted to open a URL in your browser to complete authentication.\n");

        // Run the auth container interactively
        // Use inherit for stdin/stdout/stderr to ensure proper TTY forwarding
        let status = std::process::Command::new("docker")
            .args([
                "run",
                "--rm",
                "-it",
                "-v",
                &format!("{}:/home/claude-user/.claude", auth_dir.display()),
                "-e",
                "PATH=/home/claude-user/.npm-global/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
                "-e",
                "HOME=/home/claude-user",
                "-e",
                "AUTH_METHOD=oauth",  // Specify OAuth method
                "-w",
                "/home/claude-user",
                "--user",
                "claude-user",
                "--entrypoint",
                "bash",
                image_name,
                "-c",
                "/app/scripts/auth-setup.sh",
            ])
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()?;

        // Check if authentication was successful
        let credentials_path = auth_dir.join(".credentials.json");
        let success =
            status.success() && credentials_path.exists() && credentials_path.metadata()?.len() > 0;

        if success {
            println!("\n✅ Authentication successful!");
            println!("Press Enter to continue...");
            let _ = std::io::stdin().read_line(&mut String::new());

            // Success - transition to main view
            self.auth_setup_state = None;
            self.current_view = View::SessionList;
            self.check_current_directory_status();
            self.pending_async_action = Some(AsyncAction::RefreshWorkspaces);
        } else {
            println!("\n❌ Authentication failed!");
            println!("Press Enter to return to the authentication menu...");
            let _ = std::io::stdin().read_line(&mut String::new());

            if let Some(ref mut auth_state) = self.auth_setup_state {
                auth_state.error_message = Some(
                    "❌ Authentication failed\n\n\
                     Please try again or use API Key method."
                        .to_string(),
                );
                auth_state.is_processing = false;
            }
        }

        // Re-enable raw mode and return to TUI
        use crossterm::terminal::{EnterAlternateScreen, enable_raw_mode};
        let _ = enable_raw_mode();
        let _ = execute!(std::io::stdout(), EnterAlternateScreen);

        // Force UI refresh
        self.ui_needs_refresh = true;

        Ok(())
    }

    /// Check if Docker is available and running (synchronous, static version)
    pub fn is_docker_available_sync() -> bool {
        use std::process::{Command, Stdio};

        match Command::new("docker")
            .arg("info")
            .stdout(Stdio::null())  // Suppress stdout
            .stderr(Stdio::null())  // Suppress stderr
            .status()
        {
            Ok(status) => status.success(),
            Err(_) => false,
        }
    }

    /// Check if Docker is available and running
    async fn is_docker_available(&self) -> bool {
        // Try to run a simple docker command to check if Docker is available
        match std::process::Command::new("docker")
            .args(["version", "--format", "{{.Server.Version}}"])
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    let version = String::from_utf8_lossy(&output.stdout);
                    info!("Docker is available, version: {}", version.trim());
                    true
                } else {
                    let error = String::from_utf8_lossy(&output.stderr);
                    warn!("Docker command failed: {}", error);
                    false
                }
            }
            Err(e) => {
                warn!("Docker not found or not accessible: {}", e);
                false
            }
        }
    }

    /// Save API key authentication
    async fn save_api_key(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let api_key = match &self.auth_setup_state {
            Some(auth_state) => auth_state.api_key_input.clone(),
            None => return Err("No API key to save".into()),
        };

        // Validate API key format
        if !api_key.starts_with("sk-") || api_key.len() < 20 {
            return Err("Invalid API key format".into());
        }

        // Create .env file in agents-in-a-box directory
        let home_dir = dirs::home_dir().ok_or("Could not determine home directory")?;
        let claude_box_dir = home_dir.join(".agents-in-a-box");
        std::fs::create_dir_all(&claude_box_dir)?;

        let env_path = claude_box_dir.join(".env");
        std::fs::write(&env_path, format!("ANTHROPIC_API_KEY={}\n", api_key))?;

        info!("API key saved to {:?}", env_path);

        // Success - transition to main view
        self.auth_setup_state = None;
        self.current_view = View::SessionList;
        self.check_current_directory_status();
        self.pending_async_action = Some(AsyncAction::RefreshWorkspaces);

        Ok(())
    }

    /// Handle re-authentication of Claude credentials
    async fn handle_reauthenticate(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Check if any sessions are currently running
        let running_session_count =
            self.workspaces.iter().map(|w| w.running_sessions().len()).sum::<usize>();

        if running_session_count > 0 {
            warn!(
                "Found {} running sessions - re-authentication will affect them",
                running_session_count
            );

            // For now, we'll show an error and require manual session cleanup
            // TODO: Add confirmation dialog with option to stop sessions automatically
            if let Some(ref mut auth_state) = self.auth_setup_state {
                auth_state.error_message = Some(format!(
                    "❌ Cannot re-authenticate with {} running sessions\n\n\
                     Running sessions use the current credentials.\n\
                     Please stop all sessions before re-authenticating.\n\n\
                     Use 'd' to delete sessions or wait for them to complete.",
                    running_session_count
                ));
                auth_state.is_processing = false;
            } else {
                // Create auth state to show the error
                self.auth_setup_state = Some(AuthSetupState {
                    selected_method: AuthMethod::OAuth,
                    api_key_input: String::new(),
                    is_processing: false,
                    show_cursor: false,
                    error_message: Some(format!(
                        "❌ Cannot re-authenticate with {} running sessions\n\n\
                         Running sessions use the current credentials.\n\
                         Please stop all sessions before re-authenticating.\n\n\
                         Use 'd' to delete sessions or wait for them to complete.",
                        running_session_count
                    )),
                });
                self.current_view = View::AuthSetup;
            }
            return Ok(());
        }

        // No running sessions - safe to proceed with re-authentication
        info!("No running sessions found - proceeding with re-authentication");

        // Create backup of existing credentials
        let home_dir = dirs::home_dir().ok_or("Could not determine home directory")?;
        let auth_dir = home_dir.join(".agents-in-a-box/auth");

        let credentials_path = auth_dir.join(".credentials.json");
        let claude_json_path = auth_dir.join(".claude.json");
        let backup_suffix = format!(".backup-{}", chrono::Utc::now().timestamp());

        // Create backups if files exist
        if credentials_path.exists() {
            let backup_path = credentials_path.with_extension(&format!("json{}", backup_suffix));
            std::fs::copy(&credentials_path, &backup_path)?;
            info!("Backed up credentials to {:?}", backup_path);
        }

        if claude_json_path.exists() {
            let backup_path = claude_json_path.with_extension(&format!("json{}", backup_suffix));
            std::fs::copy(&claude_json_path, &backup_path)?;
            info!("Backed up claude.json to {:?}", backup_path);
        }

        // Remove existing credentials to trigger re-authentication
        if credentials_path.exists() {
            std::fs::remove_file(&credentials_path)?;
            info!("Removed existing credentials");
        }

        if claude_json_path.exists() {
            std::fs::remove_file(&claude_json_path)?;
            info!("Removed existing claude.json");
        }

        // Initialize auth setup state and switch to auth view
        self.auth_setup_state = Some(AuthSetupState {
            selected_method: AuthMethod::OAuth, // Default to OAuth
            api_key_input: String::new(),
            is_processing: false,
            show_cursor: false,
            error_message: Some(
                "🔄 Previous credentials cleared - please authenticate again".to_string(),
            ),
        });
        self.current_view = View::AuthSetup;

        info!("Re-authentication initiated - switched to auth setup view");
        Ok(())
    }

    async fn handle_restart_session(
        &mut self,
        session_id: Uuid,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("Initiating restart UI flow for session {}", session_id);

        // Find the session in our workspace list
        let session_info = self.workspaces.iter().find_map(|workspace| {
            workspace
                .sessions
                .iter()
                .find(|s| s.id == session_id)
                .map(|session| (workspace, session))
        });

        if let Some((workspace, session)) = session_info {
            match &session.status {
                crate::models::SessionStatus::Stopped => {
                    info!(
                        "Session {} is stopped, starting restart UI flow",
                        session_id
                    );

                    // Start the new session UI flow with pre-populated data from the existing session
                    self.current_view = View::NewSession;
                    self.new_session_state = Some(NewSessionState {
                        available_repos: vec![workspace.path.clone()],
                        filtered_repos: vec![(0, workspace.path.clone())],
                        selected_repo_index: Some(0),
                        branch_name: session.branch_name.clone(),
                        step: NewSessionStep::InputBranch, // Start at branch input since repo is pre-selected
                        filter_text: String::new(),
                        is_current_dir_mode: false,
                        skip_permissions: session.skip_permissions,
                        mode: session.mode.clone(),
                        boss_prompt: if let Some(ref prompt) = session.boss_prompt {
                            TextEditor::from_string(prompt)
                        } else {
                            TextEditor::new()
                        },
                        file_finder: FuzzyFileFinderState::new(),
                        restart_session_id: Some(session_id), // Mark this as a restart operation
                    });

                    self.add_info_notification(
                        "🔄 Restarting session - review and update settings as needed".to_string(),
                    );
                }
                crate::models::SessionStatus::Idle => {
                    info!(
                        "Session {} is idle (tmux running but Claude stopped), restarting Claude in tmux",
                        session_id
                    );

                    // For Idle sessions, we restart Claude within the existing tmux session
                    if let Err(e) = self.restart_claude_in_tmux(session_id).await {
                        error!("Failed to restart Claude in tmux for session {}: {}", session_id, e);
                        self.add_error_notification(format!(
                            "❌ Failed to restart Claude: {}",
                            e
                        ));
                    } else {
                        self.add_success_notification(
                            "✓ Claude restarted successfully".to_string(),
                        );
                    }
                }
                status => {
                    warn!(
                        "Cannot restart session {} - current status: {:?}",
                        session_id, status
                    );
                    self.add_error_notification(format!(
                        "❌ Cannot restart session - current status: {:?}",
                        status
                    ));
                }
            }
        } else {
            error!("Session {} not found in workspaces", session_id);
            self.add_error_notification("❌ Session not found".to_string());
        }

        Ok(())
    }

    pub fn show_git_view(&mut self) {
        // Get the selected session's workspace path
        if let Some(session) = self.get_selected_session() {
            let worktree_path = std::path::PathBuf::from(&session.workspace_path);
            let mut git_state = crate::components::GitViewState::new(worktree_path);

            // Refresh git status
            if let Err(e) = git_state.refresh_git_status() {
                tracing::error!("Failed to refresh git status: {}", e);
                return;
            }

            self.git_view_state = Some(git_state);
            self.current_view = View::GitView;
        } else {
            tracing::warn!("No session selected for git view");
        }
    }

    pub fn git_commit_and_push(&mut self) {
        let result = if let Some(git_state) = self.git_view_state.as_mut() {
            git_state.commit_and_push()
        } else {
            return;
        };

        match result {
            Ok(message) => {
                tracing::info!("Git commit and push successful: {}", message);
                // Set pending event to be processed in next loop iteration
                self.pending_event = Some(crate::app::events::AppEvent::GitCommitSuccess(message));
                // Refresh git status after successful push
                if let Some(git_state) = self.git_view_state.as_mut() {
                    if let Err(e) = git_state.refresh_git_status() {
                        tracing::error!("Failed to refresh git status after push: {}", e);
                        self.add_warning_notification(
                            "⚠️ Push successful but failed to refresh git status".to_string(),
                        );
                    }
                }
            }
            Err(e) => {
                tracing::error!("Git commit and push failed: {}", e);
                self.add_error_notification(format!("❌ Git push failed: {}", e));
            }
        }
    }

    // Quick commit dialog methods
    pub fn is_in_quick_commit_mode(&self) -> bool {
        self.quick_commit_message.is_some()
    }

    pub fn start_quick_commit(&mut self) {
        // Only start quick commit if we have a selected session and it's in a git repository
        if let Some(session) = self.get_selected_session() {
            // Check if the workspace path is a git repository
            let workspace_path = std::path::Path::new(&session.workspace_path);
            let git_dir = workspace_path.join(".git");

            if git_dir.exists() {
                self.quick_commit_message = Some(String::new());
                self.quick_commit_cursor = 0;
                self.add_info_notification(
                    "📝 Enter commit message and press Enter to commit & push".to_string(),
                );
            } else {
                self.add_warning_notification(
                    "⚠️ Selected workspace is not a git repository".to_string(),
                );
            }
        } else {
            self.add_warning_notification("⚠️ No session selected".to_string());
        }
    }

    pub fn cancel_quick_commit(&mut self) {
        self.quick_commit_message = None;
        self.quick_commit_cursor = 0;
        self.add_info_notification("❌ Quick commit cancelled".to_string());
    }

    pub fn add_char_to_quick_commit(&mut self, ch: char) {
        if let Some(ref mut message) = self.quick_commit_message {
            message.insert(self.quick_commit_cursor, ch);
            self.quick_commit_cursor += 1;
        }
    }

    pub fn backspace_quick_commit(&mut self) {
        if let Some(ref mut message) = self.quick_commit_message {
            if self.quick_commit_cursor > 0 {
                self.quick_commit_cursor -= 1;
                message.remove(self.quick_commit_cursor);
            }
        }
    }

    pub fn move_quick_commit_cursor_left(&mut self) {
        if self.quick_commit_cursor > 0 {
            self.quick_commit_cursor -= 1;
        }
    }

    pub fn move_quick_commit_cursor_right(&mut self) {
        if let Some(ref message) = self.quick_commit_message {
            if self.quick_commit_cursor < message.len() {
                self.quick_commit_cursor += 1;
            }
        }
    }

    pub fn confirm_quick_commit(&mut self) {
        if let Some(ref message) = self.quick_commit_message {
            if message.trim().is_empty() {
                self.add_warning_notification("⚠️ Commit message cannot be empty".to_string());
                return;
            }

            // Perform the quick commit
            self.perform_quick_commit(message.trim().to_string());
        }
    }

    fn perform_quick_commit(&mut self, commit_message: String) {
        let worktree_path = if let Some(session) = self.get_selected_session() {
            std::path::PathBuf::from(&session.workspace_path)
        } else {
            return;
        };

        // Use the shared git operations function - DRY compliance!
        match crate::git::operations::commit_and_push_changes(&worktree_path, &commit_message) {
            Ok(success_message) => {
                tracing::info!("Quick commit successful: {}", success_message);
                // Set pending event to be processed in next loop iteration
                self.pending_event = Some(crate::app::events::AppEvent::GitCommitSuccess(
                    success_message,
                ));
                // Clear quick commit state
                self.quick_commit_message = None;
                self.quick_commit_cursor = 0;
            }
            Err(e) => {
                tracing::error!("Quick commit failed: {}", e);
                self.add_error_notification(format!("❌ Quick commit failed: {}", e));
                // Keep quick commit dialog open so user can try again
            }
        }
    }

    /// Add a notification to the notification queue
    pub fn add_notification(&mut self, notification: Notification) {
        self.notifications.push(notification);
    }

    /// Add a success notification
    pub fn add_success_notification(&mut self, message: String) {
        self.add_notification(Notification::success(message));
    }

    /// Add an error notification
    pub fn add_error_notification(&mut self, message: String) {
        self.add_notification(Notification::error(message));
    }

    /// Add an info notification
    pub fn add_info_notification(&mut self, message: String) {
        self.add_notification(Notification::info(message));
    }

    /// Add a warning notification
    pub fn add_warning_notification(&mut self, message: String) {
        self.add_notification(Notification::warning(message));
    }

    /// Remove expired notifications
    pub fn cleanup_expired_notifications(&mut self) {
        self.notifications.retain(|n| !n.is_expired());
    }

    /// Get current notifications (non-expired)
    pub fn get_current_notifications(&self) -> Vec<&Notification> {
        self.notifications.iter().filter(|n| !n.is_expired()).collect()
    }

    // ============================================================================
    // Tmux Integration Methods
    // ============================================================================

    /// Start background task to update tmux preview content every 100ms
    /// NOTE: This is now handled via the main update loop calling update_tmux_previews()
    /// This method is kept for compatibility but does nothing
    pub fn start_preview_updates(&mut self) {
        // Preview updates are now handled by calling update_tmux_previews() from main loop
        // No background task needed
        info!("Preview updates will be handled via main update loop");
    }

    /// Stop the preview update task
    pub fn stop_preview_updates(&mut self) {
        if let Some(task) = self.preview_update_task.take() {
            task.abort();
        }
    }

    /// Update preview content for all tmux sessions (called from main update loop)
    pub async fn update_tmux_previews(&mut self) -> anyhow::Result<()> {
        use crate::tmux::ClaudeProcessDetector;

        // Collect session IDs, preview content, and health status to avoid borrowing conflicts
        let mut updates = Vec::new();
        let detector = ClaudeProcessDetector::new();

        for (session_id, tmux_session) in &self.tmux_sessions {
            // Check if session is attached (without mutable borrow)
            let should_update = self
                .workspaces
                .iter()
                .flat_map(|w| &w.sessions)
                .find(|s| s.id == *session_id)
                .map(|s| !s.is_attached)
                .unwrap_or(false);

            if should_update {
                // Capture full scrollback history for preview (allows scrolling through history)
                match tmux_session.capture_full_history().await {
                    Ok(content) => {
                        // Check if Claude is running by analyzing the content
                        let claude_running = detector.has_claude_status_bar(&content);
                        updates.push((*session_id, content, claude_running));
                    }
                    Err(e) => {
                        warn!("Failed to capture tmux pane content for session {}: {}", session_id, e);
                    }
                }
            }
        }

        // Apply updates
        for (session_id, content, claude_running) in updates {
            if let Some(session) = self.find_session_mut(session_id) {
                session.set_preview(content);

                // Update session status based on Claude health
                use crate::models::SessionStatus;
                let new_status = if claude_running {
                    SessionStatus::Running
                } else {
                    SessionStatus::Idle
                };

                // Only update if status changed to avoid unnecessary refreshes
                if session.status != new_status {
                    session.set_status(new_status);
                    info!(
                        "Session {} status updated to: {}",
                        session_id,
                        if claude_running { "Running" } else { "Idle" }
                    );
                }

                self.ui_needs_refresh = true;
            }
        }

        Ok(())
    }

    /// Restart Claude in an existing tmux session (for Idle sessions)
    async fn restart_claude_in_tmux(&mut self, session_id: Uuid) -> anyhow::Result<()> {
        use anyhow::Context;
        use std::process::Command;

        // Get session details
        let session = self
            .find_session(session_id)
            .ok_or_else(|| anyhow::anyhow!("Session not found"))?;

        let tmux_session_name = session
            .tmux_session_name
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No tmux session associated with this session"))?
            .clone();

        let workspace_path = session.workspace_path.clone();
        let skip_permissions = session.skip_permissions;

        info!(
            "Restarting Claude in tmux session '{}' for workspace '{}'",
            tmux_session_name, workspace_path
        );

        // Send 'claude' command to the tmux session
        // This assumes the user stopped Claude with Ctrl+C or it crashed
        let claude_cmd = if skip_permissions {
            "claude --dangerously-skip-permissions".to_string()
        } else {
            "claude".to_string()
        };

        // Send the command to tmux using 'send-keys'
        let output = Command::new("tmux")
            .args(&[
                "send-keys",
                "-t",
                &tmux_session_name,
                &claude_cmd,
                "C-m", // Press Enter
            ])
            .output()
            .context("Failed to send claude command to tmux")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to send command to tmux: {}", stderr);
        }

        // Update session status to Running (will be confirmed by next preview update)
        if let Some(session) = self.find_session_mut(session_id) {
            session.set_status(crate::models::SessionStatus::Running);
        }

        info!("Successfully sent Claude restart command to tmux session '{}'", tmux_session_name);
        Ok(())
    }

    /// Helper to find a session by ID across all workspaces
    fn find_session(&self, session_id: uuid::Uuid) -> Option<&crate::models::session::Session> {
        for workspace in &self.workspaces {
            for session in &workspace.sessions {
                if session.id == session_id {
                    return Some(session);
                }
            }
        }
        None
    }

    /// Helper to find a mutable session by ID across all workspaces
    fn find_session_mut(&mut self, session_id: uuid::Uuid) -> Option<&mut crate::models::session::Session> {
        for workspace in &mut self.workspaces {
            for session in &mut workspace.sessions {
                if session.id == session_id {
                    return Some(session);
                }
            }
        }
        None
    }
}

pub struct App {
    pub state: AppState,
}

impl App {
    pub fn new() -> Self {
        Self {
            state: AppState::new(),
        }
    }

    pub async fn init(&mut self) {
        // Initialize log streaming coordinator
        let (mut coordinator, log_sender) = LogStreamingCoordinator::new();

        // Only initialize the streaming manager if Docker is available
        // (log streaming requires Docker for Boss mode containers)
        if AppState::is_docker_available_sync() {
            info!("Docker available - initializing log streaming manager");
            if let Err(e) = coordinator.init_manager(log_sender.clone()) {
                warn!("Failed to initialize log streaming manager: {}", e);
            } else {
                info!("Log streaming coordinator initialized successfully");
            }
        } else {
            info!("Docker not available - skipping log streaming manager initialization");
            info!("Log streaming will be available when Docker is started");
        }

        self.state.log_streaming_coordinator = Some(coordinator);
        self.state.log_sender = Some(log_sender);

        // Try to refresh OAuth tokens if they're expired (before checking first-time setup)
        let home_dir = dirs::home_dir();
        if let Some(home) = home_dir {
            let credentials_path =
                home.join(".agents-in-a-box").join("auth").join(".credentials.json");

            // Only attempt refresh if we have OAuth credentials that need refreshing
            // AND Docker is available (token refresh requires Docker for Boss mode)
            if credentials_path.exists() && AppState::oauth_token_needs_refresh(&credentials_path) {
                if self.state.is_docker_available().await {
                    info!("Docker available - attempting OAuth token refresh on startup");
                    match self.state.refresh_oauth_tokens().await {
                        Ok(()) => info!("OAuth tokens refreshed successfully on startup"),
                        Err(e) => warn!("Failed to refresh OAuth tokens: {}", e),
                    }
                } else {
                    info!("Docker not available - skipping OAuth token refresh (Boss mode will require Docker)");
                    // Don't show error - user might only use Interactive mode which doesn't need Docker
                }
            }
        }

        // REMOVED: Auth check moved to Boss mode selection only
        // Interactive mode should work without Docker authentication
        // Authentication is only required for Boss mode (Docker-based sessions)
        info!("App::init() - skipping upfront auth check (deferred to Boss mode selection)");

        // Always start with SessionList view
        info!("Starting with SessionList view (auth deferred until Boss mode)");
        // Initialize Claude integration
        if let Err(e) = self.state.init_claude_integration().await {
            warn!("Failed to initialize Claude integration: {}", e);
        }

        self.state.check_current_directory_status();
        self.state.load_real_workspaces().await;

        // Start log streaming for any running sessions
        if let Err(e) = self.init_log_streaming_for_sessions().await {
            warn!(
                "Failed to initialize log streaming for existing sessions: {}",
                e
            );
        }
    }

    /// Initialize log streaming for all running sessions
    async fn init_log_streaming_for_sessions(&mut self) -> anyhow::Result<()> {
        if let Some(coordinator) = &mut self.state.log_streaming_coordinator {
            // Collect session info for streaming
            let sessions: Vec<(Uuid, String, String, crate::models::SessionMode)> = self
                .state
                .workspaces
                .iter()
                .flat_map(|w| &w.sessions)
                .filter(|s| s.status == crate::models::SessionStatus::Running)
                .filter_map(|s| {
                    s.container_id.clone().map(|container_id| {
                        (
                            s.id,
                            container_id,
                            format!("{}-{}", s.name, s.branch_name),
                            s.mode.clone(),
                        )
                    })
                })
                .collect();

            if !sessions.is_empty() {
                info!(
                    "Starting log streaming for {} running sessions",
                    sessions.len()
                );
                for (session_id, container_id, container_name, session_mode) in &sessions {
                    if let Err(e) = coordinator
                        .start_streaming(
                            *session_id,
                            container_id.clone(),
                            container_name.clone(),
                            session_mode.clone(),
                        )
                        .await
                    {
                        warn!(
                            "Failed to start log streaming for session {}: {}",
                            session_id, e
                        );
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn tick(&mut self) -> anyhow::Result<()> {
        // Clean up expired notifications
        self.state.cleanup_expired_notifications();

        // Periodic OAuth token refresh check (every 5 minutes)
        let now = Instant::now();
        let should_check_token = self
            .state
            .last_token_refresh_check
            .map(|last| now.duration_since(last).as_secs() >= 300) // Check every 5 minutes
            .unwrap_or(true); // First time

        if should_check_token {
            self.state.last_token_refresh_check = Some(now);

            // Check if we need to refresh OAuth tokens
            let home_dir = dirs::home_dir();
            if let Some(home) = home_dir {
                let credentials_path =
                    home.join(".agents-in-a-box").join("auth").join(".credentials.json");

                if credentials_path.exists()
                    && AppState::oauth_token_needs_refresh(&credentials_path)
                {
                    info!("OAuth token needs refresh (periodic check)");

                    // Only attempt refresh if Docker is available
                    if self.state.is_docker_available().await {
                        // Refresh tokens inline (this is quick enough not to block UI)
                        match self.state.refresh_oauth_tokens().await {
                            Ok(()) => {
                                info!("OAuth tokens refreshed successfully (periodic)");
                                // Add a notification to inform the user
                                self.state.add_notification(Notification {
                                    message: "✅ OAuth tokens refreshed automatically".to_string(),
                                    notification_type: NotificationType::Success,
                                    created_at: Instant::now(),
                                    duration: Duration::from_secs(5),
                                });
                            }
                            Err(e) => {
                                warn!("Failed to refresh OAuth tokens (periodic): {}", e);
                                // Add a warning notification
                                self.state.add_notification(Notification {
                                    message: format!("⚠️ Token refresh failed: {}", e),
                                    notification_type: NotificationType::Warning,
                                    created_at: Instant::now(),
                                    duration: Duration::from_secs(10),
                                });
                            }
                        }
                    } else {
                        info!("Docker not available - skipping periodic OAuth token refresh");
                    }
                }
            }
        }

        // Process incoming log entries (non-blocking)
        let mut log_entries = Vec::new();
        if let Some(coordinator) = &mut self.state.log_streaming_coordinator {
            // Collect all available log entries without blocking
            while let Some((session_id, log_entry)) = coordinator.try_next_log() {
                log_entries.push((session_id, log_entry));
            }
        }

        // Add log entries to the state
        for (session_id, log_entry) in log_entries {
            self.state.add_live_log(session_id, log_entry);
        }

        // Update tmux session previews for Interactive mode sessions
        // This captures pane content from tmux and updates session.preview_content
        if let Err(e) = self.state.update_tmux_previews().await {
            warn!("Failed to update tmux previews: {}", e);
        }

        // Process any pending async actions
        if self.state.pending_async_action.is_some() {
            info!(">>> tick() detected pending_async_action: {:?}", self.state.pending_async_action);
        }
        match self.state.process_async_action().await {
            Ok(()) => {
                if self.state.pending_async_action.is_some() {
                    info!(">>> After process_async_action, still pending: {:?}", self.state.pending_async_action);
                }
            }
            Err(e) => {
                warn!("Error processing async action: {}", e);
                // Return to safe state if there was an error
                self.state.new_session_state = None;
                self.state.current_view = View::SessionList;
                self.state.pending_async_action = None;
            }
        }

        // Update logic for the app (e.g., refresh container status)

        // Periodic log updates for attached sessions
        let now = Instant::now();
        let should_update_logs = self
            .state
            .last_log_check
            .map(|last| now.duration_since(last).as_secs() >= 3) // Update every 3 seconds
            .unwrap_or(true); // First time

        if should_update_logs {
            self.state.last_log_check = Some(now);

            // If we have an attached session, fetch its logs
            if let Some(attached_id) = self.state.attached_session_id {
                // Check if we should update this session's logs (don't spam updates)
                let should_update_session = self
                    .state
                    .log_last_updated
                    .get(&attached_id)
                    .map(|last| now.duration_since(*last).as_secs() >= 2) // Update session logs every 2 seconds
                    .unwrap_or(true);

                if should_update_session {
                    // Fetch logs in the background (don't block the UI)
                    if let Err(e) = self.state.fetch_claude_logs(attached_id).await {
                        warn!("Failed to fetch logs for session {}: {}", attached_id, e);
                    } else {
                        self.state.log_last_updated.insert(attached_id, now);
                        // Set flag to refresh UI with new logs
                        self.state.ui_needs_refresh = true;
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if UI needs immediate refresh and clear the flag
    pub fn needs_ui_refresh(&mut self) -> bool {
        if self.state.ui_needs_refresh {
            self.state.ui_needs_refresh = false;
            true
        } else {
            false
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

// Include the test module inline
#[cfg(test)]
#[path = "state_tests.rs"]
mod state_tests;
