# Fix tmux Copy/Paste

## Problem
Copy/paste doesn't work in tmux shell sessions (but works in Claude sessions).

## Root Cause
Tmux sessions created by the TUI lack clipboard configuration. The following tmux options are needed:

1. `set-clipboard on` - Enable OSC 52 clipboard integration
2. Platform-specific copy commands for mouse selection

## Solution

### Create Shared Clipboard Configuration

Add a centralized function in `src/tmux/mod.rs` to configure clipboard for any tmux session:

```rust
/// Configure clipboard integration for a tmux session
pub async fn configure_clipboard(session_name: &str) -> Result<()> {
    use tokio::process::Command;

    // Enable set-clipboard for OSC 52 escape sequence support
    // This allows the terminal to access the system clipboard
    Command::new("tmux")
        .args(["set-option", "-t", session_name, "set-clipboard", "on"])
        .status()
        .await?;

    // For mouse copy mode, configure copy-pipe to system clipboard
    #[cfg(target_os = "macos")]
    {
        // macOS: Use pbcopy
        Command::new("tmux")
            .args(["set-option", "-t", session_name, "copy-command", "pbcopy"])
            .status()
            .await?;
    }

    #[cfg(target_os = "linux")]
    {
        // Linux: Try xclip or xsel
        if which::which("xclip").is_ok() {
            Command::new("tmux")
                .args(["set-option", "-t", session_name, "copy-command", "xclip -selection clipboard"])
                .status()
                .await?;
        }
    }

    Ok(())
}
```

### Integration Points

1. **`src/tmux/session.rs`** - Add to `configure_session()`
2. **`src/interactive/session_manager.rs`** - Add to `configure_tmux_session()`
3. **`src/main.rs`** - Add after shell session creation (both `OpenWorkspaceShell` and `OpenShellAtPath`)

### Files to Modify

| File | Change |
|------|--------|
| `src/tmux/mod.rs` | Add `configure_clipboard()` function |
| `src/tmux/session.rs` | Call `configure_clipboard()` in `configure_session()` |
| `src/interactive/session_manager.rs` | Call `configure_clipboard()` in `configure_tmux_session()` |
| `src/main.rs` | Add clipboard config after shell session creation |

## Verification

- [ ] Test copy FROM tmux shell (select text, should be in system clipboard)
- [ ] Test paste INTO tmux shell (system clipboard content should paste)
- [ ] Test on macOS (pbcopy/pbpaste)
- [ ] Test both workspace shell (`$` key) and ad-hoc shells

## Notes

- OSC 52 (set-clipboard) is the modern way to handle clipboard in terminals
- Mouse selection + drag should trigger copy
- Paste uses the terminal's paste (Cmd+V on macOS) which sends the clipboard through the PTY
