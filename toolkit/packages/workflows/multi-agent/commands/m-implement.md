---
description: Multi-agent implementation - Execute pre-planned DAG with parallel agent waves
tags: [orchestration, implementation, multi-agent]
---

# /m-implement - Multi-Agent Implementation

Execute a pre-planned DAG by spawning agents in waves with automated monitoring.

## Usage

```bash
/m-implement <session_id>
/m-implement <session_id> --resume
/m-implement <session_id> --from-wave 2
```

## Implementation

```bash
#!/bin/bash

# Parse arguments
SESSION_ID="${1:-}"
RESUME_FLAG=""
FROM_WAVE_FLAG=""
FROM_WAVE_NUM=""

shift
while [[ $# -gt 0 ]]; do
    case $1 in
        --resume)
            RESUME_FLAG="--resume"
            shift
            ;;
        --from-wave)
            if [[ -z "${2:-}" ]]; then
                echo "❌ Error: --from-wave requires a wave number." >&2
                exit 1
            fi
            FROM_WAVE_NUM="$2"
            FROM_WAVE_FLAG="--from-wave $2"
            shift 2
            ;;
        *)
            shift
            ;;
    esac
done

# Validate session ID
if [ -z "$SESSION_ID" ]; then
    echo "❌ Error: Session ID required"
    echo ""
    echo "Usage: /m-implement <session_id> [--resume] [--from-wave N]"
    echo ""
    echo "Examples:"
    echo "  /m-implement orch-1735689600"
    echo "  /m-implement orch-1735689600 --resume"
    echo "  /m-implement orch-1735689600 --from-wave 3"
    exit 1
fi

# Verify DAG file exists
DAG_FILE="${HOME}/.claude/orchestration/state/dag-${SESSION_ID}.json"
if [ ! -f "$DAG_FILE" ]; then
    echo "❌ Error: DAG file not found: $DAG_FILE"
    echo ""
    echo "💡 Create a DAG first using /m-plan"
    exit 1
fi

# Verify orchestrator-runner exists
ORCHESTRATOR="${HOME}/.claude/utils/orchestrator-runner.sh"
if [ ! -x "$ORCHESTRATOR" ]; then
    echo "❌ Error: Orchestrator runner not found or not executable: $ORCHESTRATOR"
    exit 1
fi

# Display starting message
echo ""
echo "🎯 Starting multi-agent orchestration..."
echo "   Session: $SESSION_ID"
if [ -n "$RESUME_FLAG" ]; then
    echo "   Mode: Resume from last completed wave"
elif [ -n "$FROM_WAVE_FLAG" ]; then
    echo "   Mode: Start from wave $FROM_WAVE_NUM"
else
    echo "   Mode: Fresh start"
fi
echo ""

# Execute orchestration
$ORCHESTRATOR run "$SESSION_ID" $RESUME_FLAG $FROM_WAVE_FLAG

# Check exit code
EXIT_CODE=$?

case $EXIT_CODE in
    0)
        echo ""
        echo "✨ Orchestration completed successfully!"
        echo ""
        echo "📝 Next Steps:"
        echo "  1. Review agent work: tmux attach -t <agent-id>"
        echo "  2. Merge branches: /merge-agent-work <workstream-id>"
        echo "  3. Run tests"
        echo "  4. Cleanup worktrees: /cleanup-agent-worktree <workstream-id>"
        ;;
    1)
        echo ""
        echo "⚠️  Orchestration failed"
        echo ""
        echo "📝 Recovery Options:"
        echo "  - Check agent outputs: tmux attach -t <agent-id>"
        echo "  - Fix issues and resume: /m-implement $SESSION_ID --resume"
        echo "  - View session state: ~/.claude/utils/orchestrator-state.sh print $SESSION_ID"
        exit 1
        ;;
    2)
        echo ""
        echo "💰 Budget limit exceeded - orchestration stopped"
        echo ""
        echo "📝 Options:"
        echo "  - Review completed work so far"
        echo "  - Increase budget in config.json"
        echo "  - Resume with: /m-implement $SESSION_ID --resume"
        exit 2
        ;;
    *)
        echo ""
        echo "❌ Unknown error occurred (exit code: $EXIT_CODE)"
        exit $EXIT_CODE
        ;;
esac
```

## How It Works

1. **Validates Prerequisites**
   - Session ID provided
   - DAG file exists
   - Orchestrator runner available

2. **Invokes Orchestrator**
   - Delegates to `~/.claude/utils/orchestrator-runner.sh`
   - Passes session ID and flags
   - Streams output in real-time

3. **Handles Results**
   - Success (code 0): Shows next steps
   - Failure (code 1): Offers recovery options
   - Budget exceeded (code 2): Suggests increasing budget

4. **Resume Support**
   - `--resume`: Continues from last completed wave
   - `--from-wave N`: Starts from specific wave
   - Preserves all session state

## Automated Behavior

The orchestrator automatically:
- Loads DAG and calculates waves
- Spawns agents in parallel (max 4 concurrent)
- Monitors agent status every 30s
- Tracks costs and budgets
- Kills idle agents (>15min)
- Handles failures gracefully
- Saves checkpoints after each wave

## Error Handling

| Scenario | Detection | Response |
|----------|-----------|----------|
| Agent fails | Status monitoring | Mark failed, block next wave |
| Agent timeout | Idle >15min | Kill agent, mark killed |
| Budget exceeded | Cost tracking | Stop spawning, complete current wave |
| Missing DAG | File check | Display error, suggest /m-plan |

## Monitoring Active Orchestration

While orchestration runs, you can:
- Monitor progress output (streamed to console)
- Attach to individual agents: `tmux attach -t agent-<id>`
- Press Ctrl+C to pause (agents continue in background)
- Resume later: `/m-implement <session-id> --resume`

## Notes

- DAG must be created first using `/m-plan`
- Agents run in tmux sessions (persistent)
- Each agent gets isolated git worktree
- Costs tracked automatically
- State saved to `~/.claude/orchestration/state/`
