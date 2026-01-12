# Configuration Files

This directory contains recommended configuration files for optimal experience with agents-in-a-box.

## tmux.conf

**Purpose**: Reduces screen flickering when using Claude Code and other AI CLIs inside tmux.

### The Problem

Claude Code generates 4,000-6,700 scroll events per second when actively updating its display. This causes visible flickering when running inside tmux due to rapid cursor movements and screen redraws.

See: [anthropics/claude-code#9935](https://github.com/anthropics/claude-code/issues/9935)

### Installation

**Option 1: Replace your tmux config** (if you don't have custom settings)
```bash
cp config/tmux.conf ~/.tmux.conf
tmux source-file ~/.tmux.conf  # reload in existing sessions
```

**Option 2: Append to existing config**
```bash
cat config/tmux.conf >> ~/.tmux.conf
tmux source-file ~/.tmux.conf
```

### Key Settings

| Setting | Default | Recommended | Purpose |
|---------|---------|-------------|---------|
| `escape-time` | 500ms | 0 | Eliminates input lag |
| `status-interval` | 15s | 30s | Reduces status bar redraws |
| `automatic-rename` | on | off | Reduces CPU overhead |
| `set-titles` | on | off | Prevents title update flicker |

### Note

These settings help significantly but won't eliminate flicker entirely - the fundamental issue is the high scroll event rate from Claude Code. For a flicker-free experience, consider running Claude Code outside of tmux.
