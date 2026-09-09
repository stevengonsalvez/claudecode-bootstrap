---
title: "agents-in-a-box: documentation"
---

Canonical source of truth for the monorepo. Everything published to the website under `/docs/*` is rendered from these files.

## 1. Start here

- [Overview](product/what-is-ainb.md): what ainb is, who it is for, component breakdown
- [Install](tui/install.md): Homebrew, curl script, prebuilt binaries, cargo build
- [Quickstart](tui/quickstart.md): first session in 60 seconds
- [Concepts](product/concepts.md): workspaces, worktree isolation, tmux persistence, attention
- [Keyboard shortcuts](tui/keyboard-shortcuts.md): complete keybindings table

## 2. Using ainb

- [Starting a new session](tui/start-session.md): repo picker and new-session wizard
- [Attaching to sessions](tui/attach.md): full-screen and in-pane live tmux attach
- [Code review diff](tui/code-review.md): Warp-style diff viewer with collapsible hunks
- [Fleet and chat bridge](fleet-bridge.md): drive the fleet from Telegram, Slack or Discord
- [ATC background watcher](atc-plumbing.md): always-on watcher and session lifecycle
- [Shared MCP pool](tui/mcp-pool.mdx): shared MCP server pool across sessions
- [Token optimisation](tui/token-optimization.mdx): Headroom proxy and RTK context compression
- [Inbox and notifications](tui/inbox-notifications.md): ainb-hooks notification feed
- [Browser dashboard](tui/web.md): read-only web view (`ainb web`)

## 3. Configure and Extend

- [Value proposition](product/value.md): detailed comparison and workflow impact
- [Plugins overview](plugins/overview.md): subprocess plugin system v2
- [Plugins user guide](plugins/user-guide.md): installing and configuring plugins
- [In-tree plugins](plugins/overview.md):
  - [burndown](plugins/burndown.md): cost and spend tracking
  - [session-reader](plugins/session-reader.md): JSONL event streaming
  - [witr](plugins/witr.md): process causality tree
  - [learnings](plugins/learnings.md): knowledge graph
  - [abtop](plugins/abtop.md): fleet process monitor
- [Skill manager guide](skill-manager/guide.mdx): discovery, sync, drift check, and promotion
- [Toolkit overview](toolkit/overview.md): portable skills and agents across 9 tools
- [Reflect memory overview](knowledge/overview.md): GraphRAG and QMD knowledge capture
- [Memory browser](knowledge/reflect-memory/serve.md): `reflect serve` web interface

## 4. Reference

- [CLI reference](tui/cli.md): generated from `--help`, verified in CI
- [Plugin authoring guide](plugins/authoring.md): build native binary plugins
- [Plugin wire spec v2](plugins/spec-v2.md): JSON-RPC protocol specification
- [Fleet cost rollups](tui/fleet-cost.md): spend across sessions and budget alerts
- [Observability overview](observability/overview.md): telemetry and causality
- [OpenTelemetry to Grafana](reference/otel-grafana.md): Prometheus and dashboard setup
- [Architecture deep-dive](reference/architecture.md): whole-system architecture
- [TUI architecture](tui/architecture.md): ratatui host and event loop
- [Repositories map](reference/repositories.md): monorepo crate map
- [Hangar control plane](hangar/architecture.md): task boards and autopilots
- [Reflect memory internals](knowledge/reflect-memory/problem-and-fit.md):
  - [The construct](knowledge/reflect-memory/construct.md)
  - [Recall reference: 57 ports](knowledge/reflect-memory/recall.md)
  - [Comparison](knowledge/reflect-memory/comparison.md)

## 5. Help

- [Troubleshooting and FAQ](tui/faq.md): common questions and fixes
- [Daemons overlay](tui/daemons.mdx): diagnostic overlay for background services
- [Contributing guide](contributing/building.md): build, test, CI/CD, and release
- [Glossary](reference/glossary.md): terms and definitions
