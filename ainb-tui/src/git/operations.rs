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
            .output()?;

        if !add_output.status.success() {
            let stderr = String::from_utf8_lossy(&add_output.stderr);
            return Err(anyhow::anyhow!("git add failed: {}", stderr));
        }

        // Commit with --no-gpg-sign to avoid hanging on GPG passphrase
        debug!("Committing with message: {}", commit_message);
        let commit_output = Command::new("git")
            .args(&["commit", "--no-gpg-sign", "-m", commit_message])
            .output()?;

        if !commit_output.status.success() {
            let stderr = String::from_utf8_lossy(&commit_output.stderr);
            // Check if it's "nothing to commit" which isn't really an error
            if stderr.contains("nothing to commit") || stderr.contains("no changes added") {
                return Err(anyhow::anyhow!("Nothing to commit - no changes staged"));
            }
            return Err(anyhow::anyhow!("git commit failed: {}", stderr));
        }

        // Push changes - let git use its configured credential helper
        // Don't set GIT_TERMINAL_PROMPT=0 as it breaks credential helpers
        debug!("Pushing changes...");
        let push_output = Command::new("git")
            .args(&["push"])
            .output()?;

        if !push_output.status.success() {
            let stderr = String::from_utf8_lossy(&push_output.stderr);
            let stdout = String::from_utf8_lossy(&push_output.stdout);
            error!("git push failed - stderr: {}", stderr);
            error!("git push failed - stdout: {}", stdout);

            // Provide user-friendly error messages
            if stderr.contains("Authentication failed") || stderr.contains("could not read Username") {
                return Err(anyhow::anyhow!(
                    "Push failed: Authentication required. Please configure git credentials:\n\
                     • For HTTPS: Run 'git config --global credential.helper osxkeychain' (macOS) or 'git config --global credential.helper cache' (Linux)\n\
                     • For SSH: Ensure your SSH key is added to ssh-agent"
                ));
            }
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
    use git2::{Repository, Signature, CredentialType};

    let repo = Repository::open(worktree_path)?;

    // Stage all changes
    let mut index = repo.index()?;

    // Add all files in the working directory
    index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
    index.write()?;

    // Try to get user config for commit signature
    let config = repo.config().ok();
    let user_name = config
        .as_ref()
        .and_then(|c| c.get_string("user.name").ok())
        .unwrap_or_else(|| "Claude Box".to_string());
    let user_email = config
        .as_ref()
        .and_then(|c| c.get_string("user.email").ok())
        .unwrap_or_else(|| "claude-box@local".to_string());

    // Create commit
    let signature = Signature::now(&user_name, &user_email)?;
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

    // Set up credentials callback that handles both SSH and HTTPS
    let mut callbacks = git2::RemoteCallbacks::new();
    callbacks.credentials(|url, username_from_url, allowed_types| {
        debug!("Credential callback: url={}, username={:?}, allowed={:?}",
               url, username_from_url, allowed_types);

        // Try SSH key from agent first (for git@github.com URLs)
        if allowed_types.contains(CredentialType::SSH_KEY) {
            debug!("Trying SSH key from agent");
            if let Ok(cred) = git2::Cred::ssh_key_from_agent(username_from_url.unwrap_or("git")) {
                return Ok(cred);
            }
        }

        // Try default credentials (for HTTPS with credential helper)
        if allowed_types.contains(CredentialType::USER_PASS_PLAINTEXT) {
            debug!("Trying default credentials for HTTPS");
            // Try to use git credential helper via command line
            if let Some(creds) = get_git_credentials(url) {
                return git2::Cred::userpass_plaintext(&creds.0, &creds.1);
            }
        }

        // Try default (for systems with credential managers)
        if allowed_types.contains(CredentialType::DEFAULT) {
            debug!("Trying default credentials");
            return git2::Cred::default();
        }

        Err(git2::Error::from_str("No suitable credentials found"))
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
                    "Authentication failed. Please configure git credentials:\n\
                     • For HTTPS: git config --global credential.helper osxkeychain (macOS)\n\
                     • For SSH: Ensure SSH key is added to ssh-agent"
                }
                _ => "Push failed. Please check your remote repository configuration.",
            };

            error!("git2 push failed: {}", e);
            Err(anyhow::anyhow!("Push failed: {}", user_friendly_msg))
        }
    }
}

/// Try to get credentials from git's credential helper
fn get_git_credentials(url: &str) -> Option<(String, String)> {
    use std::io::Write;

    // Parse the URL to get protocol and host
    let protocol = if url.starts_with("https://") { "https" } else { "http" };
    let host = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("");

    if host.is_empty() {
        return None;
    }

    // Ask git credential helper for credentials
    let mut child = match Command::new("git")
        .args(&["credential", "fill"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return None,
    };

    // Write credential request to stdin
    if let Some(mut stdin) = child.stdin.take() {
        let input = format!("protocol={}\nhost={}\n\n", protocol, host);
        if stdin.write_all(input.as_bytes()).is_err() {
            return None;
        }
    }

    // Read response
    let output = match child.wait_with_output() {
        Ok(o) => o,
        Err(_) => return None,
    };

    if !output.status.success() {
        return None;
    }

    // Parse response
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut username = None;
    let mut password = None;

    for line in stdout.lines() {
        if let Some(user) = line.strip_prefix("username=") {
            username = Some(user.to_string());
        } else if let Some(pass) = line.strip_prefix("password=") {
            password = Some(pass.to_string());
        }
    }

    match (username, password) {
        (Some(u), Some(p)) => Some((u, p)),
        _ => None,
    }
}
