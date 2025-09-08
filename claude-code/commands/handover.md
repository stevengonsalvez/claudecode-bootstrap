# Handover Command

Use this command to generate a session handover document when transferring work to another team member or continuing work in a new session.

## Usage

```
/handover [optional-notes]
```

## Description

This command generates a comprehensive handover document that includes:

- Current session health status
- Task progress and todos
- Technical context and working files
- Instructions for resuming work
- Any blockers or important notes

## Example

```
/handover Working on authentication refactor, need to complete OAuth integration
```

## Output Location

The handover document MUST be saved to:
- **Primary Location**: `.claude/session/handover-{{TIMESTAMP}}.md`
- **Backup Location**: `./handover-{{TIMESTAMP}}.md` (project root)

## File Naming Convention

Use this format: `handover-YYYY-MM-DD-HH-MM-SS.md`

Example: `handover-2024-01-15-14-30-45.md`

**CRITICAL**: Always obtain the timestamp programmatically:
```bash
# Generate timestamp - NEVER type dates manually
TIMESTAMP=$(date +"%Y-%m-%d-%H-%M-%S")
FILENAME="handover-${TIMESTAMP}.md"
```

## Implementation

1. **ALWAYS** get the current timestamp using `date` command:
   ```bash
   date +"%Y-%m-%d %H:%M:%S"  # For document header
   date +"%Y-%m-%d-%H-%M-%S"  # For filename
   ```
2. Generate handover using `~/.claude/templates/handover-template.md`
3. Replace all `{{VARIABLE}}` placeholders with actual values
4. Save to BOTH locations (primary and backup)
5. Display the full file path to the user for reference
6. Verify the date in the filename matches the date in the document header

The handover document will be saved as a markdown file and can be used to seamlessly continue work in a new session.