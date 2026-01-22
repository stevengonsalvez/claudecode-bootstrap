# /reflect - Self-Improvement Reflection

Analyze the current conversation for corrections, success patterns, and learnings. Extract insights and propose targeted updates to agent files.

**Philosophy**: "Correct once, never again."

## Usage

Invoke `/reflect` with no arguments, then ask in chat what to focus on:

- Standard reflection (default)
- Targeted reflection (agent or file)
- Auto-reflection toggle (on/off)
- Status
- Review low-confidence learnings

## Modes

### Standard Reflection (default)

Scan entire conversation for:
- **High confidence**: Explicit corrections ("never do X", "always check Y")
- **Medium confidence**: Approved approaches (user confirmed it worked)
- **Low confidence**: Observations (patterns that worked, not explicitly validated)
- **New Skills**: Reusable techniques, workarounds, debugging patterns worth preserving

### Targeted Reflection (ask in chat)

Focus analysis on learnings relevant to a specific agent:
Examples of targets:
- `code-reviewer`
- `backend-developer`
- `CLAUDE.md` (global preferences)

### Toggle Auto-Reflection (ask in chat)

Ask the user if they want auto-reflection enabled, disabled, or to view status.

## Implementation

When this command is invoked:

### 1. Determine Intent (no args)

Ask the user which mode to run:
- Standard reflection
- Targeted reflection (which agent/file?)
- Auto-reflection toggle (on/off)
- Status
- Review low-confidence learnings

If the user chooses:
- **on/off**: update `{{HOME_TOOL_DIR}}/session/reflect-state.yaml`
- **status**: read `{{HOME_TOOL_DIR}}/session/reflect-state.yaml` and `{{HOME_TOOL_DIR}}/session/reflect-metrics.yaml`
- **review**: run reflection filtered to low-confidence learnings

### 2. Scan Conversation for Signals

Look for these patterns in the conversation:

**High Confidence (Explicit Corrections)**:
- "never", "don't", "stop doing", "wrong", "not like that"
- "always", "must", "required", "the rule is"
- Frustration: "I already told you", "again?"

**Medium Confidence (Approved Approaches)**:
- "perfect", "exactly", "that's right", "yes, like that"
- User accepted output without modification
- Positive acknowledgment of technique

**Low Confidence (Observations)**:
- Patterns that seemed to work
- Inferred preferences from context
- Edge cases discovered

### 3. Classify and Match to Agent Files or Create Skills

For each signal detected:

1. **Determine category**: Code Style, Architecture, Process, Domain, Tools, **New Skill**
2. **For agent updates**: Match to `{{HOME_TOOL_DIR}}/agents/*/` file, identify section, draft addition
3. **For new skills**: Check quality gates, generate skill using template

**Skill-Worthy Signals** (create new skill instead of updating agent):
- Non-obvious debugging (>10 min investigation, not in docs)
- Misleading error (root cause different from error message)
- Workaround discovered through experimentation
- Configuration insight (differs from standard/documented)
- Reusable pattern (would help in similar future situations)

**Skill Quality Gates** (must pass before creating):
- [ ] Reusable: Will help with future tasks
- [ ] Non-trivial: Requires discovery, not just docs lookup
- [ ] Specific: Can describe exact trigger conditions
- [ ] Verified: Solution actually worked
- [ ] No duplication: Doesn't exist already

### 4. Generate Reflection Report

Produce this exact output format:

```markdown
# Reflection Analysis

## Session Context
- **Date**: [timestamp]
- **Messages Analyzed**: [count]
- **Focus**: [all agents OR specific agent name]

## Signals Detected

| # | Signal | Confidence | Source Quote | Category |
|---|--------|------------|--------------|----------|
| 1 | [extracted learning] | HIGH | "[user's exact words]" | Code Style |
| 2 | [extracted learning] | MEDIUM | "[context]" | Architecture |
| 3 | [skill-worthy pattern] | HIGH | "[context]" | New Skill |

## Proposed Agent Updates

### Change 1: Update code-reviewer

**Target**: `{{HOME_TOOL_DIR}}/agents/engineering/code-reviewer.md`
**Section**: Review Heuristics
**Confidence**: HIGH
**Rationale**: User explicitly stated this rule

```diff
--- a/{{HOME_TOOL_DIR}}/agents/engineering/code-reviewer.md
+++ b/{{HOME_TOOL_DIR}}/agents/engineering/code-reviewer.md
@@ -82,6 +82,7 @@
 ## Review Heuristics

 * **Security**: validate inputs, authn/z flows, encryption, CSRF/XSS/SQLi.
+* **Security**: [new rule from learning]
```

## Proposed New Skills

### Skill 1: [skill-name]

**Quality Gate Check**:
- [x] Reusable: [why]
- [x] Non-trivial: [why]
- [x] Specific: [trigger conditions]
- [x] Verified: [how verified]
- [x] No duplication: [checked against]

**Will create**: `{{TOOL_DIR}}/skills/[skill-name]/SKILL.md`

**Preview**:
```yaml
---
name: [skill-name]
description: |
  [Include exact error messages, symptoms for semantic matching]
  Use when: (1) [condition], (2) [condition]
  Solves: [problem]
---
```

## Conflict Check

- [x] No conflicts with existing rules detected
- OR: Warning - potential conflict with existing rule in [file:line]

## Commit Message

```
reflect: add learnings from session [date]

Agent updates:
- [learning 1 summary]

New skills:
- [skill-name]: [brief description]

Extracted: [N] signals ([H] high, [M] medium, [L] low confidence)
```

## Review Prompt

Apply these changes?
- `Y` - Apply all changes and commit
- `N` - Discard all changes
- `modify` - Let me adjust specific changes
- `1,3` - Apply only agent changes 1 and 3
- `s1` - Apply only skill 1
- `all-skills` - Apply all skills, skip agent updates
```

### 5. Handle User Response

**On `Y` (approve)**:
1. Apply each change using Edit tool
2. Run `git add` on modified files
3. Commit with generated message
4. Update `{{HOME_TOOL_DIR}}/session/learnings.yaml` log
5. Update `{{HOME_TOOL_DIR}}/session/reflect-metrics.yaml`
6. Report success

**On `N` (reject)**:
1. Discard proposed changes
2. Log rejection for analysis
3. Ask if user wants to modify any signals

**On `modify`**:
1. Present each change individually
2. Allow editing the proposed addition
3. Reconfirm before applying

**On selective (e.g., `1,3`)**:
1. Apply only specified changes
2. Log partial acceptance
3. Commit only applied changes

### 6. Update Metrics

After each reflection, update `{{HOME_TOOL_DIR}}/session/reflect-metrics.yaml`:

```yaml
last_reflection: [timestamp]
total_sessions_analyzed: [N]
total_signals_detected: [N]
total_changes_proposed: [N]
total_changes_accepted: [N]
acceptance_rate: [%]
most_updated_agents:
  - agent-name: count
confidence_breakdown:
  high: [N]
  medium: [N]
  low: [N]
```

## Example Session

```
User: /reflect

Claude: # Reflection Analysis

## Session Context
- **Date**: 2026-01-18 10:30:00
- **Messages Analyzed**: 47
- **Focus**: All agents

## Signals Detected

| # | Signal | Confidence | Source Quote | Category |
|---|--------|------------|--------------|----------|
| 1 | Never invent button styles | HIGH | "never come up with button styles on your own" | Code Style |
| 2 | Prefer cursor pagination | MEDIUM | User approved cursor-based approach | Architecture |
| 3 | Use snake_case in Python | HIGH | "we use snake_case for all Python" | Code Style |

## Proposed Changes

### Change 1: Update ui-designer

**Target**: `{{HOME_TOOL_DIR}}/agents/design/ui-designer.md`
**Section**: Heuristics
**Confidence**: HIGH
**Rationale**: Explicit correction from user

[diff...]

### Change 2: Update api-architect

**Target**: `{{HOME_TOOL_DIR}}/agents/engineering/api-architect.md`
**Section**: Design Patterns
**Confidence**: MEDIUM
**Rationale**: User approved this approach for pagination

[diff...]

### Change 3: Update backend-developer

**Target**: `{{HOME_TOOL_DIR}}/agents/universal/backend-developer.md`
**Section**: Code Style
**Confidence**: HIGH
**Rationale**: Explicit style rule from user

[diff...]

## Conflict Check
- [x] No conflicts detected

## Commit Message
```
reflect: add style and pagination learnings

- Use design system tokens, never invent styles
- Prefer cursor-based pagination for large datasets
- Use snake_case for Python identifiers

Extracted: 3 signals (2 high, 1 medium confidence)
```

## Review Prompt
Apply these changes? (Y/N/modify/1,2,3)

User: Y

Claude: Applied 3 changes:
- {{HOME_TOOL_DIR}}/agents/design/ui-designer.md
- {{HOME_TOOL_DIR}}/agents/engineering/api-architect.md
- {{HOME_TOOL_DIR}}/agents/universal/backend-developer.md

Committed: abc123f "reflect: add style and pagination learnings"

Metrics updated:
- Total learnings: 35 -> 38
- Acceptance rate: 92%
- Most updated: code-reviewer (12), backend-developer (9)
```

## Background Execution Mode

When triggered by hooks (PreCompact), reflection runs as a background subagent:

1. Spawns via Task tool with `run_in_background: true`
2. Outputs to project `{{TOOL_DIR}}/reflections/YYYY-MM-DD_HH-MM-SS.md`
3. Updates global index at `{{HOME_TOOL_DIR}}/reflections/index.md`
4. Does NOT block the session
5. Changes are queued for review (not auto-applied)

### Output Locations

**Project-level** (auto-created, versioned with repo):
- `{{TOOL_DIR}}/reflections/YYYY-MM-DD_HH-MM-SS.md` - Full reflection
- `{{TOOL_DIR}}/reflections/index.md` - Project reflection summary
- `{{TOOL_DIR}}/skills/{skill-name}/SKILL.md` - New skills (project-level for review)

**Global** (auto-created):
- `{{HOME_TOOL_DIR}}/reflections/by-project/{project}/` - Cross-project aggregation
- `{{HOME_TOOL_DIR}}/reflections/by-agent/{agent}/learnings.md` - Per-agent learnings
- `{{HOME_TOOL_DIR}}/reflections/index.md` - Global summary

**Note**: Skills are created in PROJECT `{{TOOL_DIR}}/skills/` so you can:
- Review before committing
- Move to global `{{HOME_TOOL_DIR}}/skills/` if broadly applicable
- Keep project-specific if only relevant to this repo

### Reflection Output File Format

```markdown
# Session Reflection: YYYY-MM-DD HH:MM

## Context
- **Project**: [project name]
- **Branch**: [git branch]
- **Messages**: [count]
- **Trigger**: PreCompact / Manual / UserPrompt

## Signals Detected
[table of signals with confidence levels]

## Proposed Changes
[diffs for each proposed change]

## Pending Review
- [ ] Change 1: [description]
- [ ] Change 2: [description]

## Metrics
- Signals: N detected (H high, M medium, L low)
- Auto-triggered: true/false
```

## Integration

### With /handover

If auto-reflection is enabled (`/reflect on`), the PreCompact hook will trigger reflection before handover when context compaction occurs.

### With Session Health

At 🟡 (approaching limit) health status (70%+ context), the UserPromptSubmit hook injects reminders to run `/reflect`.

## File Locations

| File | Purpose |
|------|---------|
| `{{HOME_TOOL_DIR}}/session/reflect-state.yaml` | Toggle state, pending reviews |
| `{{HOME_TOOL_DIR}}/session/learnings.yaml` | Log of all applied learnings |
| `{{HOME_TOOL_DIR}}/session/reflect-metrics.yaml` | Aggregate metrics |
| `{{TOOL_DIR}}/reflections/` | Project reflection outputs |
| `{{TOOL_DIR}}/skills/{name}/SKILL.md` | New skills (project-level) |
| `{{HOME_TOOL_DIR}}/reflections/` | Global reflection index |
| `{{HOME_TOOL_DIR}}/skills/{name}/SKILL.md` | Global skills (moved manually) |

## Safety Notes

- **Human-in-the-loop**: Never applies changes without explicit approval
- **Git versioned**: All changes committed, easy to rollback
- **Incremental**: Only adds to files, never deletes existing rules
- **Conflict aware**: Warns if new rule might contradict existing

## Troubleshooting

**No signals detected**:
- Session may not have had corrections or explicit feedback
- Try `/reflect review` to check pending low-confidence items

**Conflict warning**:
- Review the existing rule cited
- Decide if new rule should override or be adjusted
- Can modify before applying

**Agent file not found**:
- Check agent name spelling
- Use `/reflect status` to see available targets
- May need to create agent file first
