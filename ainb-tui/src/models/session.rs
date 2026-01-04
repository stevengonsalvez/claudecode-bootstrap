// ABOUTME: Session data model representing a Claude Code container instance with git worktree

#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionMode {
    Interactive, // Traditional interactive mode with shell access
    Boss,        // Non-interactive mode with direct prompt execution
}

impl Default for SessionMode {
    fn default() -> Self {
        SessionMode::Interactive
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    Running,
    Stopped,
    Idle,  // Tmux exists but Claude stopped
    Error(String),
}

impl SessionStatus {
    pub fn indicator(&self) -> &'static str {
        match self {
            SessionStatus::Running => "●",
            SessionStatus::Stopped => "⏸",
            SessionStatus::Idle => "○",  // Empty circle for idle
            SessionStatus::Error(_) => "✗",
        }
    }

    pub fn is_running(&self) -> bool {
        matches!(self, SessionStatus::Running)
    }

    /// Helper to check if session can be restarted
    pub fn can_restart(&self) -> bool {
        matches!(self, SessionStatus::Idle | SessionStatus::Error(_))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub name: String,
    pub workspace_path: String,
    pub branch_name: String,
    pub container_id: Option<String>,
    pub status: SessionStatus,
    pub created_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
    pub git_changes: GitChanges,
    pub recent_logs: Option<String>,
    pub skip_permissions: bool, // Whether to use --dangerously-skip-permissions flag
    pub mode: SessionMode,      // Interactive or Boss mode
    pub boss_prompt: Option<String>, // The prompt for boss mode execution

    // Tmux integration fields
    pub tmux_session_name: Option<String>, // Name of the tmux session if using tmux backend
    pub preview_content: Option<String>,   // Cached preview content for display
    pub is_attached: bool,                 // Whether user is currently attached to the session
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GitChanges {
    pub added: u32,
    pub modified: u32,
    pub deleted: u32,
}

impl GitChanges {
    pub fn total(&self) -> u32 {
        self.added + self.modified + self.deleted
    }

    pub fn format(&self) -> String {
        if self.total() == 0 {
            "No changes".to_string()
        } else {
            format!("+{} ~{} -{}", self.added, self.modified, self.deleted)
        }
    }
}

impl Session {
    pub fn new(name: String, workspace_path: String) -> Self {
        Self::new_with_options(name, workspace_path, false, SessionMode::Interactive, None)
    }

    pub fn new_with_options(
        name: String,
        workspace_path: String,
        skip_permissions: bool,
        mode: SessionMode,
        boss_prompt: Option<String>,
    ) -> Self {
        let now = Utc::now();
        let branch_name = format!("agents-in-a-box/{}", name.replace(' ', "-").to_lowercase());

        Self {
            id: Uuid::new_v4(),
            name,
            workspace_path,
            branch_name,
            container_id: None,
            status: SessionStatus::Stopped,
            created_at: now,
            last_accessed: now,
            git_changes: GitChanges::default(),
            recent_logs: None,
            skip_permissions,
            mode,
            boss_prompt,
            tmux_session_name: None,
            preview_content: None,
            is_attached: false,
        }
    }

    pub fn update_last_accessed(&mut self) {
        self.last_accessed = Utc::now();
    }

    pub fn set_status(&mut self, status: SessionStatus) {
        self.status = status;
        self.update_last_accessed();
    }

    pub fn set_container_id(&mut self, container_id: Option<String>) {
        self.container_id = container_id;
        self.update_last_accessed();
    }

    // Tmux integration methods

    /// Get the tmux session name for this session
    /// Format: tmux_{sanitized_name}
    pub fn get_tmux_name(&self) -> String {
        format!(
            "tmux_{}",
            self.name.replace(' ', "_").replace('.', "_").replace('/', "_")
        )
    }

    /// Set the preview content for this session
    pub fn set_preview(&mut self, content: String) {
        self.preview_content = Some(content);
        self.update_last_accessed();
    }

    /// Mark the session as attached
    pub fn mark_attached(&mut self) {
        self.is_attached = true;
        self.update_last_accessed();
    }

    /// Mark the session as detached
    pub fn mark_detached(&mut self) {
        self.is_attached = false;
        self.update_last_accessed();
    }

    /// Set the tmux session name
    pub fn set_tmux_session_name(&mut self, name: String) {
        self.tmux_session_name = Some(name);
        self.update_last_accessed();
    }
}
