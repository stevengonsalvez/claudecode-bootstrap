---
description: rule for rules
globs: 
alwaysApply: false
---
---
description: Cursor Rules Location
globs: *.md
---
# Cursor Rules Location

Rules for placing and organizing Cursor rule files in the repository.

<rule>
name: cursor_rules_location
description: Standards for placing Cursor rule files in the correct directory
filters:
  # Match any .md files
  - type: file_extension
    pattern: "\\.md$"
  # Match files that look like Cursor rules
  - type: content
    pattern: "(?s)<rule>.*?</rule>"
  # Match file creation events
  - type: event
    pattern: "file_create"

actions:
  - type: reject
    conditions:
      - pattern: "^(?!\\.\\/\\.cursor\\/rules\\/.*\\.md$)"
        message: "Cursor rule files (.md) must be placed in the .cursor/rules directory"

  - type: suggest
    message: |
      When creating Cursor rules:

      1. Always place rule files in PROJECT_ROOT/.cursor/rules/:
         ```
         .cursor/rules/
         ├── your-rule-name.md
         ├── another-rule.md
         └── ...
         ```

      2. Follow the naming convention:
         - Use kebab-case for filenames
         - Always use .md extension
         - Make names descriptive of the rule's purpose

      3. Directory structure:
         ```
         PROJECT_ROOT/
         ├── .cursor/
         │   └── rules/
         │       ├── your-rule-name.md
         │       └── ...
         └── ...
         ```

      4. Never place rule files:
         - In the project root
         - In subdirectories outside .cursor/rules
         - In any other location

examples:
  - input: |
      # Bad: Rule file in wrong location
      rules/my-rule.md
      my-rule.md
      .rules/my-rule.md

      # Good: Rule file in correct location
      .cursor/rules/my-rule.md
    output: "Correctly placed Cursor rule file"

metadata:
  priority: high
  version: 1.0
</rule>
