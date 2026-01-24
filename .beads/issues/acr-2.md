---
id: acr-2
title: "Update create-rule.js for claude-code to use packages/"
type: task
status: closed
priority: 1
created: 2026-01-24
blocked_by: [acr-1]
---

# Update create-rule.js for claude-code to use packages/

Make claude-code installation use packages/ structure like Codex already does.

## Changes Required

```javascript
'claude-code': {
    usePackagesStructure: true,  // Like Codex
    packageMappings: {
        'agents': 'agents',
        'skills': 'skills',
        'workflows/single-agent/commands': 'commands',
        'workflows/multi-agent/commands': 'commands',
        'utilities/commands': 'commands',
        'utilities/templates': 'templates',
        'utilities/hooks': 'hooks',
    },
    additionalFiles: ['claude-code-4.5/CLAUDE.md', 'claude-code-4.5/settings.json']
}
```

## Acceptance Criteria

- [ ] claude-code config uses usePackagesStructure: true
- [ ] packageMappings correctly maps all package directories
- [ ] CLAUDE.md and settings.json still come from claude-code-4.5/
