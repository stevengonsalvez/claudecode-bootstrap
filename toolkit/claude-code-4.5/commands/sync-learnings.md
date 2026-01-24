---
description: Sync user-level Claude config changes back to toolkit repository
---

# Sync Learnings

Bidirectional sync between `~/.claude/` (user-level) and toolkit repository.

## Purpose

When working on projects, learnings get captured in user-level agent files via `/reflect`. This command syncs improvements bidirectionally:
- **TO_REPO**: New/updated files in ~/.claude that should be version controlled
- **TO_HOME**: Newer files in repo that should update ~/.claude

## Workflow

1. **Assess**: Compare directories and categorize differences
2. **Plan**: Generate sync plan table with actions and rationale
3. **Execute**: Copy files in parallel where possible
4. **Commit**: Single commit for TO_REPO changes

## Directories to Sync

| Source (user-level) | Target (toolkit) |
|---------------------|------------------|
| `~/.claude/agents/` | `toolkit/claude-code-4.5/agents/` |
| `~/.claude/commands/` | `toolkit/claude-code-4.5/commands/` |
| `~/.claude/templates/` | `toolkit/claude-code-4.5/templates/` |

## Exclusion Categories (Never Sync)

### Category 1: Personal Instrumentation
Files with personal/optional integrations that others may not have:
- `hooks/*.py` with Langfuse, telemetry, or personal API integrations
- Keep separate - don't pollute shared repo with personal tooling

### Category 2: Project-Specific Commands
Commands that only make sense for specific private projects:
- `commands/data-setup-*.md` - Project-specific data setup
- `commands/load-frameworks.md` - Project-specific framework loading
- Keep in ~/.claude only if private, or in project `.claude/` if shareable

### Category 3: Session/Ephemeral Data
Runtime state that shouldn't be versioned:
- `session/` - Session state files
- `reflections/` - Reflection logs
- `plans/` - Temporary plan files
- `*.json` session files (agent-*.json, webapp-testing.yaml, etc.)

### Category 4: User Settings
Personal configuration:
- `settings.json` - User-specific settings
- `settings.local.json` - Local overrides

## Assessment Phase

Before syncing, generate an assessment table:

```markdown
# Sync Assessment: ~/.claude ↔ toolkit/claude-code-4.5

## Summary

| Action | Files | Reason |
|--------|-------|--------|
| **SYNC TO REPO** | N files | Useful generic additions |
| **SYNC TO ~/.claude** | N files | Repo has newer versions |
| **DON'T SYNC** | Multiple | Project-specific or session data |

---

## ✅ SYNC TO REPO (from ~/.claude)

### 1. `path/to/file.md` (NEW|UPDATED)
- **Purpose**: [what this file does]
- **Why sync**: [why it's generic/useful]
- **Assessment**: ✅ Valuable addition

---

## ⬇️ SYNC TO ~/.claude (from repo)

### 1. `path/to/file.md`
- **Status**: Exists in repo but NOT in ~/.claude | Repo version is NEWER
- **What's new**: [description of changes]
- **Assessment**: ✅ Copy to ~/.claude

---

## ❌ DON'T SYNC

### [Category Name]
- `file1.md` - [reason]
- `file2.md` - [reason]
```

## Execution

### Shell Alias Workaround

Many shells alias `cp` to `cp -i` (interactive mode). Always use `\cp` to bypass:

```bash
# ❌ WRONG - Will prompt for confirmation and fail in automation
cp source dest

# ✅ CORRECT - Bypasses alias, forces overwrite
\cp source dest
```

### Parallel Execution

All file copies are independent - execute in parallel:

```bash
# Execute all copies simultaneously
\cp ~/.claude/agents/engineering/new-agent.md toolkit/claude-code-4.5/agents/engineering/ &
\cp ~/.claude/commands/new-command.md toolkit/claude-code-4.5/commands/ &
\cp toolkit/claude-code-4.5/commands/updated.md ~/.claude/commands/ &
wait
```

### Commit Format

Single commit for all TO_REPO changes:

```
chore: sync claude commands and agents

- Add [new-file]: [brief description]
- Update [updated-file]: [what changed]
```

## Quick Diff Commands

```bash
# Find all differences (both directions)
diff -rq ~/.claude/agents/ toolkit/claude-code-4.5/agents/ 2>/dev/null
diff -rq ~/.claude/commands/ toolkit/claude-code-4.5/commands/ 2>/dev/null

# Find files only in ~/.claude (candidates for TO_REPO)
diff -rq ~/.claude/agents/ toolkit/claude-code-4.5/agents/ 2>/dev/null | grep "Only in /Users"

# Find files only in repo (candidates for TO_HOME)
diff -rq ~/.claude/agents/ toolkit/claude-code-4.5/agents/ 2>/dev/null | grep "Only in toolkit"

# Show actual diff for a specific file
diff ~/.claude/agents/engineering/example.md toolkit/claude-code-4.5/agents/engineering/example.md
```

## Safety Checks

- **Never overwrite** without assessment first
- **Check modification times** when both versions exist - sync newer to older
- **Skip binaries** and non-text files
- **Validate** markdown frontmatter before copying agent files
- **Exclude** files matching exclusion categories

## Example Session

```
User: /sync-learnings

Claude: # Sync Assessment: ~/.claude ↔ toolkit/claude-code-4.5

## Summary

| Action | Files | Reason |
|--------|-------|--------|
| **SYNC TO REPO** | 4 files | Useful generic additions |
| **SYNC TO ~/.claude** | 2 files | Repo has newer versions |
| **DON'T SYNC** | Multiple | Project-specific or session data |

## ✅ SYNC TO REPO

### 1. `agents/engineering/focused-repository-analyzer.md` (NEW)
- **Purpose**: Targeted analysis of external repos
- **Assessment**: ✅ Generic utility

[... more files ...]

## ⬇️ SYNC TO ~/.claude

### 1. `commands/research.md`
- **Status**: Repo version is NEWER with External Repository Discovery section
- **Assessment**: ✅ Copy to ~/.claude

Proceed with sync? [Y/n]

User: Y

Claude: Executing sync...

✅ focused-repository-analyzer.md → repo
✅ session-info.md → repo
✅ research.md → ~/.claude

Committed: chore: sync claude commands and agents
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
