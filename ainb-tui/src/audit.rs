// ABOUTME: Audit logging for user-initiated mutations
//
// Provides a persistent audit trail for all destructive or state-changing
// operations initiated by the user. This helps diagnose issues like
// unexpectedly deleted worktrees or changed configurations.
//
// Audit log is written to: ~/.agents-in-a-box/logs/audit.jsonl
// Format: JSON Lines (one JSON object per line) for easy grep/parsing

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use tracing::{error, info};
use uuid::Uuid;

/// Types of auditable actions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    // Session operations
    SessionCreated,
    SessionDeleted,
    SessionAttached,
    SessionDetached,

    // Worktree operations
    WorktreeCreated,
    WorktreeRemoved,
    WorktreePruned,

    // Config operations
    ConfigSaved,
    ConfigReset,

    // Cleanup operations
    OrphanedContainersCleanup,
    BrokenSymlinksCleanup,

    // Git operations
    GitWorktreePrune,
    GitBranchCreated,
    GitBranchDeleted,

    // Other
    FactoryReset,
    Custom(String),
}

impl std::fmt::Display for AuditAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditAction::SessionCreated => write!(f, "SESSION_CREATED"),
            AuditAction::SessionDeleted => write!(f, "SESSION_DELETED"),
            AuditAction::SessionAttached => write!(f, "SESSION_ATTACHED"),
            AuditAction::SessionDetached => write!(f, "SESSION_DETACHED"),
            AuditAction::WorktreeCreated => write!(f, "WORKTREE_CREATED"),
            AuditAction::WorktreeRemoved => write!(f, "WORKTREE_REMOVED"),
            AuditAction::WorktreePruned => write!(f, "WORKTREE_PRUNED"),
            AuditAction::ConfigSaved => write!(f, "CONFIG_SAVED"),
            AuditAction::ConfigReset => write!(f, "CONFIG_RESET"),
            AuditAction::OrphanedContainersCleanup => write!(f, "ORPHANED_CONTAINERS_CLEANUP"),
            AuditAction::BrokenSymlinksCleanup => write!(f, "BROKEN_SYMLINKS_CLEANUP"),
            AuditAction::GitWorktreePrune => write!(f, "GIT_WORKTREE_PRUNE"),
            AuditAction::GitBranchCreated => write!(f, "GIT_BRANCH_CREATED"),
            AuditAction::GitBranchDeleted => write!(f, "GIT_BRANCH_DELETED"),
            AuditAction::FactoryReset => write!(f, "FACTORY_RESET"),
            AuditAction::Custom(s) => write!(f, "CUSTOM:{}", s),
        }
    }
}

/// Result of an audited action
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditResult {
    Success,
    Failed(String),
    Partial(String),
}

/// A single audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// When the action occurred
    pub timestamp: DateTime<Utc>,

    /// Type of action
    pub action: AuditAction,

    /// Result of the action
    pub result: AuditResult,

    /// Session ID if applicable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<Uuid>,

    /// Tmux session name if applicable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tmux_session: Option<String>,

    /// Path involved (worktree, config file, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    /// Additional context/details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,

    /// Trigger source (user action, automatic, etc.)
    pub trigger: AuditTrigger,
}

/// What triggered the audit action
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditTrigger {
    /// User pressed a key (e.g., 'D' for delete)
    UserKeypress(String),
    /// User clicked a UI element
    UserClick,
    /// Automatic cleanup/maintenance
    Automatic,
    /// Startup initialization
    Startup,
    /// Shutdown cleanup
    Shutdown,
    /// API/CLI command
    Command(String),
}

impl std::fmt::Display for AuditTrigger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditTrigger::UserKeypress(key) => write!(f, "keypress:{}", key),
            AuditTrigger::UserClick => write!(f, "click"),
            AuditTrigger::Automatic => write!(f, "automatic"),
            AuditTrigger::Startup => write!(f, "startup"),
            AuditTrigger::Shutdown => write!(f, "shutdown"),
            AuditTrigger::Command(cmd) => write!(f, "command:{}", cmd),
        }
    }
}

/// Global audit logger
static AUDIT_LOGGER: Mutex<Option<AuditLogger>> = Mutex::new(None);

/// Audit logger that writes to a JSONL file
pub struct AuditLogger {
    writer: BufWriter<File>,
    log_path: PathBuf,
}

impl AuditLogger {
    /// Initialize the global audit logger
    pub fn init() -> std::io::Result<()> {
        let log_path = Self::log_path();

        // Create parent directory
        if let Some(parent) = log_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Open file in append mode
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;

        let logger = AuditLogger {
            writer: BufWriter::new(file),
            log_path: log_path.clone(),
        };

        let mut global = AUDIT_LOGGER.lock().unwrap();
        *global = Some(logger);

        info!("Audit logging initialized: {:?}", log_path);
        Ok(())
    }

    /// Get the audit log file path
    pub fn log_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".agents-in-a-box")
            .join("logs")
            .join("audit.jsonl")
    }

    /// Write an entry to the audit log
    fn write_entry(&mut self, entry: &AuditEntry) -> std::io::Result<()> {
        let json = serde_json::to_string(entry)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        writeln!(self.writer, "{}", json)?;
        self.writer.flush()?;
        Ok(())
    }
}

/// Log an audit entry (main API)
pub fn audit_log(entry: AuditEntry) {
    // Also log to tracing for immediate visibility
    info!(
        target: "audit",
        action = %entry.action,
        result = ?entry.result,
        trigger = %entry.trigger,
        session_id = ?entry.session_id,
        path = ?entry.path,
        "AUDIT: {}",
        entry.action
    );

    // Write to audit file
    let mut global = match AUDIT_LOGGER.lock() {
        Ok(g) => g,
        Err(e) => {
            error!("Failed to acquire audit logger lock: {}", e);
            return;
        }
    };

    if let Some(ref mut logger) = *global {
        if let Err(e) = logger.write_entry(&entry) {
            error!("Failed to write audit entry: {}", e);
        }
    } else {
        // Logger not initialized, try to init it
        drop(global); // Release lock before re-acquiring
        if let Err(e) = AuditLogger::init() {
            error!("Failed to initialize audit logger: {}", e);
            return;
        }

        // Retry with initialized logger
        if let Ok(mut global) = AUDIT_LOGGER.lock() {
            if let Some(ref mut logger) = *global {
                if let Err(e) = logger.write_entry(&entry) {
                    error!("Failed to write audit entry after init: {}", e);
                }
            }
        }
    }
}

// ============================================================================
// Convenience functions for common audit scenarios
// ============================================================================

/// Log a session deletion
pub fn audit_session_deleted(
    session_id: Uuid,
    tmux_session: Option<String>,
    worktree_path: Option<String>,
    trigger: AuditTrigger,
    result: AuditResult,
) {
    audit_log(AuditEntry {
        timestamp: Utc::now(),
        action: AuditAction::SessionDeleted,
        result,
        session_id: Some(session_id),
        tmux_session,
        path: worktree_path,
        details: None,
        trigger,
    });
}

/// Log a worktree removal
pub fn audit_worktree_removed(
    session_id: Option<Uuid>,
    worktree_path: &str,
    trigger: AuditTrigger,
    result: AuditResult,
) {
    audit_log(AuditEntry {
        timestamp: Utc::now(),
        action: AuditAction::WorktreeRemoved,
        result,
        session_id,
        tmux_session: None,
        path: Some(worktree_path.to_string()),
        details: None,
        trigger,
    });
}

/// Log a worktree creation
pub fn audit_worktree_created(
    session_id: Uuid,
    worktree_path: &str,
    branch_name: &str,
    trigger: AuditTrigger,
    result: AuditResult,
) {
    audit_log(AuditEntry {
        timestamp: Utc::now(),
        action: AuditAction::WorktreeCreated,
        result,
        session_id: Some(session_id),
        tmux_session: None,
        path: Some(worktree_path.to_string()),
        details: Some(format!("branch: {}", branch_name)),
        trigger,
    });
}

/// Log a git worktree prune operation
pub fn audit_git_worktree_prune(
    trigger: AuditTrigger,
    result: AuditResult,
    pruned_count: Option<usize>,
) {
    audit_log(AuditEntry {
        timestamp: Utc::now(),
        action: AuditAction::GitWorktreePrune,
        result,
        session_id: None,
        tmux_session: None,
        path: None,
        details: pruned_count.map(|c| format!("pruned {} worktrees", c)),
        trigger,
    });
}

/// Log a config save
pub fn audit_config_saved(
    config_path: &str,
    trigger: AuditTrigger,
    result: AuditResult,
    details: Option<String>,
) {
    audit_log(AuditEntry {
        timestamp: Utc::now(),
        action: AuditAction::ConfigSaved,
        result,
        session_id: None,
        tmux_session: None,
        path: Some(config_path.to_string()),
        details,
        trigger,
    });
}

/// Log orphaned containers cleanup
pub fn audit_orphaned_cleanup(
    trigger: AuditTrigger,
    result: AuditResult,
    details: String,
) {
    audit_log(AuditEntry {
        timestamp: Utc::now(),
        action: AuditAction::OrphanedContainersCleanup,
        result,
        session_id: None,
        tmux_session: None,
        path: None,
        details: Some(details),
        trigger,
    });
}

/// Log a session creation
pub fn audit_session_created(
    session_id: Uuid,
    tmux_session: &str,
    worktree_path: &str,
    branch_name: &str,
    trigger: AuditTrigger,
    result: AuditResult,
) {
    audit_log(AuditEntry {
        timestamp: Utc::now(),
        action: AuditAction::SessionCreated,
        result,
        session_id: Some(session_id),
        tmux_session: Some(tmux_session.to_string()),
        path: Some(worktree_path.to_string()),
        details: Some(format!("branch: {}", branch_name)),
        trigger,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_entry_serialization() {
        let entry = AuditEntry {
            timestamp: Utc::now(),
            action: AuditAction::SessionDeleted,
            result: AuditResult::Success,
            session_id: Some(Uuid::new_v4()),
            tmux_session: Some("tmux_test_branch".to_string()),
            path: Some("/path/to/worktree".to_string()),
            details: None,
            trigger: AuditTrigger::UserKeypress("D".to_string()),
        };

        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("session_deleted"));
        assert!(json.contains("tmux_test_branch"));
    }

    #[test]
    fn test_action_display() {
        assert_eq!(format!("{}", AuditAction::SessionDeleted), "SESSION_DELETED");
        assert_eq!(format!("{}", AuditAction::WorktreeRemoved), "WORKTREE_REMOVED");
    }
}
