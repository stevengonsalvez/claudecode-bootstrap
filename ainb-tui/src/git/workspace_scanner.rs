// ABOUTME: Workspace detection and validation for git repositories

#![allow(dead_code)]

use anyhow::{Context, Result};
use git2::Repository;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

use crate::models::Workspace;

#[derive(Debug, Clone)]
pub struct ScanResult {
    pub workspaces: Vec<Workspace>,
    pub errors: Vec<String>,
}

pub struct WorkspaceScanner {
    search_paths: Vec<PathBuf>,
    max_depth: usize,
    ignore_patterns: Vec<String>,
}

impl WorkspaceScanner {
    pub fn new() -> Self {
        Self::with_additional_paths(vec![])
    }

    pub fn with_additional_paths(additional_paths: Vec<PathBuf>) -> Self {
        let mut search_paths = Self::default_search_paths();
        search_paths.extend(additional_paths);

        Self {
            search_paths,
            max_depth: 3,
            ignore_patterns: vec![
                "node_modules".to_string(),
                ".git".to_string(),
                "target".to_string(),
                "dist".to_string(),
                "build".to_string(),
            ],
        }
    }

    pub fn with_search_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.search_paths = paths;
        self
    }

    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    pub fn scan(&self) -> Result<ScanResult> {
        info!(
            "Starting workspace scan with {} search paths",
            self.search_paths.len()
        );

        // Log each search path for debugging
        for (i, path) in self.search_paths.iter().enumerate() {
            info!("Search path {}: {}", i + 1, path.display());
        }

        let mut workspaces = Vec::new();
        let mut errors = Vec::new();

        for search_path in &self.search_paths {
            info!("Scanning path: {}", search_path.display());
            match self.scan_directory(search_path, 0) {
                Ok(mut found_workspaces) => {
                    info!("Found {} workspaces in {}", found_workspaces.len(), search_path.display());
                    workspaces.append(&mut found_workspaces);
                }
                Err(e) => {
                    let error_msg = format!("Error scanning {}: {}", search_path.display(), e);
                    warn!("{}", error_msg);
                    errors.push(error_msg);
                }
            }
        }

        // Sort workspaces by name for consistent ordering
        workspaces.sort_by(|a, b| a.name.cmp(&b.name));

        info!(
            "Workspace scan complete: found {} workspaces, {} errors",
            workspaces.len(),
            errors.len()
        );

        Ok(ScanResult { workspaces, errors })
    }

    pub fn validate_workspace(path: &Path) -> Result<bool> {
        if !path.exists() {
            return Ok(false);
        }

        if !path.is_dir() {
            return Ok(false);
        }

        // Check if it's a git repository
        match Repository::open(path) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    pub fn create_workspace_from_path(path: &Path) -> Result<Workspace> {
        let repo = Repository::open(path)
            .with_context(|| format!("Failed to open git repository at {}", path.display()))?;

        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_string();

        // Validate the repository state
        Self::validate_repository(&repo)?;

        Ok(Workspace::new(name, path.to_path_buf()))
    }

    fn scan_directory(&self, path: &Path, current_depth: usize) -> Result<Vec<Workspace>> {
        if current_depth > self.max_depth {
            return Ok(Vec::new());
        }

        if !path.exists() || !path.is_dir() {
            return Ok(Vec::new());
        }

        let mut workspaces = Vec::new();

        // Check if current directory is a git repository
        if Self::validate_workspace(path)? {
            debug!("Found git repository at: {}", path.display());
            match Self::create_workspace_from_path(path) {
                Ok(workspace) => workspaces.push(workspace),
                Err(e) => {
                    warn!("Failed to create workspace from {}: {}", path.display(), e);
                }
            }
            // Don't recurse into git repositories
            return Ok(workspaces);
        }

        // Recursively scan subdirectories
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();

                if !entry_path.is_dir() {
                    continue;
                }

                // Skip ignored directories
                if let Some(dir_name) = entry_path.file_name().and_then(|n| n.to_str()) {
                    if self.ignore_patterns.iter().any(|pattern| dir_name.contains(pattern)) {
                        continue;
                    }
                }

                match self.scan_directory(&entry_path, current_depth + 1) {
                    Ok(mut sub_workspaces) => {
                        workspaces.append(&mut sub_workspaces);
                    }
                    Err(e) => {
                        debug!("Error scanning {}: {}", entry_path.display(), e);
                    }
                }
            }
        }

        Ok(workspaces)
    }

    fn validate_repository(repo: &Repository) -> Result<()> {
        // Check if repository is not bare
        if repo.is_bare() {
            return Err(anyhow::anyhow!("Repository is bare"));
        }

        // Check if we can access the HEAD
        match repo.head() {
            Ok(_) => Ok(()),
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("reference 'refs/heads/master' not found")
                    || error_msg.contains("reference 'refs/heads/main' not found")
                    || error_msg.contains("unborn branch or empty repository")
                {
                    // Empty repository is okay
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("Cannot access repository HEAD: {}", e))
                }
            }
        }
    }

    fn default_search_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        // Add common development directories
        if let Some(home_dir) = dirs::home_dir() {
            // Common development directories
            for subdir in &["projects", "code", "dev", "workspace", "src", "repos"] {
                let path = home_dir.join(subdir);
                if path.exists() {
                    paths.push(path);
                }
            }

            // Desktop and Documents for casual projects
            for subdir in &["Desktop", "Documents"] {
                let path = home_dir.join(subdir);
                if path.exists() {
                    paths.push(path);
                }
            }
        }

        // Add current directory if it's a git repository
        if let Ok(current_dir) = std::env::current_dir() {
            if Self::validate_workspace(&current_dir).unwrap_or(false) {
                paths.push(current_dir);
            }
        }

        // If no paths found, default to home directory
        if paths.is_empty() {
            if let Some(home_dir) = dirs::home_dir() {
                paths.push(home_dir);
            }
        }

        paths
    }
}

impl Default for WorkspaceScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_git_repo(path: &Path) -> Result<()> {
        Repository::init(path)?;
        Ok(())
    }

    #[test]
    fn test_validate_workspace_with_git_repo() {
        let temp_dir = TempDir::new().unwrap();
        create_test_git_repo(temp_dir.path()).unwrap();

        assert!(WorkspaceScanner::validate_workspace(temp_dir.path()).unwrap());
    }

    #[test]
    fn test_validate_workspace_without_git_repo() {
        let temp_dir = TempDir::new().unwrap();

        assert!(!WorkspaceScanner::validate_workspace(temp_dir.path()).unwrap());
    }

    #[test]
    fn test_validate_workspace_nonexistent_path() {
        let nonexistent = PathBuf::from("/nonexistent/path");

        assert!(!WorkspaceScanner::validate_workspace(&nonexistent).unwrap());
    }

    #[test]
    fn test_create_workspace_from_path() {
        let temp_dir = TempDir::new().unwrap();
        create_test_git_repo(temp_dir.path()).unwrap();

        let workspace = WorkspaceScanner::create_workspace_from_path(temp_dir.path()).unwrap();
        assert_eq!(workspace.path, temp_dir.path());
        assert!(!workspace.name.is_empty());
    }

    #[test]
    fn test_scan_directory_with_git_repo() {
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("test-repo");
        fs::create_dir(&repo_dir).unwrap();
        create_test_git_repo(&repo_dir).unwrap();

        let scanner = WorkspaceScanner::new();
        let workspaces = scanner.scan_directory(temp_dir.path(), 0).unwrap();

        assert_eq!(workspaces.len(), 1);
        assert_eq!(workspaces[0].name, "test-repo");
    }

    #[test]
    fn test_scan_ignores_patterns() {
        let temp_dir = TempDir::new().unwrap();

        // Create separate directories that should be ignored
        // Note: Don't create a git repo in ".git" subdirectory as it confuses the parent detection
        for ignored in &["node_modules", "target", "dist", "build"] {
            let ignored_dir = temp_dir.path().join(ignored);
            fs::create_dir(&ignored_dir).unwrap();
            create_test_git_repo(&ignored_dir).unwrap();
        }

        // Create a directory that should not be ignored
        let valid_dir = temp_dir.path().join("valid-repo");
        fs::create_dir(&valid_dir).unwrap();
        create_test_git_repo(&valid_dir).unwrap();

        // Create another valid directory
        let another_valid_dir = temp_dir.path().join("my-project");
        fs::create_dir(&another_valid_dir).unwrap();
        create_test_git_repo(&another_valid_dir).unwrap();

        let scanner = WorkspaceScanner::new();
        let workspaces = scanner.scan_directory(temp_dir.path(), 0).unwrap();

        // Debug: print what we found
        println!(
            "Found workspaces: {:?}",
            workspaces.iter().map(|w| &w.name).collect::<Vec<_>>()
        );

        // Should find the valid repositories but not the ignored ones
        assert!(workspaces.len() >= 2, "Should find at least 2 workspaces");

        let valid_workspace = workspaces.iter().find(|w| w.name == "valid-repo");
        assert!(
            valid_workspace.is_some(),
            "Should find valid-repo workspace"
        );

        let project_workspace = workspaces.iter().find(|w| w.name == "my-project");
        assert!(
            project_workspace.is_some(),
            "Should find my-project workspace"
        );

        // Check that ignored directories are not included
        for ignored in &["node_modules", "target", "dist", "build"] {
            let ignored_workspace = workspaces.iter().find(|w| w.name == *ignored);
            assert!(
                ignored_workspace.is_none(),
                "Should not find {} workspace",
                ignored
            );
        }
    }
}
