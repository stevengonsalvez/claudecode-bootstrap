// ABOUTME: Git integration module for workspace detection, worktree management, and git operations

pub mod diff_analyzer;
pub mod operations;
pub mod remote_repo_manager;
pub mod repo_source;
pub mod repository;
pub mod workspace_scanner;
pub mod worktree_manager;

pub use remote_repo_manager::{RemoteBranch, RemoteRepoError, RemoteRepoManager};
pub use repo_source::{ParsedRepo, RepoSource, RepoSourceError};
pub use workspace_scanner::WorkspaceScanner;
pub use worktree_manager::{WorktreeError, WorktreeInfo, WorktreeManager};
