# Plan: close the notch gaps (apps/ainb-fleet-macos)

**Date:** 2026-09-07
**Decision:** Stevie picked "close the gaps" over freezing the Swift app for the Tauri desktop.
**Format:** diagram-first, table-second, no prose paragraphs.

## Where the notch stands against the daemon

```
┌──────────────┐  hangar.sock   ┌──────────────────────┐
│ AINBFleet    │───────────────▶│ hangar daemon 1.25.0 │
│ Swift notch  │ negotiate v2   │ = HEAD on proto      │
└──────┬───────┘                └──────────┬───────────┘
       │ uses: snapshot, fleet/event,      │ has, notch ignores:
       │ chat poll 1/s (5 RPCs incl. a     ▼
       │ WRITE tx), quota, usage, atc  ┌──────────────────────┐
       │                               │ message_subscribe    │ live chat push
       │                               │ transcript_subscribe │ ACP stream
       │                               │ reconcile_structured │ stale interview
       │                               │ adapter_list         │ copilot dial
       │                               │ copilot_configure    │
       │                               └──────────────────────┘
```

## Evidence gathered today

| Fact | Where |
|------|-------|
| Wire enums identical Swift vs Rust; contract CI green on main through 09-07 | `FleetWire.swift`, `fleet.rs`, workflow runs |
| Proto drift since notch last synced is additive only (`provider` optional, `turn_deadline_ms`) | `git diff b95dcf08..HEAD -- ainb-hangar-proto` |
| Notch names `provider: "claude-agent-acp"` and `cwd: $HOME` on EVERY 1 s chat poll | `FleetStore.pageChat` |
| Daemon refuses a named provider or cwd that differs from the held scope (`ScopeHeld`) | `acp_session.rs:178` |
| TUI already omits `provider` for that reason (commit 5de2c20e); still names `cwd` | `fleet/control.rs:221` |
| Live copilot scope is held with cwd = a worktree dir, so the notch is refused right now | `hangar.db fleet_acp_session` |
| Each idempotent create is an INSERT inside a write tx (unique-violation replay) | `fleet_acp_session.rs:515` |
| Daemon is in a `database is locked` storm all day (300 to 1300 per hour) | `hangar/logs/hangar-daemon.stderr.log` |
| `acp_session_create` times out at 5 s live; `channel_list` and `adapter_list` answer in 0.1 s | CLI timing |
| One connection may hold fleet + message + transcript forwarders at once | `rpc/mod.rs:600-665` |
| After `message_subscribe` the same socket gets `message_event`, `confirm_event`, `activity_event` | `copilot.rs`, `spawn_notification_forwarder` |
| Swift already decodes MessageEvent, ConfirmEvent, ActivityEvent, CopilotConfigure frames | `FleetWire.swift:1092+` |
| Only the CLI (`msg follow`, `transcript follow`) consumes live subscriptions today | `cli/fleet/msg.rs`, `cli/fleet/acp.rs` |
| No read RPC for the current copilot engine; TUI dial defaults to first adapter until Applied | `copilot_dial.rs:172` |

## PRs

| id | branch | scope | files | proof | after |
|----|--------|-------|-------|-------|-------|
| A | `f/notch-copilot-scope-reuse` | omit `provider` and `cwd` on create (daemon: `cwd` optional, reuse held); mint once per scope, not per poll; legacy retry ladder | proto `fleet.rs`, `rpc/mod.rs`, `control.rs`, `FleetWire.swift`, `FleetStore.swift` | daemon test, Swift wire + contract tests | none |
| B | `f/notch-chat-live-push` | `fleet/message_subscribe` on the live socket; fold `message_event`, `confirm_event`, `activity_event` into `chat`; poll drops to a 30 s safety net | `FleetConnection.swift`, `FleetStore.swift`, `FleetChatPaneView.swift`, `FleetWire.swift` | contract test: send on one socket, event on the other | A |
| C | `f/notch-acp-transcript` | `transcript_list` + `transcript_subscribe`; Swift port of the `acp.*` classifier arms; transcript section in the chat pane | `FleetConnection.swift`, `FleetStore.swift`, new `FleetTranscriptPresentation.swift`, `FleetChatPaneView.swift` | classifier unit tests mirroring `transcript.rs`; contract ack test | B |
| D | `f/notch-reconcile-interview` | `ControlAction.reconcileStructured`; store intent gated like the TUI; "Verify" button on the interview card | `FleetWire.swift`, `FleetStore.swift`, `FleetDesktopController.swift` | encode test, receipt notice test | A |
| E | `f/notch-copilot-dial` | `adapter_list` + `copilot_configure`; engine, mode, model, effort dial in the chat header | `FleetConnection.swift`, `FleetStore.swift`, `FleetChatPaneView.swift`, `FleetChatPresentation.swift` | contract tests against fixture daemon | B |

## Sequence

```
A ──▶ B ──▶ C
 └──▶ D      └──▶ E
```

Each PR: superstar-engineer builds, code-reviewer reviews, fixes land, CI green, merge commit.

## Out of scope

- `fleet/copilot_gate`: called by the copilot ACP tool gate, not by a UI client.
- The lock storm itself: separate daemon work (retention on `fleet_provider_event`).
- Tauri desktop: `docs/plans/2026-09-04-desktop-shared-core-spec.md`, untouched.
