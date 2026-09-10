---
title: "Keyboard shortcuts"
---

Keys verified against the in-app help overlay (`?`) and the event handlers in `crates/ainb-core/src/app/events.rs`.

## Home screen navigation

| Key | Action |
|-----|--------|
| `s` | Sessions |
| `a` | Agents (agent selection) |
| `R` | Recovery |
| `i` | Stats (usage analytics) |
| `k` | Skills |
| `w` | Witr (process causality) |
| `t` | abtop (top-for-agents monitor) |
| `b` | Inbox (notifications) |
| `f` | Fleet control panel (statuses, answer ASK, approve/deny) |
| `d` | Daemons overlay |
| `c` | Catalog |
| `C` | Config |
| `v` | Changelog |
| `n` | New session |
| `Tab` | Toggle focus (sidebar ↔ content) |
| `?` | Toggle help |
| `q` | Quit |

## Navigation (lists)

| Key | Action |
|-----|--------|
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `h` / `←` | Previous workspace |
| `l` / `→` | Next workspace |
| `g` | Go to top |
| `G` | Go to bottom |

## Session actions

| Key | Action |
|-----|--------|
| `n` | New session (local or remote) |
| `a` | [Attach](/tui/attach) full-screen (TUI suspends; `Ctrl+B` `d` detaches) |
| `A` | [In-pane attach](/tui/attach): the preview pane becomes a live embedded tmux client (`Ctrl+Q` releases) |
| `B` | Toggle the sessions sidebar |
| `e` | Restart stopped session |
| `u` | Re-authenticate credentials |
| `d` | Delete session |
| `x` | Cleanup orphaned containers |
| `f` | Refresh workspaces |
| `Space` | Toggle multi-select |
| `Shift`+`D` | Delete all selected sessions |

## Fleet control panel (`f`)

| Key | Action |
|-----|--------|
| `↑`/`↓` or `k`/`j` | Move row selection |
| `Tab` / `Shift+Tab` | Move the ASK option cursor (on an ASK row) |
| `Enter` / `a` | Answer the selected ASK with the highlighted option |
| `y` / `n` | Approve / deny the selected APPROVE permission request |
| `n` (non-APPROVE row) | Open the new-ATC name prompt |
| `B` | Broadcast a ping prompt to the selected session |
| `r` | Force refresh |
| `q` / `Esc` | Back |

## Daemons overlay (`d`)

| Key | Action |
|-----|--------|
| `r` | Refresh |
| `R` | Restart notifyd: the single resume/repair command for a dead approve socket |
| `Esc` / `q` / `d` | Close |

## Recovery screen

| Key | Action |
|-----|--------|
| `Space` | Toggle select |
| `Shift`+`D` | Delete selected |

## Git actions

| Key | Action |
|-----|--------|
| `g` | Open the [Code Review](/tui/code-review) diff for the selected session |
| `p` | Commit & push |

### Within the Code Review diff

| Key | Action |
|-----|--------|
| `↑` / `↓` | Move across the file tree (file → scroll body) |
| `←` / `→` | Switch between file tree and diff view |
| `Tab` | Toggle focus between file tree and diff view |
| `Space` | Expand / collapse current file |
| `e` | Expand all files |
| `c` | Collapse all files |
| `n` / `N` | Next / previous hunk |
| `[` / `]` | Next / previous file |
| `w` | Toggle word-level diff emphasis |
| `b` | Toggle blame view |
| `s` | Toggle side-by-side vs inline view |
| `Enter` | Open current file in editor |
| `Esc` / `q` | Exit diff viewer |

## Command-line flags

| Key / Flag | Action |
|------------|--------|
| `ainb --worktree` | Force worktree isolation for new session |
| `ainb --create-branch <name>` | Create and switch to new branch in worktree |
| `ainb --repo <path>` | Target specific repository path |
| `ainb --remote-repo <url>` | Clone and open remote repository |
| `ainb --tool <claude\|codex>` | Select agent provider explicitly |
| `ainb --list` | List active sessions and exit |
| `ainb --kill <name>` | Terminate named session |

## Usage / Stats screen

| Key | Action |
|-----|--------|
| `Tab` | Switch usage tab (Daily/Weekly/Project/Burndown/Optimize) |
| `1`–`5` | Today / Week / 30 days / Month / All |
| `p` | Cycle provider (All / Claude / Codex) |
| `/` | Add include filter |
| `x` | Add exclude filter |
| `d` | Enter custom `YYYY-MM-DD YYYY-MM-DD` range |
| `c` | Clear filters |
| `r` | Reload |

## General

| Key | Action |
|-----|--------|
| `?` | Toggle help overlay |
| `q` / `Esc` | Back / quit |
| `Ctrl`+`C` | Force quit |

## See also

- [Overview](/tui/overview)
- [CLI reference](/tui/cli)
- [Docs hub](/readme)
