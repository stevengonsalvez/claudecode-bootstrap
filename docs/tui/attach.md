---
title: "Attaching to tmux sessions"
description: "Full-screen attach ('a') and in-pane embedded attach ('A') in ainb."
---

Every row in the session list (agent sessions, workspace shells, and external sessions under **Other tmux**) is backed by a real tmux session. `ainb` gives you two ways to attach:

1. **Full-screen attach (`a`):** The TUI suspends and the standard tmux client takes over your terminal.
2. **In-pane attach (`A`):** The preview pane becomes a live embedded tmux client inside `ainb`.

Leaving either mode never kills the session: detaching only disconnects the client, and the agent continues running in the background.

## Full-screen attach (`a`)

![Full-screen attach: 'a' suspends the TUI into the real tmux client, a command runs inside the session, Ctrl+B d detaches back to ainb](../assets/screenshots/attach-fullscreen.gif)

Press `a` on any selected session. The TUI suspends and `tmux attach-session` fills the entire terminal. You have access to the full tmux status bar, prefix keys, copy mode, and scrollback.

```text
s                       open the session list
a                       full-screen attach: TUI suspends, tmux takes terminal
echo FULLSCREEN_OK ⏎    executed inside the session
Ctrl+B d                detach: ainb resumes, session continues running
```

Use full-screen attach when you want to work inside a session for an extended period.

## In-pane attach (`A`)

![In-pane attach: Shift+A turns the preview pane into a live embedded tmux client with an INTERACTIVE badge](../assets/screenshots/attach-in-pane.gif)

Press `A` (Shift+A) to turn the right preview pane into an interactive terminal in place. An `INTERACTIVE` badge appears and all keystrokes route directly into the session.

```text
s                       open the session list
A                       in-pane attach: INTERACTIVE badge, embed fills the pane
echo INPANE_OK ⏎        executed inside the embedded session
Ctrl+Q                  release: badge clears, read-only preview restores
```

While active, `Ctrl+Q` is the reserved shortcut to release keyboard focus back to `ainb`. Press `B` to collapse the left sidebar for extra terminal width.

Use in-pane attach for quick interventions: answering prompts, confirming commands, or clearing errors without leaving the session list.

## Comparison

| Feature | Full-screen (`a`) | In-pane (`A`) |
|---------|-------------------|---------------|
| Surface | Whole terminal (TUI suspends) | Right pane (ainb remains visible) |
| Visual indicator | Full tmux status bar | `● INTERACTIVE` badge on pane |
| Exit command | `Ctrl+B d` (standard tmux detach) | `Ctrl+Q` |
| Return state | Resumes TUI session list | Returns to read-only live preview |
| Session survival | Survives disconnect | Survives disconnect |

## Keyboard controls

| Key | Action |
|-----|--------|
| `a` | Full-screen attach to selected session |
| `A` | In-pane embedded attach |
| `Ctrl+B d` | Detach from full-screen session |
| `Ctrl+Q` | Release in-pane embed back to TUI |
| `1` to `9` | Quick-attach using row index badges |
| `B` | Toggle sidebar to expand pane width |

## Next steps

- [Starting a new session](/tui/start-session): wizard configuration options
- [Code review diff](/tui/code-review): inspect file diffs before attaching
- [Keyboard shortcuts](/tui/keyboard-shortcuts): all navigation bindings
