// ABOUTME: Git integration module for workspace detection, worktree management, and git operations

pub mod diff_analyzer;
pub mod operations;
pub mod repository;
pub mod workspace_scanner;
pub mod worktree_manager;

pub use workspace_scanner::WorkspaceScanner;
pub use worktree_manager::{WorktreeError, WorktreeInfo, WorktreeManager};
