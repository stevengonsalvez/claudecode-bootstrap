---
title: "TUI screen tour"
description: "Interactive terminal screens and navigation in ainb."
---

`ainb` is a terminal UI and CLI for managing AI coding sessions in isolated git worktrees and tmux sessions.

Launch the TUI with no arguments:

```bash
ainb            # launch the TUI (default when no command is given)
ainb init       # first-time setup and prerequisite check
ainb auth       # set up provider authentication
```

First launch runs an onboarding check for prerequisites (git, tmux, provider CLIs). Use `ainb init --check` to verify your environment headlessly.

## Screen tour

From the home screen, single keys jump to each screen:

| Screen | Key | Description |
|--------|-----|-------------|
| Sessions | `s` | List, attach, restart, and delete agent sessions |
| Agents | `a` | Select a provider or agent before spawning a session |
| Recovery | `R` | Find and resume orphaned sessions or interrupted worktrees |
| Stats | `i` | Usage analytics: Daily, Weekly, Project, Burndown, [Savings](/tui/token-optimization) (`[` / `]` switch tabs) |
| Daemons | `d` | Read-only [Daemons overlay](/tui/daemons): shared MCP pool and Headroom proxy |
| Skills | `k` | Browse and manage available skills |
| Inbox | `I` | Central notification inbox for agent events |
| Config | `C` | View configuration options |

## Session controls

- **Code Review diff (`g`):** Open the Warp-style [Code Review](/tui/code-review) diff for the selected session. Displays file tree, collapsible blocks, syntax highlighting, and word-level emphasis. Also available standalone as `ainb diff-review`.
- **Full-screen attach (`a`):** Attach directly to the session tmux instance. Detach with `Ctrl-b d` to return to the TUI.
- **In-pane attach (`A`):** Embed a live tmux client in-place inside the preview pane. Press `Ctrl+Q` to release keyboard focus back to the TUI.

## Next steps

- [Quickstart](/tui/quickstart): start your first session
- [Starting a new session](/tui/start-session): configure wizard options
- [Attaching to sessions](/tui/attach): full-screen and embedded tmux modes
- [Keyboard shortcuts](/tui/keyboard-shortcuts): complete keymap
- [CLI reference](/tui/cli): all subcommands and flags
