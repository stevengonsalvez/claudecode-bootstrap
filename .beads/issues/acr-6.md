---
id: acr-6
title: "Add CI/CD checks for packages sync"
type: task
status: closed
priority: 3
created: 2026-01-24
blocked_by: [acr-5]
---

# Add CI/CD checks for packages sync

Ensure packages/ and installed tools stay in sync.

## Checks to Add

1. **Diff check**: Verify no content drift between packages/ subdirs
2. **Install test**: Run all 5 tool installations in CI
3. **File count validation**: Compare expected vs actual file counts
4. **Template substitution test**: Verify {{TOOL_DIR}} replaced correctly

## Implementation

```yaml
# .github/workflows/toolkit-validation.yml
name: Toolkit Validation
on: [push, pull_request]
jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install tools and verify
        run: |
          for tool in claude-code codex gemini amazonq cursor; do
            node create-rule.js --tool=$tool --targetFolder=./test-$tool
            # Verify expected files exist
          done
```

## Acceptance Criteria

- [ ] CI workflow created
- [ ] All 5 tool installations tested
- [ ] Drift detection implemented
- [ ] PR checks block on validation failure
