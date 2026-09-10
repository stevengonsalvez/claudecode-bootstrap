---
title: "Code Review (git diff)"
description: "The Warp-style Code Review surface in ainb: file sidebar, collapsible diff blocks, Dracula syntax highlighting, word-level emphasis, and hunk navigation."
---

The **Code Review** surface displays an agent session's git changes. Press **`g`** on a session to open it, or run **`ainb diff-review [path]`** to review any repository directly without a session.

It provides a hierarchical file sidebar, collapsible per-file diff blocks in a continuous scroll, Dracula syntax highlighting, word-level intra-line emphasis (changed substrings highlighted within row tints), line-number change bars, expandable context, and hunk navigation.

![ainb Code Review surface: file sidebar, per-file diff blocks, syntax highlighting, green/red row tints, word-level emphasis, and a line-number gutter](../assets/screenshots/code-review-diff.gif)

## Keyboard controls

| Key | Action |
|-----|--------|
| `g` | Open Code Review for selected session |
| `↑` / `↓` | Move selection across file tree (auto-scrolls diff body to file) |
| `j` / `k` | Scroll diff body |
| `n` / `N` | Jump to next / previous hunk (with `Hunk x/y` counter) |
| `Space` / `Enter` | Toggle folder, or collapse/expand file diff block |
| `e` / `E` | Expand / collapse all folders |
| `z` | Reveal more context lines at nearest gap |
| `[` / `]` | Jump to previous / next file |
| `Tab` | Cycle Review → Commits → Markdown |
| `Esc` / `q` | Return to previous view |
| **Mouse** | Scroll diff wheel; click tree rows to select files or toggle folders |

## Word-level emphasis

When a line changes, the whole row receives a muted green (added) or red (removed) tint, and the **exact substring that changed** receives a brighter highlight.

![Word-level intra-line emphasis and expand-context](../assets/screenshots/code-review-expand.gif)

## Collapsible context blocks

Each file diff block can be collapsed to its header (`Space` or `Enter`). Hidden context lines can be revealed with `z`.

![Collapsing a file diff block to its header and back](../assets/screenshots/code-review-collapse.gif)

## Hunk navigation

Press `n` and `N` to jump between hunks across all modified files. The header displays a live `Hunk x/y` position indicator.

![Jumping between hunks with n and N](../assets/screenshots/code-review-hunk.gif)

## Standalone `ainb diff-review`

Run the Code Review surface directly from your shell:

```bash
ainb diff-review            # review uncommitted changes in current directory
ainb diff-review ./path     # review another directory or worktree
```

## Next steps

- [Overview](/tui/overview): all TUI screens and shortcuts
- [Attaching to sessions](/tui/attach): inspect running agent tmux sessions
- [CLI reference](/tui/cli): command-line flags for diff-review
