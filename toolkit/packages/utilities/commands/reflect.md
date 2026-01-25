---
allowed-tools:
  - Skill
  - Read
  - Edit
  - Bash
description: |
  Self-improvement reflection. Analyzes conversations for corrections and learnings,
  proposes updates to agent files or creates new skills. Alias for reflect skill.
---

# /reflect - Self-Improvement Reflection

Invokes the `reflect` skill for conversation analysis and self-improvement.

**Philosophy**: "Correct once, never again."

## Usage

```bash
/reflect                    # Analyze current conversation
/reflect [agent-name]       # Focus on learnings for specific agent
/reflect on                 # Enable auto-reflection at session end
/reflect off                # Disable auto-reflection
/reflect status             # Show toggle state and metrics
/reflect review             # Review pending low-confidence learnings
```

## Quick Reference

| Command | Action |
|---------|--------|
| `/reflect` | Analyze conversation for learnings |
| `/reflect on` | Enable auto-reflection |
| `/reflect off` | Disable auto-reflection |
| `/reflect status` | Show state and metrics |
| `/reflect review` | Review low-confidence learnings |
| `/reflect [agent]` | Focus on specific agent |

## Implementation

This command invokes the portable `reflect` skill located at:
`toolkit/packages/skills/reflect/SKILL.md`

The skill provides:
- Signal detection with confidence levels (HIGH/MEDIUM/LOW)
- Category classification (Code Style, Architecture, Process, Domain, Tools)
- Agent file mapping and update proposals
- New skill creation from non-trivial learnings
- Metrics tracking and state management

### Skill Components

```
reflect/
├── SKILL.md                      # Core skill workflow
├── scripts/
│   ├── state_manager.py          # State file CRUD
│   ├── signal_detector.py        # Pattern matching
│   └── metrics_updater.py        # Metrics aggregation
├── references/
│   ├── signal_patterns.md        # Detection rules
│   ├── agent_mappings.md         # Target mappings
│   └── skill_template.md         # Skill generation
└── assets/
    ├── reflection_template.md    # Output template
    └── learnings_schema.yaml     # Schema definition
```

## Portability

The reflect skill works with any LLM tool that supports:
- File read/write operations
- Text pattern matching
- Git operations (optional)

State directory is configurable via `REFLECT_STATE_DIR` env var.

## Full Documentation

See the complete skill documentation:
- **Workflow**: `toolkit/packages/skills/reflect/SKILL.md`
- **Signal Patterns**: `toolkit/packages/skills/reflect/references/signal_patterns.md`
- **Agent Mappings**: `toolkit/packages/skills/reflect/references/agent_mappings.md`
- **Skill Template**: `toolkit/packages/skills/reflect/references/skill_template.md`

## Deprecated

The previous agent-based implementation at `agents/meta/reflect.md` is deprecated.
This command now uses the portable skill-based approach for better cross-tool compatibility.
