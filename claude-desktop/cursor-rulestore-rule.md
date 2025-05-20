---
description: rule for rules
globs: *-rule.md
alwaysApply: false
---
---
description: Claude Desktop Rules Location
globs: *.md
---
# Claude Desktop Rules Location

Rules for placing and organizing Claude Desktop rule files in the repository.

<rule>
name: claude_rules_location
description: Standards for placing Claude Desktop rule files in the correct directory
filters:
  # Match any .md files
  - type: file_extension
    pattern: "\\.md$"
  # Match files that look like Claude rules
  - type: content
    pattern: "(?s)<rule>.*?</rule>"
  # Match file creation events
    pattern: "file_create"

actions:
  - type: reject
    conditions:
      - pattern: "^(?!\\./\\.claude/rules/.*\\.md$)"
        message: "Claude Desktop rule files (.md) must be placed in the .claude/rules directory"

  - type: suggest
    message: |
      When creating Claude Desktop rules:

      1. Always place rule files in the following location:
         ```
         .claude/rules/
         ├── your-rule-name.md
         └── ...
         ```

      2. Follow the naming convention:
         - Use kebab-case for filenames
         - Always use .md extension
         - Make names descriptive of the rule's purpose

      3. Directory structure:
         ```
         PROJECT_ROOT/
         ├── .claude/
         │   └── rules/
         │       ├── your-rule-name.md
         │       └── ...
         └── ...
         ```

      4. Never place rule files:
         - In the project root
         - In subdirectories outside .claude/rules
         - In any other location

examples:
  - input: |
      # Bad: Rule file in wrong location
      rules/my-rule.md
      my-rule.md
      .rules/my-rule.md

      # Good: Rule file in correct location
      .claude/rules/my-rule.md
    output: "Correctly placed Claude Desktop rule file"

metadata:
  priority: high
  version: 1.0
</rule> 