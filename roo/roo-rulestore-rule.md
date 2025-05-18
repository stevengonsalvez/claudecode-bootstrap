---
description: Rule for Roo rule file locations
globs: "*-rule.md" # Applies to all rule files
alwaysApply: false
---
# Roo Rules Location

Rules for placing and organizing Roo rule files in the repository.

<rule>
name: roo_rules_location
description: Standards for placing Roo rule files in the correct directory
filters:
  # Match .md files with the correct naming pattern
  - type: file_extension
    pattern: "\\-rule\\.md$"
  # Match files that look like rule definitions
  - type: content
    pattern: "(?s)<rule>.*?</rule>"
  # Match file creation events
  - type: event
    pattern: "file_create"

actions:
  - type: reject
    conditions:
      # Ensure files are in .roo/rules/ and end with -rule.md
      - pattern: "^(?!\\.\\/\\.roo\\/rules\\/.*\\-rule\\.md$)"
        message: "Roo rule files (<rulename>-rule.md) must be placed in the .roo/rules/ directory"

  - type: suggest
    message: |
      When creating Roo rules:

      1. Always place rule files in PROJECT_ROOT/.roo/rules/:
         ```
         .roo/rules/
         ├── your-rule-name-rule.md
         ├── another-rule-rule.md
         └── ...
         ```

      2. Follow the naming convention:
         - Use kebab-case for filenames
         - Format should be <rulename>-rule.md
         - Always use .md extension
         - Make names descriptive of the rule's purpose

      3. Directory structure:
         ```
         PROJECT_ROOT/
         ├── .roo/
         │   └── rules/
         │       ├── your-rule-name-rule.md
         │       └── ...
         └── ...
         ```

      4. Never place rule files:
         - In the project root
         - In subdirectories outside .roo/rules
         - In any other location

examples:
  - input: |
      # Bad: Rule file in wrong location
      rules/my-rule-rule.md
      my-rule-rule.md
      .rules/my-rule-rule.md

      # Bad: Incorrect file naming
      .roo/rules/my_rule.md
      .roo/rules/myrule.md
      .roo/rules/my-rule.md

      # Good: Rule file in correct location with correct naming
      .roo/rules/my-rule-rule.md
    output: "Correctly placed Roo rule file with proper naming"

metadata:
  priority: high
  version: 1.0
</rule> 