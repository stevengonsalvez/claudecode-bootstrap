---
title: "Fleet chat bridge"
description: "Two-way mobile bridge for Telegram, Slack, and Discord built into ainb."
---

The `ainb fleet bridge` is a built-in background daemon that relays messages two-way between mobile chat applications and active `ainb` sessions. You can supervise and unblock agent sessions from your phone while away from the keyboard.

Supported platforms:
- **Telegram** (long-polling bot)
- **Slack** (Socket Mode app)
- **Discord** (Gateway WebSocket bot)

```
  Telegram (long-poll) ─▶
  Slack (socket-mode)  ─▶ ainb fleet bridge ─tmux send-keys─▶ ainb session
  Discord (gateway)    ─▶  (shared relay core)                │
     ▲                         │                              ▼
     └──── reply split ────────┴──── wait for assistant ◀──── read JSONL
```

## How it works

1. **Session discovery:** The bridge discovers running sessions via `ainb list --format json`.
2. **Dispatch:** Messages are injected safely into the target tmux session via `tmux send-keys`.
3. **Reply capture:** The bridge monitors the session's JSONL transcript and extracts the assistant's turn output upon completion (`end_turn`).
4. **Routing:** Prefix a message with `name: <text>` to target a specific session (e.g. `auth: run the tests`). Unprefixed messages route to the active conductor or primary session.

## Configuration

Add your channel credentials to `~/.agents-in-a-box/config/config.toml`:

```toml
[fleet.bridge]
response_timeout = 300              # optional shared default (seconds)

[fleet.bridge.telegram]
token = "$TELEGRAM_BOT_TOKEN"       # supports env vars or keychain:service
user_id = 123456789                # authorized Telegram user id
default_target = "conductor"        # optional session target
require_mention_in_groups = true

[fleet.bridge.slack]
bot_token = "$SLACK_BOT_TOKEN"      # xoxb-... (Web API token)
app_token = "$SLACK_APP_TOKEN"      # xapp-... (Socket Mode token)
user_id = "U0123ABC"               # authorized Slack user id
default_target = "conductor"
listen_mode = "mentions"            # "mentions" (default) or "all"

[fleet.bridge.discord]
token = "$DISCORD_BOT_TOKEN"        # Discord bot token
user_id = "123456789012345678"     # authorized Discord snowflake id
default_target = "conductor"
channel_id = "123456789012345678"  # optional fallback channel
```

## CLI commands

```bash
# Run in foreground:
ainb fleet bridge run

# Install as background service (launchd on macOS / systemd on Linux):
ainb fleet bridge install

# Check service and config status:
ainb fleet bridge status

# Stop and remove background service:
ainb fleet bridge uninstall
```

Logs are written to `~/.agents-in-a-box/phone-bridge.log`.

## Next steps

- [ATC background watcher](/atc-plumbing): always-on session watcher
- [Attaching to sessions](/tui/attach): terminal attachment controls
- [CLI reference](/tui/cli): fleet bridge command flags
