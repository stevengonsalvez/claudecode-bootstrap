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
- **Primary Location**: `{{TOOL_DIR}}/session/handover-{{TIMESTAMP}}.md`
- **Backup Location**: `./handover-{{TIMESTAMP}}.md` (project root)

## File Naming Convention

Use this format: `handover-YYYY-MM-DD-HH-MM-SS.md`

Example: `handover-2024-01-15-14-30-45.md`

## Implementation

1. Generate handover using `{{TOOL_DIR}}/templates/handover-template.md`
2. Replace all `{{VARIABLE}}` placeholders with actual values
3. Save to the specified location
4. Display the full file path to the user for reference

The handover document will be saved as a markdown file and can be used to seamlessly continue work in a new session.