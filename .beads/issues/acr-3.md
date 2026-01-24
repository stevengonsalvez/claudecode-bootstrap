---
id: acr-3
title: "Test all 5 tool installations after migration"
type: task
status: closed
priority: 1
created: 2026-01-24
blocked_by: [acr-2]
---

# Test all 5 tool installations after migration

Verify all tools still install correctly from packages/ structure.

## Tools to Test

| Tool | Command | Verify |
|------|---------|--------|
| Claude Code | `node create-rule.js --tool=claude-code` | ~/.claude/ populated correctly |
| Codex | `node create-rule.js --tool=codex` | ~/.codex/ with prompts generated |
| Gemini | `node create-rule.js --tool=gemini --targetFolder=./test` | .gemini/ with shared content |
| AmazonQ | `node create-rule.js --tool=amazonq --targetFolder=./test` | .amazonq/rules/ populated |
| Cursor | `node create-rule.js --tool=cursor --targetFolder=./test` | .cursor/rules/ populated |

## Acceptance Criteria

- [ ] All 5 tools install without errors
- [ ] File counts match pre-migration
- [ ] Template substitution works ({{TOOL_DIR}}, {{HOME_TOOL_DIR}})
- [ ] No missing commands, agents, or skills
