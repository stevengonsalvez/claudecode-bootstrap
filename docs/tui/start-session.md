---
title: "Starting a new session"
description: "Configure options in the new-session wizard: repo, preset, agent, model, mode, yolo, and branch."
---

Every session in `ainb` runs in its own **git worktree + tmux + agent**, fully isolated. The new-session wizard is where you choose what runs and where.

## Open the wizard

From the home or Sessions screen, press **`n`**. `ainb` opens the repo picker (type to filter, or paste an `owner/repo`, an `https://` or `ssh://` URL, or a local directory path), then press `Enter` to select it.

You land on the **Configure** screen. Move between rows with **`Tab`** (or `↑` / `↓`), change the focused value with **`←` / `→`**, and launch from the **`[ Launch ]`** row:

![The new-session Configure wizard cycling through options: Preset, Agent, Model, Mode, Yolo, Branch, and Launch](../assets/screenshots/new-session-options.gif)

## Wizard options

### Preset

A named bundle of all settings below. `ainb` includes presets like `claude-interactive-yolo`, `codex-interactive-yolo`, `opusplan`, and `shell`, with **`Custom`** at the end:

- Pick a **named preset** to lock rows to proven defaults.
- Pick **`Custom`** to unlock every row for manual tuning.
- After editing Custom, press **`Ctrl+S`** to save it as a new named preset.

### Agent

Select which tool drives the session:

| Agent | Description |
|-------|-------------|
| **Claude** | Anthropic's Claude Code |
| **Codex** | OpenAI's Codex CLI |
| **Copilot** | GitHub Copilot CLI |
| **Gemini** `[soon]` | Greyed out, non-selectable placeholder |
| **Shell** | Plain terminal with no agent |
| **SSH** | Remote host execution |

### Model

Model selector for supported providers. **Claude** and **Codex** expose model variants (Claude Opus, Sonnet, Haiku; Codex GPT variants). Other agents default to their CLI system configuration.

### Mode

- **Interactive:** You drive and the agent asks for confirmation. Standard mode.
- **Boss** `[alpha]`: Autonomous mode where the agent executes independently.

### Yolo

Controls automatic permission grants:

- **ON:** Auto-approve tool operations (fast, but permits unprompted commands).
- **OFF:** Prompt for confirmation on sensitive actions.

### Branch

Each session gets its own git worktree. The row displays **`<source> → <worktree>`** (base branch and new session branch, default `agents/<hash>`). Press `Enter` to rename or choose another base.

### Launch

Press `Enter` on **`[ Launch ]`**, or press **`Ctrl+Enter`** from any row to quick-launch with active settings.

## Keyboard controls

| Key | Action |
|-----|--------|
| `Tab` / `↑` / `↓` | Move between setting rows |
| `←` / `→` | Cycle focused value |
| `Enter` | Launch session, or edit branch target |
| `Ctrl+Enter` | Quick-launch from any row |
| `Ctrl+S` | Save Custom settings as new preset |
| `Esc` | Return to repo picker |

## Next steps

- [Quickstart](/tui/quickstart): command-line `ainb run` equivalents
- [Attaching to sessions](/tui/attach): full-screen and embedded tmux attach
- [Keyboard shortcuts](/tui/keyboard-shortcuts): complete keymap
- [Overview](/tui/overview): all TUI screens
