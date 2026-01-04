// ABOUTME: Interactive session manager for host-based Docker-free sessions
//
// Manages the lifecycle of Interactive mode sessions which run directly on the host:
// - Creates git worktrees for branch isolation
// - Starts tmux sessions for terminal multiplexing
// - Runs claude CLI directly on the host
// - Discovers existing sessions by scanning tmux
//
// This manager is completely independent of Docker and ContainerManager,
// enabling lightweight, fast development workflows.

#![allow(dead_code)]

use crate::git::WorktreeManager;
use crate::models::{Session, SessionMode, SessionStatus};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::process::Command;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum InteractiveSessionError {
    #[error("Worktree error: {0}")]
    Worktree(#[from] crate::git::WorktreeError),

    #[error("Tmux error: {0}")]
    Tmux(String),

    #[error("Session not found: {0}")]
    SessionNotFound(Uuid),

    #[error("Session already exists: {0}")]
    SessionAlreadyExists(Uuid),

    #[error("Invalid session state: {0}")]
    InvalidState(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Error: {0}")]
    Other(#[from] anyhow::Error),
}

/// Represents an active Interactive mode session
#[derive(Debug, Clone)]
pub struct InteractiveSession {
    pub session_id: Uuid,
    pub worktree_path: PathBuf,
    pub source_repository: PathBuf, // The original git repository path
    pub tmux_session_name: String,
    pub branch_name: String,
    pub workspace_name: String,
    pub created_at: DateTime<Utc>,
}

/// Manager for Interactive mode sessions (host-based, no Docker)
pub struct InteractiveSessionManager {
    worktree_manager: WorktreeManager,
    active_sessions: HashMap<Uuid, InteractiveSession>,
}

impl InteractiveSessionManager {
    /// Create a new Interactive session manager
    ///
    /// NOTE: This does NOT require Docker, unlike SessionLifecycleManager
    pub fn new() -> Result<Self, InteractiveSessionError> {
        let worktree_manager = WorktreeManager::new()
            .map_err(|e| InteractiveSessionError::InvalidState(format!("Failed to create worktree manager: {}", e)))?;

        Ok(Self {
            worktree_manager,
            active_sessions: HashMap::new(),
        })
    }

    /// Create a new Interactive session with worktree and tmux
    ///
    /// # Arguments
    /// * `session_id` - Unique identifier for the session
    /// * `workspace_name` - Name of the workspace
    /// * `workspace_path` - Path to the git repository
    /// * `branch_name` - Branch name to create worktree for
    /// * `base_branch` - Optional base branch to branch from
    ///
    /// # Returns
    /// * `Result<InteractiveSession>` - The created session or an error
    pub async fn create_session(
        &mut self,
        session_id: Uuid,
        workspace_name: String,
        workspace_path: PathBuf,
        branch_name: String,
        base_branch: Option<String>,
        skip_permissions: bool,
    ) -> Result<InteractiveSession, InteractiveSessionError> {
        info!(
            "Creating Interactive session {} for branch '{}' in workspace '{}' (skip_permissions={})",
            session_id, branch_name, workspace_name, skip_permissions
        );

        // Check if session already exists
        if self.active_sessions.contains_key(&session_id) {
            return Err(InteractiveSessionError::SessionAlreadyExists(session_id));
        }

        // Step 1: Create git worktree
        info!("Creating worktree for branch '{}'", branch_name);
        let worktree_info = self.worktree_manager.create_worktree(
            session_id,
            &workspace_path,
            &branch_name,
            base_branch.as_deref(),
        )?;

        info!("Created worktree at: {}", worktree_info.path.display());

        // Step 2: Create tmux session name
        let tmux_session_name = Self::generate_tmux_name(&branch_name);

        // Step 3: Start tmux session
        info!("Starting tmux session: {}", tmux_session_name);
        self.start_tmux_session(&tmux_session_name, &worktree_info.path).await?;

        // Step 4: Start claude CLI in tmux session
        info!("Starting claude CLI in tmux session (skip_permissions={})", skip_permissions);
        self.start_claude_in_tmux(&tmux_session_name, skip_permissions).await?;

        // Step 5: Create session record
        let session = InteractiveSession {
            session_id,
            worktree_path: worktree_info.path.clone(),
            source_repository: worktree_info.source_repository.clone(),
            tmux_session_name: tmux_session_name.clone(),
            branch_name: branch_name.clone(),
            workspace_name: workspace_name.clone(),
            created_at: Utc::now(),
        };

        self.active_sessions.insert(session_id, session.clone());

        info!("Successfully created Interactive session {}", session_id);
        Ok(session)
    }

    /// Discover and list all active Interactive sessions by scanning tmux
    ///
    /// This enables stateless recovery - we can discover sessions created in
    /// previous app instances by matching tmux session names to worktrees.
    ///
    /// # Returns
    /// * `Result<Vec<InteractiveSession>>` - List of discovered sessions
    pub async fn list_sessions(&mut self) -> Result<Vec<InteractiveSession>, InteractiveSessionError> {
        info!("Discovering Interactive sessions from tmux");

        // Get all tmux sessions
        let output = Command::new("tmux")
            .args(["list-sessions", "-F", "#{session_name}"])
            .output()
            .await?;

        if !output.status.success() {
            // No tmux server running or no sessions
            debug!("No tmux sessions found (tmux might not be running)");
            return Ok(Vec::new());
        }

        let tmux_sessions = String::from_utf8_lossy(&output.stdout);
        let mut discovered_sessions = Vec::new();

        // Filter for our tmux sessions (prefix: tmux_)
        for tmux_name in tmux_sessions.lines() {
            if !tmux_name.starts_with("tmux_") {
                continue;
            }

            debug!("Found tmux session: {}", tmux_name);

            // Try to find corresponding worktree
            if let Ok(session) = self.discover_session_from_tmux(tmux_name).await {
                discovered_sessions.push(session);
            }
        }

        info!("Discovered {} Interactive sessions", discovered_sessions.len());
        Ok(discovered_sessions)
    }

    /// Discover a session from a tmux session name
    ///
    /// Matches tmux session to worktree by reverse-engineering the branch name
    async fn discover_session_from_tmux(&self, tmux_name: &str) -> Result<InteractiveSession, InteractiveSessionError> {
        // Remove "tmux_" prefix and reverse sanitization
        let sanitized = tmux_name.strip_prefix("tmux_").unwrap_or(tmux_name);
        let branch_guess = sanitized.replace('_', "/");

        // Try to find worktree with matching branch
        // Use list_all_worktrees() which scans by-session directory with UUID symlinks
        let worktrees = self.worktree_manager.list_all_worktrees()
            .map_err(|e| InteractiveSessionError::InvalidState(format!("Failed to list worktrees: {}", e)))?;

        for (session_id, worktree) in worktrees {
            if worktree.branch_name.contains(&branch_guess) ||
               Self::generate_tmux_name(&worktree.branch_name) == tmux_name {

                // Extract workspace name from worktree directory name
                // Worktree naming: <repo-name>--<branch-hash>--<session-id>
                // Split by "--" and take the first part (repo name)
                let workspace_name = worktree.path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .and_then(|name| {
                        // Split by "--" and take the first part (repo name)
                        name.split("--").next()
                    })
                    .unwrap_or_else(|| {
                        // Fallback to source repository name
                        worktree.source_repository
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown")
                    })
                    .to_string();

                return Ok(InteractiveSession {
                    session_id, // Use the session_id from the symlink directory
                    worktree_path: worktree.path,
                    source_repository: worktree.source_repository,
                    tmux_session_name: tmux_name.to_string(),
                    branch_name: worktree.branch_name,
                    workspace_name,
                    created_at: Utc::now(), // We don't persist creation time
                });
            }
        }

        Err(InteractiveSessionError::InvalidState(
            format!("No matching worktree found for tmux session {}", tmux_name)
        ))
    }

    /// Remove an Interactive session (cleanup tmux and worktree)
    ///
    /// # Arguments
    /// * `session_id` - UUID of the session to remove
    ///
    /// # Returns
    /// * `Result<()>` - Success or an error
    pub async fn remove_session(&mut self, session_id: Uuid) -> Result<(), InteractiveSessionError> {
        info!(">>> InteractiveSessionManager::remove_session() START: {}", session_id);

        // Try to get session from active_sessions first
        let session_opt = self.active_sessions.remove(&session_id);
        info!("Session in active_sessions: {}", session_opt.is_some());

        // Step 1: Kill tmux session
        // If we have the session in memory, use its tmux_session_name
        // Otherwise, try to find it by discovering from worktree
        let tmux_session_name = if let Some(ref session) = session_opt {
            info!("Using tmux session name from memory: {}", session.tmux_session_name);
            session.tmux_session_name.clone()
        } else {
            // Try to get worktree info and derive tmux session name
            info!("Session not in memory, discovering from worktree");
            match self.worktree_manager.get_worktree_info(session_id) {
                Ok(worktree) => {
                    info!("Found worktree with branch: {}", worktree.branch_name);
                    let tmux_name = Self::generate_tmux_name(&worktree.branch_name);
                    info!("Generated tmux session name: {}", tmux_name);
                    tmux_name
                }
                Err(e) => {
                    // Couldn't find worktree, can't determine tmux session name
                    error!("Could not find worktree for session {}: {}", session_id, e);
                    // Still try to remove worktree in case it exists
                    if let Err(remove_err) = self.worktree_manager.remove_worktree(session_id) {
                        warn!("Failed to remove worktree: {}", remove_err);
                    }
                    return Err(InteractiveSessionError::SessionNotFound(session_id));
                }
            }
        };

        info!("Attempting to kill tmux session: {}", tmux_session_name);
        let output = Command::new("tmux")
            .args(["kill-session", "-t", &tmux_session_name])
            .output()
            .await?;

        if output.status.success() {
            info!("Successfully killed tmux session: {}", tmux_session_name);
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Failed to kill tmux session '{}': {}", tmux_session_name, stderr);
            // Continue anyway - session might already be dead
        }

        // Step 2: Remove worktree
        info!("Attempting to remove worktree for session {}", session_id);
        match self.worktree_manager.remove_worktree(session_id) {
            Ok(()) => info!("Successfully removed worktree for session {}", session_id),
            Err(e) => {
                error!("Failed to remove worktree for session {}: {}", session_id, e);
                return Err(e.into());
            }
        }

        info!("<<< InteractiveSessionManager::remove_session() COMPLETE: {}", session_id);
        Ok(())
    }

    /// Check if a session is still alive (tmux session exists)
    ///
    /// # Arguments
    /// * `session_id` - UUID of the session to check
    ///
    /// # Returns
    /// * `Result<bool>` - True if session is alive, false otherwise
    pub async fn is_session_alive(&self, session_id: Uuid) -> Result<bool, InteractiveSessionError> {
        let session = self.active_sessions
            .get(&session_id)
            .ok_or(InteractiveSessionError::SessionNotFound(session_id))?;

        let output = Command::new("tmux")
            .args(["has-session", "-t", &session.tmux_session_name])
            .output()
            .await?;

        Ok(output.status.success())
    }

    /// Get a session by ID
    pub fn get_session(&self, session_id: Uuid) -> Option<&InteractiveSession> {
        self.active_sessions.get(&session_id)
    }

    /// Get all active sessions
    pub fn get_all_sessions(&self) -> Vec<&InteractiveSession> {
        self.active_sessions.values().collect()
    }

    // ===== Private Helper Methods =====

    /// Generate a tmux session name from a branch name
    ///
    /// Sanitizes the branch name to be tmux-compatible
    fn generate_tmux_name(branch_name: &str) -> String {
        let sanitized = branch_name
            .replace(' ', "_")
            .replace('.', "_")
            .replace('/', "_")
            .replace(':', "_");
        format!("tmux_{}", sanitized)
    }

    /// Start a new tmux session
    async fn start_tmux_session(&self, session_name: &str, work_dir: &Path) -> Result<(), InteractiveSessionError> {
        // Check if session already exists
        let check = Command::new("tmux")
            .args(["has-session", "-t", session_name])
            .output()
            .await?;

        if check.status.success() {
            warn!("Tmux session '{}' already exists, killing it first", session_name);
            Command::new("tmux")
                .args(["kill-session", "-t", session_name])
                .output()
                .await?;
        }

        // Create new detached tmux session
        let output = Command::new("tmux")
            .args([
                "new-session",
                "-d",              // Detached
                "-s", session_name,
                "-c", work_dir.to_str().context("Invalid work directory path")?,
                "-x", "120",       // Width
                "-y", "40",        // Height
            ])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(InteractiveSessionError::Tmux(
                format!("Failed to create tmux session '{}': {}", session_name, stderr)
            ));
        }

        // Configure tmux session
        self.configure_tmux_session(session_name).await?;

        info!("Started tmux session: {}", session_name);
        Ok(())
    }

    /// Configure tmux session settings
    async fn configure_tmux_session(&self, session_name: &str) -> Result<(), InteractiveSessionError> {
        // Set history limit
        Command::new("tmux")
            .args([
                "set-option", "-t", session_name,
                "history-limit", "50000"
            ])
            .status()
            .await?;

        // Enable mouse scrolling
        Command::new("tmux")
            .args([
                "set-option", "-t", session_name,
                "mouse", "on"
            ])
            .status()
            .await?;

        Ok(())
    }

    /// Start claude CLI in the tmux session
    async fn start_claude_in_tmux(&self, session_name: &str, skip_permissions: bool) -> Result<(), InteractiveSessionError> {
        // Build the claude command with appropriate flags
        let claude_cmd = if skip_permissions {
            "claude --dangerously-skip-permissions"
        } else {
            "claude"
        };

        info!("Starting claude with command: {}", claude_cmd);

        // Send command to tmux to start claude
        let output = Command::new("tmux")
            .args([
                "send-keys", "-t", session_name,
                claude_cmd, "C-m"  // C-m = Enter key
            ])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(InteractiveSessionError::Tmux(
                format!("Failed to start claude in tmux: {}", stderr)
            ));
        }

        info!("Started claude CLI in tmux session: {} (skip_permissions={})", session_name, skip_permissions);
        Ok(())
    }
}

/// Convert InteractiveSession to Session model for UI
impl InteractiveSession {
    pub fn to_session_model(&self) -> Session {
        let mut session = Session::new_with_options(
            self.workspace_name.clone(),
            self.worktree_path.to_string_lossy().to_string(),
            false, // skip_permissions
            SessionMode::Interactive,
            None, // boss_prompt
        );

        session.id = self.session_id;
        session.branch_name = self.branch_name.clone();
        session.tmux_session_name = Some(self.tmux_session_name.clone());
        session.container_id = None; // No Docker container
        session.status = SessionStatus::Running; // If tmux session exists, it's running
        session.created_at = self.created_at;

        session
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_tmux_name() {
        assert_eq!(
            InteractiveSessionManager::generate_tmux_name("feature/my-feature"),
            "tmux_feature_my-feature"
        );

        assert_eq!(
            InteractiveSessionManager::generate_tmux_name("fix.bug:test"),
            "tmux_fix_bug_test"
        );

        assert_eq!(
            InteractiveSessionManager::generate_tmux_name("simple"),
            "tmux_simple"
        );
    }

    #[test]
    fn test_session_manager_creation() {
        let manager = InteractiveSessionManager::new();
        assert!(manager.is_ok(), "Should create manager without Docker");
    }
}
