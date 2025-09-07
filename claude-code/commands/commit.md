# Commit

You are tasked with creating git commits for the changes made during this session.

## Initial Response

When invoked, respond with:
```
I'll help you create git commits for the changes in this session.

Let me review what was accomplished and prepare appropriate commits.
```

## Process

### Step 1: Pre-Commit Cleanup

Before creating any commits, ALWAYS perform cleanup:

1. **Check for files that should NOT be committed**:
   ```bash
   # Look for env files that might not be ignored
   ls -la .env* env.* *.env
   
   # Check if they're in .gitignore
   cat .gitignore | grep -E "\.env|env\."
   ```
   
   If any `.env` files are not in `.gitignore`, add them:
   ```bash
   echo ".env*" >> .gitignore
   echo "*.env" >> .gitignore
   ```

2. **Remove debug/test files created during development**:
   ```bash
   # Look for common debug/test files
   ls -la test_*.* debug_*.* tmp_*.* temp_*.*
   ```
   
   Remove any files that were created just to assist development:
   ```bash
   rm test_script.js debug_output.txt temp_*.py
   ```

3. **Remove unnecessary documentation**:
   ```bash
   # Check for markdown files created during this session
   git status | grep "\.md"
   ```
   
   Unless explicitly requested by the user, remove:
   - Work tracking documents
   - Temporary notes
   - Auto-generated docs
   
   Keep only:
   - Explicitly requested documentation
   - Essential README updates
   - API documentation if requested

4. **Verify cleanup**:
   ```bash
   git status
   ```
   
   Present to user if cleanup is needed:
   ```
   I found some files that should be cleaned up before committing:
   
   Files to remove:
   - test_oauth.js (debug script)
   - debug_notes.md (work tracking)
   - .env.local (should be in .gitignore)
   
   Shall I clean these up before creating commits?
   ```

### Step 2: Understand What Changed

1. **Review the conversation history**:
   - Understand what was accomplished in this session
   - Identify the purpose and context of changes
   - Note any bug fixes, features, or refactoring done

2. **Check git status** (after cleanup):
   ```bash
   git status
   ```
   - See all modified, added, and deleted files
   - Identify untracked files that need to be added

3. **Review the actual changes**:
   ```bash
   git diff
   git diff --staged
   ```
   - Understand the specific modifications
   - Group related changes together
   - Identify if changes should be one commit or multiple

### Step 3: Plan Your Commits

1. **Determine commit strategy**:
   - Single commit for related changes
   - Multiple commits for distinct features/fixes
   - Atomic commits that each serve a single purpose

2. **Group files logically**:
   - Feature files together
   - Test files with their implementation
   - Configuration changes separately if significant

3. **Draft commit messages**:
   - Use imperative mood ("Add", "Fix", "Update", not "Added", "Fixed")
   - First line: concise summary (50 chars or less ideally)
   - Blank line, then detailed explanation if needed
   - Focus on WHY the change was made, not just what

### Step 4: Present Your Plan

Show the user your commit plan:

```
Based on the changes, I plan to create [N] commit(s):

**Commit 1**: [Type]: [Summary]
Files:
- path/to/file1.js
- path/to/file2.js

Message:
```
feat: add OAuth2 authentication support

- Implement OAuth2 flow with refresh tokens
- Add token storage and validation
- Include error handling for auth failures
```

**Commit 2**: [Type]: [Summary]
Files:
- tests/auth.test.js

Message:
```
test: add comprehensive OAuth2 tests

- Test token refresh flow
- Verify error handling
- Add integration tests for providers
```

Shall I proceed with these commits?
```

### Step 5: Execute Upon Confirmation

1. **Stage files for each commit**:
   ```bash
   # For each commit, add specific files
   git add path/to/file1.js path/to/file2.js
   
   # NEVER use git add -A or git add .
   # Always be specific about what you're committing
   ```

2. **Create the commit**:
   ```bash
   git commit -m "feat: add OAuth2 authentication support

   - Implement OAuth2 flow with refresh tokens
   - Add token storage and validation
   - Include error handling for auth failures"
   ```

3. **Verify the commits**:
   ```bash
   git log --oneline -n 3
   ```
   Show the user the created commits

## Commit Message Format

Follow conventional commits format:

### Types:
- **feat**: New feature
- **fix**: Bug fix
- **docs**: Documentation changes
- **style**: Code style changes (formatting, semicolons, etc)
- **refactor**: Code refactoring without changing functionality
- **perf**: Performance improvements
- **test**: Adding or updating tests
- **build**: Build system or dependencies
- **ci**: CI/CD changes
- **chore**: Maintenance tasks

### Structure:
```
<type>(<scope>): <subject>

<body>

<footer>
```

### Examples:

```bash
# Simple feature
git commit -m "feat: add user profile page"

# Bug fix with detail
git commit -m "fix: resolve race condition in payment processing

The payment webhook could process twice if requests arrived
simultaneously. Added mutex locking to ensure single processing."

# Breaking change
git commit -m "feat!: update API response format

BREAKING CHANGE: API now returns data in 'result' field instead
of root level. Clients need to update response parsing."
```

## Important Rules

### 🚫 Cleanup Rules (MUST DO BEFORE COMMITS)
1. **ALL `.env.*` files MUST be in `.gitignore`**
   - Never commit environment files
   - Add patterns to .gitignore if missing

2. **REMOVE all debug/test scripts created to assist agent**
   - Delete temporary test files
   - Remove debug output files
   - Clean up any helper scripts

3. **NO documentation unless explicitly requested**
   - Delete work tracking documents
   - Remove temporary markdown notes
   - Only keep docs the user specifically asked for

### Commit Rules
1. **NEVER add co-author information or Claude attribution**:
   - No "Generated with Claude" messages
   - No "Co-Authored-By" lines
   - Commits should be authored solely by the user

2. **Write commits as if the user wrote them**:
   - Use the project's commit style if evident
   - Match the tone of existing commits
   - Be professional and concise

3. **Be selective with staging**:
   - Only commit files that are ready
   - Don't include debug code or temporary files
   - Ensure no sensitive information is committed

## Handling Complex Scenarios

### Multiple Features in One Session

If multiple unrelated features were implemented:
```
I notice we worked on several distinct features. I'll create separate commits for:

1. OAuth implementation (5 files)
2. User profile updates (3 files)  
3. Bug fix for payment processing (2 files)

This keeps the git history clean and makes reverting easier if needed.
```

### Large Changes

For extensive changes, consider:
```
This is a large change. I recommend breaking it into logical commits:

1. Core implementation
2. Tests
3. Documentation
4. Configuration changes

This makes code review easier and helps track what changed where.
```

### Work in Progress

If implementation is incomplete:
```
The implementation isn't complete yet. Would you like me to:

1. Commit completed parts with a clear message
2. Create a WIP commit to save progress
3. Wait until the feature is complete

What's your preference?
```

## Verification Steps

After committing, always:

1. **Show the commit log**:
   ```bash
   git log --oneline -5
   ```

2. **Verify nothing was missed**:
   ```bash
   git status
   ```

3. **Check the commit contents** (if requested):
   ```bash
   git show HEAD
   ```

## Best Practices

1. **Atomic commits**: Each commit should work independently
2. **Clear messages**: Future developers should understand why
3. **Group related changes**: But don't mix unrelated changes
4. **Test before committing**: Ensure code works
5. **Review diff carefully**: Check for debug code, comments, secrets

## Quick Reference

```bash
# See what changed
git status
git diff

# Stage specific files
git add src/feature.js tests/feature.test.js

# Commit with message
git commit -m "feat: implement new feature"

# View recent commits
git log --oneline -10

# Amend last commit (if needed)
git commit --amend

# Unstage files (if needed)
git reset HEAD file.js
```

Remember: You have the full context of what was done in this session. Use that knowledge to create meaningful, well-organized commits that tell the story of what was accomplished.