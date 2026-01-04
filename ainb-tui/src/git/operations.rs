// ABOUTME: Shared git operations for commit and push functionality - ensures DRY compliance

use anyhow::Result;
use std::path::Path;
use std::process::Command;
use tracing::{debug, error};

/// Core git commit and push operation that can be used by both git view and quick commit
pub fn commit_and_push_changes(worktree_path: &Path, commit_message: &str) -> Result<String> {
    debug!(
        "Committing and pushing changes for worktree: {:?}",
        worktree_path
    );

    if commit_message.trim().is_empty() {
        return Err(anyhow::anyhow!("Commit message cannot be empty"));
    }

    // Try CLI git first as it's more reliable
    debug!("=== Using CLI git for commit and push ===");
    match commit_and_push_cli(worktree_path, commit_message) {
        Ok(result) => {
            debug!("✓ CLI git succeeded!");
            return Ok(result);
        }
        Err(e) => {
            debug!("✗ CLI git failed: {}, falling back to git2", e);
        }
    }

    // Fallback to git2 implementation
    debug!("=== Falling back to git2 implementation ===");
    commit_and_push_git2(worktree_path, commit_message)
}

fn commit_and_push_cli(worktree_path: &Path, commit_message: &str) -> Result<String> {
    debug!("Using CLI git for commit and push");

    // Store original directory
    let original_dir = std::env::current_dir()?;

    let result = (|| -> Result<String> {
        // Change to worktree directory
        std::env::set_current_dir(worktree_path)?;

        // Add all changes
        debug!("Adding all changes...");
        let add_output = Command::new("git")
            .args(&["add", "."])
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()?;

        if !add_output.status.success() {
            let stderr = String::from_utf8_lossy(&add_output.stderr);
            return Err(anyhow::anyhow!("git add failed: {}", stderr));
        }

        // Commit with --no-gpg-sign to avoid hanging on GPG passphrase
        debug!("Committing with message: {}", commit_message);
        let commit_output = Command::new("git")
            .args(&["commit", "--no-gpg-sign", "-m", commit_message])
            .env("GIT_TERMINAL_PROMPT", "0") // Disable interactive prompts
            .env("GIT_ASKPASS", "echo") // Provide dummy askpass to avoid hanging
            .output()?;

        if !commit_output.status.success() {
            let stderr = String::from_utf8_lossy(&commit_output.stderr);
            return Err(anyhow::anyhow!("git commit failed: {}", stderr));
        }

        // Push changes
        debug!("Pushing changes...");
        let push_output = Command::new("git")
            .args(&["push"])
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_ASKPASS", "echo")
            .output()?;

        if !push_output.status.success() {
            let stderr = String::from_utf8_lossy(&push_output.stderr);
            let stdout = String::from_utf8_lossy(&push_output.stdout);
            error!("git push failed - stderr: {}", stderr);
            error!("git push failed - stdout: {}", stdout);
            return Err(anyhow::anyhow!("git push failed: {}", stderr));
        }

        debug!("CLI git push succeeded");
        Ok(format!("Committed and pushed: {}", commit_message))
    })();

    // Always restore original directory
    std::env::set_current_dir(original_dir)?;

    result
}

fn commit_and_push_git2(worktree_path: &Path, commit_message: &str) -> Result<String> {
    use git2::{Repository, Signature};

    let repo = Repository::open(worktree_path)?;

    // Stage all changes
    let mut index = repo.index()?;

    // Add all files in the working directory
    index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
    index.write()?;

    // Create commit
    let signature = Signature::now("Claude Box", "claude-box@local")?;
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;

    // Get parent commit
    let parent_commit = match repo.head() {
        Ok(head) => {
            let oid = head.target().unwrap();
            Some(repo.find_commit(oid)?)
        }
        Err(_) => None, // Initial commit
    };

    let parents: Vec<&git2::Commit> = match &parent_commit {
        Some(commit) => vec![commit],
        None => vec![],
    };

    let commit_id = repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        commit_message,
        &tree,
        &parents,
    )?;

    debug!("Created commit: {}", commit_id);

    // Push to remote
    let mut remote = repo.find_remote("origin")?;
    let head_ref = repo.head()?;
    let branch = head_ref.shorthand().unwrap_or("main");
    let refspec = format!("refs/heads/{}:refs/heads/{}", branch, branch);

    let mut callbacks = git2::RemoteCallbacks::new();
    callbacks.credentials(|_url, username_from_url, _allowed_types| {
        git2::Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
    });

    let mut push_options = git2::PushOptions::new();
    push_options.remote_callbacks(callbacks);

    match remote.push(&[&refspec], Some(&mut push_options)) {
        Ok(_) => {
            debug!("git2 push succeeded");
            Ok(format!("Committed and pushed: {}", commit_message))
        }
        Err(e) => {
            let user_friendly_msg = match e.code() {
                git2::ErrorCode::Auth => {
                    "Authentication failed. Please check your SSH keys or credentials."
                }
                _ => "Push failed. Please check your remote repository configuration.",
            };

            error!("git2 push failed: {}", e);
            Err(anyhow::anyhow!("Push failed: {}", user_friendly_msg))
        }
    }
}
