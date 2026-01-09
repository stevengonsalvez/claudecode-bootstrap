// ABOUTME: Manages cloning and caching of remote git repositories

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;
use tracing::{debug, info, warn};

use super::repo_source::{ParsedRepo, RepoSource};

#[derive(Error, Debug, Clone)]
pub enum RemoteRepoError {
    #[error("Clone failed: {0}")]
    CloneFailed(String),
    #[error("Authentication failed - check your git credentials")]
    AuthFailed,
    #[error("Repository not found: {0}")]
    NotFound(String),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Invalid repository: {0}")]
    InvalidRepo(String),
    #[error("IO error: {0}")]
    IoError(String),
}

impl From<std::io::Error> for RemoteRepoError {
    fn from(err: std::io::Error) -> Self {
        RemoteRepoError::IoError(err.to_string())
    }
}

/// Information about a remote branch
#[derive(Debug, Clone)]
pub struct RemoteBranch {
    pub name: String,
    pub commit_hash: String,
    pub is_default: bool,
}

/// Manages remote repository cloning and caching
pub struct RemoteRepoManager {
    cache_dir: PathBuf,
}

impl RemoteRepoManager {
    /// Create a new RemoteRepoManager with default cache directory
    pub fn new() -> Result<Self> {
        let home_dir = dirs::home_dir().context("Failed to get home directory")?;
        let cache_dir = home_dir.join(".agents-in-a-box").join("repos");

        std::fs::create_dir_all(&cache_dir)?;

        Ok(Self { cache_dir })
    }

    /// Create with a custom cache directory (for testing)
    pub fn with_cache_dir(cache_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&cache_dir)?;
        Ok(Self { cache_dir })
    }

    /// Get the cache directory path
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Get the cache path for a parsed repo
    pub fn get_cache_path(&self, parsed: &ParsedRepo) -> PathBuf {
        self.cache_dir
            .join(&parsed.host)
            .join(&parsed.owner)
            .join(format!("{}.git", &parsed.repo_name))
    }

    /// Check if a repo is already cached
    pub fn is_cached(&self, parsed: &ParsedRepo) -> bool {
        let cache_path = self.get_cache_path(parsed);
        cache_path.exists() && cache_path.join("HEAD").exists()
    }

    /// List remote branches without cloning (uses git ls-remote)
    pub fn list_remote_branches(
        &self,
        source: &RepoSource,
    ) -> Result<Vec<RemoteBranch>, RemoteRepoError> {
        let url = source.to_clone_url();
        info!("Listing remote branches for: {}", url);

        let output = Command::new("git")
            .args(["ls-remote", "--heads", "--refs", &url])
            .output()
            .map_err(|e| RemoteRepoError::NetworkError(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(classify_git_error(&stderr, &url));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut branches: Vec<RemoteBranch> = stdout
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let commit_hash = parts[0].to_string();
                    let ref_name = parts[1];
                    // refs/heads/branch-name -> branch-name
                    let name = ref_name.strip_prefix("refs/heads/")?.to_string();
                    Some(RemoteBranch {
                        name,
                        commit_hash,
                        is_default: false,
                    })
                } else {
                    None
                }
            })
            .collect();

        // Try to determine default branch
        let default_branch = self.get_default_branch_name(source);

        // Mark default branch
        for branch in &mut branches {
            if Some(&branch.name) == default_branch.as_ref() {
                branch.is_default = true;
            }
        }

        // If no default found, mark main or master
        if !branches.iter().any(|b| b.is_default) {
            for branch in &mut branches {
                if branch.name == "main" || branch.name == "master" {
                    branch.is_default = true;
                    break;
                }
            }
        }

        // Sort: default first, then alphabetical
        branches.sort_by(|a, b| match (a.is_default, b.is_default) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        });

        debug!("Found {} branches", branches.len());
        Ok(branches)
    }

    /// Try to get the default branch name from remote
    fn get_default_branch_name(&self, source: &RepoSource) -> Option<String> {
        let url = source.to_clone_url();

        let output = Command::new("git")
            .args(["ls-remote", "--symref", &url, "HEAD"])
            .output()
            .ok()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Parse: ref: refs/heads/main\tHEAD
            for line in stdout.lines() {
                if line.starts_with("ref:") && line.contains("HEAD") {
                    if let Some(branch) = line
                        .split_whitespace()
                        .next()
                        .and_then(|s| s.strip_prefix("ref:"))
                        .and_then(|s| s.strip_prefix("refs/heads/"))
                    {
                        return Some(branch.to_string());
                    }
                }
            }
        }

        None
    }

    /// Clone a remote repository as a bare clone
    pub fn clone_bare(
        &self,
        source: &RepoSource,
        parsed: &ParsedRepo,
    ) -> Result<PathBuf, RemoteRepoError> {
        let url = source.to_clone_url();
        let cache_path = self.get_cache_path(parsed);

        if self.is_cached(parsed) {
            info!("Repository already cached at: {}", cache_path.display());
            // Fetch updates
            if let Err(e) = self.fetch_updates(&cache_path) {
                warn!("Failed to fetch updates: {}", e);
                // Continue with cached version
            }
            return Ok(cache_path);
        }

        info!("Cloning {} to {}", url, cache_path.display());

        // Create parent directories
        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let output = Command::new("git")
            .args(["clone", "--bare", &url])
            .arg(&cache_path)
            .output()
            .map_err(|e| RemoteRepoError::CloneFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(classify_git_error(&stderr, &url));
        }

        info!("Successfully cloned to: {}", cache_path.display());
        Ok(cache_path)
    }

    /// Fetch updates for a cached bare repo
    pub fn fetch_updates(&self, cache_path: &Path) -> Result<(), RemoteRepoError> {
        info!("Fetching updates for: {}", cache_path.display());

        let output = Command::new("git")
            .args(["fetch", "--all", "--prune"])
            .current_dir(cache_path)
            .output()
            .map_err(|e| RemoteRepoError::NetworkError(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Fetch failed: {}", stderr);
            // Non-fatal - we can continue with cached version
        }

        Ok(())
    }

    /// Create a worktree from a bare cached repo
    pub fn create_worktree_from_cache(
        &self,
        cache_path: &Path,
        worktree_path: &Path,
        branch_name: &str,
        base_branch: &str,
    ) -> Result<(), RemoteRepoError> {
        info!(
            "Creating worktree at {} for branch {} (base: {})",
            worktree_path.display(),
            branch_name,
            base_branch
        );

        // Create parent directory for worktree
        if let Some(parent) = worktree_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Check if the new branch already exists
        let branch_exists = Command::new("git")
            .args(["rev-parse", "--verify", branch_name])
            .current_dir(cache_path)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !branch_exists {
            // In a bare clone, refs are stored directly in refs/heads/{branch}
            // NOT in refs/remotes/origin/{branch} like a regular clone
            // So we use the branch name directly, not origin/{branch}
            let base_ref = base_branch;

            // First verify the base branch ref exists in the bare repo
            let ref_check = Command::new("git")
                .args(["rev-parse", "--verify", &format!("refs/heads/{}", base_ref)])
                .current_dir(cache_path)
                .output()?;

            if !ref_check.status.success() {
                // Get list of available branches for better error message
                let branches_output = Command::new("git")
                    .args(["branch", "--list"])
                    .current_dir(cache_path)
                    .output()
                    .ok();
                let available = branches_output
                    .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                    .unwrap_or_default();
                let branch_list: Vec<&str> = available.lines()
                    .map(|s| s.trim().trim_start_matches("* "))
                    .filter(|s| !s.is_empty())
                    .collect();

                return Err(RemoteRepoError::InvalidRepo(format!(
                    "Base branch '{}' not found. Available branches: {}",
                    base_branch,
                    if branch_list.is_empty() { "(none)".to_string() } else { branch_list.join(", ") }
                )));
            }

            // Create new branch from the base branch
            let output = Command::new("git")
                .args(["branch", branch_name, base_ref])
                .current_dir(cache_path)
                .output()?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                // Branch might already exist, which is okay
                if !stderr.contains("already exists") {
                    return Err(RemoteRepoError::InvalidRepo(format!(
                        "Failed to create branch '{}': {}",
                        branch_name, stderr
                    )));
                }
            }
        }

        // Create the worktree
        let output = Command::new("git")
            .args([
                "worktree",
                "add",
                worktree_path.to_str().unwrap(),
                branch_name,
            ])
            .current_dir(cache_path)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RemoteRepoError::CloneFailed(format!(
                "Failed to create worktree: {}",
                stderr
            )));
        }

        info!(
            "Successfully created worktree at: {}",
            worktree_path.display()
        );
        Ok(())
    }

    /// Get list of cached repositories for recent repos feature
    pub fn list_cached_repos(&self) -> Result<Vec<ParsedRepo>> {
        let mut repos = Vec::new();

        if !self.cache_dir.exists() {
            return Ok(repos);
        }

        // Walk the cache directory structure: host/owner/repo.git
        if let Ok(hosts) = std::fs::read_dir(&self.cache_dir) {
            for host_entry in hosts.flatten() {
                if !host_entry.path().is_dir() {
                    continue;
                }
                let host = host_entry.file_name().to_string_lossy().to_string();

                if let Ok(owners) = std::fs::read_dir(host_entry.path()) {
                    for owner_entry in owners.flatten() {
                        if !owner_entry.path().is_dir() {
                            continue;
                        }
                        let owner = owner_entry.file_name().to_string_lossy().to_string();

                        if let Ok(repo_dirs) = std::fs::read_dir(owner_entry.path()) {
                            for repo_entry in repo_dirs.flatten() {
                                let filename = repo_entry.file_name();
                                let filename_str = filename.to_string_lossy();

                                // Only include .git directories (bare clones)
                                if !filename_str.ends_with(".git") {
                                    continue;
                                }

                                let repo_name = filename_str.trim_end_matches(".git").to_string();

                                let url = format!("https://{}/{}/{}", host, owner, repo_name);
                                repos.push(ParsedRepo {
                                    source: RepoSource::HttpsUrl(url),
                                    host: host.clone(),
                                    owner: owner.clone(),
                                    repo_name,
                                });
                            }
                        }
                    }
                }
            }
        }

        // Sort by repo name for consistent ordering
        repos.sort_by(|a, b| {
            let a_key = format!("{}/{}", a.owner, a.repo_name);
            let b_key = format!("{}/{}", b.owner, b.repo_name);
            a_key.cmp(&b_key)
        });

        Ok(repos)
    }

    /// Remove a cached repository
    pub fn remove_cached_repo(&self, parsed: &ParsedRepo) -> Result<(), RemoteRepoError> {
        let cache_path = self.get_cache_path(parsed);

        if cache_path.exists() {
            std::fs::remove_dir_all(&cache_path)?;
            info!("Removed cached repo: {}", cache_path.display());
        }

        Ok(())
    }
}

impl Default for RemoteRepoManager {
    fn default() -> Self {
        Self::new().expect("Failed to create RemoteRepoManager")
    }
}

/// Classify git errors into appropriate RemoteRepoError variants
fn classify_git_error(stderr: &str, url: &str) -> RemoteRepoError {
    let stderr_lower = stderr.to_lowercase();

    if stderr_lower.contains("authentication failed")
        || stderr_lower.contains("permission denied")
        || stderr_lower.contains("could not read username")
        || stderr_lower.contains("invalid credentials")
        || stderr_lower.contains("fatal: could not read password")
    {
        RemoteRepoError::AuthFailed
    } else if stderr_lower.contains("not found")
        || stderr_lower.contains("does not exist")
        || stderr_lower.contains("repository not found")
        || stderr_lower.contains("fatal: repository")
    {
        RemoteRepoError::NotFound(url.to_string())
    } else if stderr_lower.contains("could not resolve host")
        || stderr_lower.contains("network")
        || stderr_lower.contains("connection")
        || stderr_lower.contains("timeout")
    {
        RemoteRepoError::NetworkError(stderr.to_string())
    } else {
        RemoteRepoError::CloneFailed(stderr.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_cache_path_generation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = RemoteRepoManager::with_cache_dir(temp_dir.path().to_path_buf()).unwrap();

        let source = RepoSource::from_input("https://github.com/user/repo").unwrap();
        let parsed = source.parse_components().unwrap();

        let cache_path = manager.get_cache_path(&parsed);
        assert!(cache_path.to_string_lossy().contains("github.com"));
        assert!(cache_path.to_string_lossy().contains("user"));
        assert!(cache_path.to_string_lossy().ends_with("repo.git"));
    }

    #[test]
    fn test_is_cached_false_for_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let manager = RemoteRepoManager::with_cache_dir(temp_dir.path().to_path_buf()).unwrap();

        let source = RepoSource::from_input("https://github.com/nonexistent/repo").unwrap();
        let parsed = source.parse_components().unwrap();

        assert!(!manager.is_cached(&parsed));
    }

    #[test]
    fn test_list_cached_repos_empty() {
        let temp_dir = TempDir::new().unwrap();
        let manager = RemoteRepoManager::with_cache_dir(temp_dir.path().to_path_buf()).unwrap();

        let repos = manager.list_cached_repos().unwrap();
        assert!(repos.is_empty());
    }

    #[test]
    fn test_error_classification_auth() {
        let err = classify_git_error("fatal: Authentication failed for 'https://github.com/private/repo'", "url");
        assert!(matches!(err, RemoteRepoError::AuthFailed));
    }

    #[test]
    fn test_error_classification_not_found() {
        let err = classify_git_error("fatal: repository 'https://github.com/user/nonexistent' not found", "url");
        assert!(matches!(err, RemoteRepoError::NotFound(_)));
    }

    #[test]
    fn test_error_classification_network() {
        let err = classify_git_error("fatal: Could not resolve host: github.com", "url");
        assert!(matches!(err, RemoteRepoError::NetworkError(_)));
    }
}
