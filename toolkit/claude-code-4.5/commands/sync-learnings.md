---
description: Sync user-level Claude config changes back to toolkit repository
---

# Sync Learnings

Detect and sync changes made at user-level (`~/.claude/`) back to the toolkit repository.

## Purpose

When working on projects, learnings get captured in user-level agent files via `/reflect`. This command syncs those improvements back to the toolkit for version control and sharing.

## Workflow

1. **Detect Changes**: Compare `~/.claude/` with `toolkit/claude-code-4.5/`
2. **Show Diff**: Display what's different (user-level vs toolkit)
3. **Selective Sync**: Allow choosing which changes to sync
4. **Apply & Commit**: Copy changes to toolkit and prepare for commit

## Directories to Sync

| Source (user-level) | Target (toolkit) |
|---------------------|------------------|
| `~/.claude/agents/` | `toolkit/claude-code-4.5/agents/` |
| `~/.claude/commands/` | `toolkit/claude-code-4.5/commands/` |
| `~/.claude/hooks/` | `toolkit/claude-code-4.5/hooks/` |
| `~/.claude/templates/` | `toolkit/claude-code-4.5/templates/` |

## Exclusions (never sync back)

These directories contain ephemeral or user-specific content:

- `session/` - Session state (ephemeral)
- `reflections/` - Reflection logs (stay at user-level)
- `plans/` - Plan files (temporary)
- `settings.json` - User-specific settings
- `settings.local.json` - Local overrides

## Execution Steps

### Step 1: Find Differences

```bash
# Compare agents
diff -rq ~/.claude/agents/ toolkit/claude-code-4.5/agents/ 2>/dev/null | grep -v "^Only in"

# Compare commands
diff -rq ~/.claude/commands/ toolkit/claude-code-4.5/commands/ 2>/dev/null | grep -v "^Only in"

# Compare hooks
diff -rq ~/.claude/hooks/ toolkit/claude-code-4.5/hooks/ 2>/dev/null | grep -v "^Only in"
```

### Step 2: For Each Difference

1. Show unified diff between user-level and toolkit version
2. Highlight what changed (additions, removals, modifications)
3. Ask: "Sync this change to toolkit? [y/n/s(kip all)/v(iew full)]"

### Step 3: Apply Approved Changes

```bash
# Copy approved file from user-level to toolkit
cp ~/.claude/agents/engineering/example.md toolkit/claude-code-4.5/agents/engineering/example.md

# Stage for commit
git add toolkit/claude-code-4.5/agents/engineering/example.md
```

### Step 4: Generate Commit

Create a commit with summary of synced changes:

```
feat(agents): sync learnings from reflect sessions

Synced changes:
- agents/engineering/test-writer-fixer.md: Added Multi-Layer Verification
- agents/engineering/playwright-test-validator.md: Added Layer 0 env check
```

## New Files Handling

For files that exist in `~/.claude/` but not in toolkit:

1. Show the new file content
2. Ask: "This is a NEW file. Add to toolkit? [y/n]"
3. If yes, copy and stage

## Safety Checks

- **Never overwrite** without showing diff first
- **Warn** if toolkit version is newer (modified after user-level)
- **Skip binaries** and non-text files
- **Validate** markdown frontmatter before copying agent files

## Example Session

```
$ /sync-learnings

Scanning for differences...

Found 2 files with changes:

1. agents/engineering/test-writer-fixer.md
   + Added "Test Verification Strategy" section (17 lines)
   Sync? [y/n/v]: y
   ✓ Synced

2. agents/engineering/playwright-test-validator.md
   + Added "Layer 0: Environment Configuration" section (16 lines)
   Sync? [y/n/v]: y
   ✓ Synced

Summary:
- 2 files synced
- 0 files skipped

Ready to commit? [y/n]: y
Created commit: feat(agents): sync learnings from reflect sessions
```

## Automation Tip

Add to session start hook for automatic detection:

```bash
# In hooks/session-start
DIFF_COUNT=$(diff -rq ~/.claude/agents/ toolkit/claude-code-4.5/agents/ 2>/dev/null | grep "differ" | wc -l)
if [ "$DIFF_COUNT" -gt 0 ]; then
  echo "⚠️  $DIFF_COUNT agent files differ from toolkit. Run /sync-learnings to sync."
fi
```
