---
title: "Value proposition"
description: "What problems ainb solves when running multiple AI coding agents in parallel."
---

One coding agent is a chat window. Five agents is an operations problem. `ainb` is built to solve that operations problem.

## Problems solved

| Challenge | Without ainb | With ainb |
|-----------|--------------|-----------|
| **Branch clobbering** | Agents share one working directory, corrupting uncommitted files and colliding on branches. | Every session gets an isolated git worktree directory and branch (`agents/<hash>`). |
| **Session survival** | Closing a terminal window or laptop sleep kills the running agent. | Agent runs in a background tmux session. Detach with `Ctrl-b d`, reattach with `ainb attach <name>`. |
| **Human in the loop** | You cycle through terminal tabs constantly checking if agents need input. | Central Fleet overlay (`f`) and Inbox (`b`) display all pending ASK and APPROVE requests in one place. |
| **Code verification** | You blindly commit or run external diff tools to see what the agent changed. | Warp-style hunk-by-hunk diff review (`g`) with collapsible files and word-level syntax highlighting. |
| **Cost attribution** | Monthly API invoices show a single lump sum across all repos. | `burndown` tracks token spend per project, per model, and per day from local JSONL logs. |
| **Tool drift** | Rules and skills copied across `~/.claude`, `~/.codex`, and `~/.copilot` drift out of sync. | `ainb-toolkit` writes skills and agents once, deploying to 9 tools from a single source. |

## What it costs

- **Zero subscriptions:** Open source (MIT), runs completely on your local machine.
- **Zero telemetry:** Does not phone home. The only network calls are those made by your AI providers.
- **Zero lock-in:** Controls tools through standard PTYs, not proprietary wrapper SDKs.

## Operational boundaries

- **Worktree isolation isolates git state, not system services:** Running agents still share local ports (e.g. 3000) and databases on your machine.
- **Prerequisites:** `git` (2.30+) and `tmux` (3.2+) are required.
- **Windows:** Supported through WSL2.

## Next steps

- [Overview](/product/what-is-ainb): monorepo architecture and component breakdown
- [Install](/tui/install): installation methods
- [Quickstart](/tui/quickstart): launch your first session in 60 seconds
- [Concepts](/product/concepts): workspaces, worktrees, and attention
