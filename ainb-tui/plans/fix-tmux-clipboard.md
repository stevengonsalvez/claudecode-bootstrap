# Fix tmux Copy/Paste

## Problem
Copy/paste doesn't work in tmux shell sessions (but works in Claude sessions).

## Root Cause
Tmux sessions created by the TUI lack clipboard configuration. The following tmux options are needed:

1. `set-clipboard on` - Enable OSC 52 clipboard integration
2. Platform-specific copy commands for mouse selection

## Solution

### Implementation

Added `configure_clipboard()` function in `src/tmux/mod.rs` that:

1. **Enables OSC 52 clipboard** via `set-option set-clipboard on`
2. **Binds mouse selection** to system clipboard using `copy-pipe-and-cancel`
3. **Supports both copy modes**: `copy-mode` (emacs) and `copy-mode-vi`

```rust
/// Configure clipboard integration for a tmux session
pub async fn configure_clipboard(session_name: &str) -> Result<()> {
    // Enable set-clipboard for OSC 52 escape sequence support
    let output = Command::new("tmux")
        .args(["set-option", "-t", session_name, "set-clipboard", "on"])
        .output()
        .await?;

    if !output.status.success() {
        anyhow::bail!("Failed to set tmux option set-clipboard: {}",
            String::from_utf8_lossy(&output.stderr));
    }

    // Platform-specific clipboard binding
    #[cfg(target_os = "macos")]
    bind_clipboard_for_copy_modes(session_name, "pbcopy").await?;

    #[cfg(target_os = "linux")]
    // Uses xclip or xsel if available
    ...
}

/// Bind clipboard for both copy-mode and copy-mode-vi
async fn bind_clipboard_for_copy_modes(session_name: &str, copy_cmd: &str) -> Result<()> {
    for mode in ["copy-mode-vi", "copy-mode"] {
        let output = Command::new("tmux")
            .args([
                "bind-key", "-T", mode, "MouseDragEnd1Pane",
                "send-keys", "-X", "copy-pipe-and-cancel", copy_cmd
            ])
            .output()
            .await?;

        if !output.status.success() {
            anyhow::bail!("Failed to bind tmux key for {}: {}",
                mode, String::from_utf8_lossy(&output.stderr));
        }
    }
    Ok(())
}
```

### Integration Points

| File | Change |
|------|--------|
| `src/tmux/mod.rs` | Added `configure_clipboard()` and `bind_clipboard_for_copy_modes()` |
| `src/tmux/session.rs` | Calls `configure_clipboard()` in `configure_session()` |
| `src/interactive/session_manager.rs` | Calls `configure_clipboard()` in `configure_tmux_session()` |
| `src/main.rs` | Calls clipboard config after shell session creation |

## Verification

- [x] Build passes
- [ ] Test copy FROM tmux shell (select text, should be in system clipboard)
- [ ] Test paste INTO tmux shell (system clipboard content should paste)
- [ ] Test on macOS (pbcopy/pbpaste)
- [ ] Test both workspace shell (`$` key) and ad-hoc shells

## Notes

- OSC 52 (set-clipboard) is the modern way to handle clipboard in terminals
- Mouse selection + drag triggers copy via `MouseDragEnd1Pane` binding
- Paste uses the terminal's paste (Cmd+V on macOS) which sends clipboard through PTY
- Proper error handling with `.output()` and `status.success()` checks
