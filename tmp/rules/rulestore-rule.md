---
description: rule for rules
globs: *-rule.md
alwaysApply: false
---
# AmazonQ Rules Location

Rules for placing and organizing AmazonQ rule files in the repository.

<rule>
name: amazonq_rules_location
description: Standards for placing AmazonQ rule files in the correct directory
filters:
  # Match .md files with the correct naming pattern
  - type: file_extension
    pattern: "\\-rule\\.md$"
  # Match files that look like AmazonQ rules
  - type: content
    pattern: "(?s)<rule>.*?</rule>"
  # Match file creation events
  - type: event
    pattern: "file_create"

actions:
  - type: reject
    conditions:
      - pattern: "^(?!\\.\\/\\.amazonq\\/rules\\/.*\\-rule\\.md$)"
        message: "AmazonQ rule files (<rulename>-rule.md) must be placed in the .amazonq/rules directory"

  - type: suggest
    message: |
      When creating AmazonQ rules:

      1. Always place rule files in PROJECT_ROOT/.amazonq/rules/:
         ```
         .amazonq/rules/
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
         ├── .amazonq/
         │   └── rules/
         │       ├── your-rule-name-rule.md
         │       └── ...
         └── ...
         ```

      4. Never place rule files:
         - In the project root
         - In subdirectories outside .amazonq/rules
         - In any other location

examples:
  - input: |
      # Bad: Rule file in wrong location
      rules/my-rule-rule.md
      my-rule-rule.md
      .rules/my-rule-rule.md

      # Bad: Incorrect file naming
      .amazonq/rules/my_rule.md
      .amazonq/rules/myrule.md
      .amazonq/rules/my-rule.md

      # Good: Rule file in correct location with correct naming
      .amazonq/rules/my-rule-rule.md
    output: "Correctly placed AmazonQ rule file with proper naming"

metadata:
  priority: high
  version: 1.0
</rule>