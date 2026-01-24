---
id: acr-5
title: "Deprecate duplicate content in claude-code-4.5/"
type: task
status: closed
priority: 3
created: 2026-01-24
blocked_by: [acr-4]
---

# Deprecate duplicate content in claude-code-4.5/

After packages/ is canonical, remove duplicates from claude-code-4.5/.

## What Stays in claude-code-4.5/

- `CLAUDE.md` - Main system prompt (tool-specific)
- `settings.json` - Default settings
- `settings.local.json.example` - Example local settings

## What Gets Removed (moved to packages/)

- `agents/` → now in packages/agents/
- `commands/` → now in packages/workflows/*/commands/ and packages/utilities/commands/
- `skills/` → now in packages/skills/
- `templates/` → now in packages/utilities/templates/
- `hooks/` → now in packages/utilities/hooks/
- `output-styles/` → now in packages/utilities/output-styles/

## Acceptance Criteria

- [ ] claude-code-4.5/ only contains tool-specific files
- [ ] All tools still work via packages/ indirection
- [ ] Documentation updated
