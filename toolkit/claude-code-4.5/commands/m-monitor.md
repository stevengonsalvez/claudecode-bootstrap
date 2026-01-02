---
description: Multi-agent monitoring - Real-time dashboard for orchestration sessions
tags: [orchestration, monitoring, multi-agent]
---

# Multi-Agent Monitoring (`/m-monitor`)

You are now in **multi-agent monitoring mode**. Display a real-time dashboard of the orchestration session status.

## Your Role

Act as a **monitoring dashboard** that displays live status of all agents, waves, costs, and progress.

## Usage

```bash
/m-monitor <session-id>
```

## Display Format

```
🚀 Multi-Agent Session: orch-1763400000

📊 Plan Summary:
  - Task: Implement BigCommerce migration
  - Created: 2025-11-17 10:00:00
  - Total Workstreams: 7
  - Total Waves: 4
  - Max Concurrent: 4

🌊 Wave 1: Complete ✅ (Duration: 8m 23s)
  ✅ agent-ws1-1763338466 (WS-1: Service Layer)
     Status: complete | Cost: $1.86 | Branch: feat/ws-1
     Worktree: worktrees/ws-1-service-layer
     Last Update: 2025-11-17 10:08:23

  ✅ agent-ws3-1763338483 (WS-3: Database Schema)
     Status: complete | Cost: $0.79 | Branch: feat/ws-3
     Worktree: worktrees/ws-3-database-schema
     Last Update: 2025-11-17 10:08:15

🌊 Wave 2: Active 🔄 (Elapsed: 3m 12s)
  🔄 agent-ws2-1763341887 (WS-2: Edge Functions)
     Status: active | Cost: $0.45 | Branch: feat/ws-2
     Worktree: worktrees/ws-2-edge-functions
     Last Update: 2025-11-17 10:11:35
     Attach: tmux attach -t agent-ws2-1763341887

  🔄 agent-ws4-1763341892 (WS-4: Frontend UI)
     Status: active | Cost: $0.38 | Branch: feat/ws-4
     Worktree: worktrees/ws-4-frontend-ui
     Last Update: 2025-11-17 10:11:42
     Attach: tmux attach -t agent-ws4-1763341892

🌊 Wave 3: Pending ⏸️
  ⏸️  agent-ws5-pending (WS-5: Checkout Flow)

🌊 Wave 4: Pending ⏸️
  ⏸️  agent-ws6-pending (WS-6: E2E Tests)
  ⏸️  agent-ws7-pending (WS-7: Documentation)

💰 Budget Status:
  - Current Cost: $3.48
  - Budget Limit: $50.00
  - Usage: 7% 🟢

⏱️  Timeline:
  - Total Elapsed: 11m 35s
  - Estimated Remaining: ~5h 30m

📋 Commands:
  - Refresh: /m-monitor <session-id>
  - Attach to agent: tmux attach -t <agent-id>
  - View agent output: tmux capture-pane -t <agent-id> -p
  - Kill idle agent: ~/.claude/utils/orchestrator-agent.sh kill <agent-id>
  - Pause session: Ctrl+C (agents continue in background)
  - Resume session: /m-implement <session-id> --resume

Status Legend:
  ✅ complete  🔄 active  ⏸️ pending  ⚠️ idle  ❌ failed  💀 killed
```

## Implementation (Phase 2)

**This is a stub command for Phase 1.** Full implementation in Phase 2 will include:

1. **Live monitoring loop** - Refresh every 30s
2. **Interactive controls** - Pause, resume, kill agents
3. **Cost tracking** - Real-time budget updates
4. **Idle detection** - Highlight idle agents
5. **Failure alerts** - Notify on failures
6. **Performance metrics** - Agent completion times

## Current Workaround

**Until Phase 2 is complete, use these manual commands:**

```bash
# View session status
~/.claude/utils/orchestrator-state.sh print <session-id>

# List all agents
~/.claude/utils/orchestrator-state.sh list-agents <session-id>

# Check specific agent
~/.claude/utils/orchestrator-state.sh get-agent <session-id> <agent-id>

# Attach to agent tmux session
tmux attach -t <agent-id>

# View agent output without attaching
tmux capture-pane -t <agent-id> -p | tail -50
```

---

**End of `/m-monitor` command (stub)**
