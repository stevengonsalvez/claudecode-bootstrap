---
id: acr-4
title: "Update sync-learnings for 3-way sync to packages/"
type: task
status: closed
priority: 2
created: 2026-01-24
blocked_by: [acr-3]
---

# Update sync-learnings for 3-way sync to packages/

Change sync destination from claude-code-4.5/ to packages/.

## New Flow

```
~/.claude/  ──learnings──→  packages/  ──generates──→  claude-code-4.5/
```

## Directory Mapping

| Source (user-level) | Target (packages) |
|---------------------|-------------------|
| ~/.claude/agents/engineering/ | packages/agents/engineering/ |
| ~/.claude/agents/universal/ | packages/agents/universal/ |
| ~/.claude/commands/ (multi-agent) | packages/workflows/multi-agent/commands/ |
| ~/.claude/commands/ (single-agent) | packages/workflows/single-agent/commands/ |
| ~/.claude/commands/ (utilities) | packages/utilities/commands/ |
| ~/.claude/skills/ | packages/skills/ |
| ~/.claude/templates/ | packages/utilities/templates/ |

## Command Routing Logic

Determine destination based on command name:
- `m-*` → multi-agent workflow
- `plan`, `implement`, `validate`, `research`, `workflow` → single-agent workflow  
- Everything else → utilities

## Acceptance Criteria

- [ ] sync-learnings.md updated with new mappings
- [ ] Command routing logic implemented
- [ ] Tested with actual sync operation
