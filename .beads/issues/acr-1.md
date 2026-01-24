---
id: acr-1
title: "Complete packages/ content - fill gaps from claude-code-4.5"
type: epic
status: closed
priority: 1
created: 2026-01-24
---

# Complete packages/ content - fill gaps from claude-code-4.5

Migration prerequisite: Before making packages/ canonical, all content from claude-code-4.5/ must exist in packages/.

## Gap Analysis

| Category | In claude-code-4.5/ | In packages/ | Action |
|----------|---------------------|--------------|--------|
| Multi-agent commands | m-plan, m-implement, m-monitor, m-workflow | Missing | Move to packages/workflows/multi-agent/commands/ |
| sync-learnings | ✅ | ❌ | Move to packages/utilities/commands/ |
| focused-repository-analyzer | ✅ | ❌ | Move to packages/agents/engineering/ |
| session-info, tui-style-guide | ✅ | ❌ | Move to packages/utilities/commands/ |

## Acceptance Criteria

- [ ] All agents synced between claude-code-4.5/agents and packages/agents
- [ ] All commands categorized and placed in appropriate packages/ location
- [ ] All skills synced
- [ ] Diff shows no missing files in packages/
