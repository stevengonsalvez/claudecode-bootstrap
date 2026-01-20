# Refactor claude-code-4.5 to use packages and finish migration

## Goal
Make claude-code-4.5 consume the shared packages structure instead of duplicating content, and complete the migration of remaining assets into packages.

## Scope
- Replace remaining hardcoded "~/.claude" / ".claude" paths in package sources with {{HOME_TOOL_DIR}} / {{TOOL_DIR}} where appropriate.
- Update claude-code-4.5 build/install flow to pull from `toolkit/packages` instead of its own copies (skills/commands/templates/reflections first, then agents/hooks/utils/workflows/output-styles/orchestration).
- Ensure template substitution runs for non-md text files used by tools (e.g., .sh/.py/.js).
- Keep claude-code-4.5 as a thin composition layer over packages.

## Acceptance Criteria
- claude-code-4.5 installs from packages without duplicating shared content.
- All paths resolve correctly for both project-local and global installs.
- Tests updated/added to cover package-backed installation.
