---
description: Rule for Cline rule file locations
globs: "*-rule.md" # Applies to all rule files
alwaysApply: false
---
# Cline Rules Location

Rules for placing and organizing Cline rule files in the repository.

<rule>
name: cline_rules_location
description: Standards for placing Cline rule files in the correct directory
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
      # Ensure files are in .clinerules/ and end with -rule.md
      - pattern: "^(?!\\.\\/\\.clinerules\\/.*\\-rule\\.md$)"
        message: "Cline rule files (<rulename>-rule.md) must be placed in the .clinerules/ directory"

  - type: suggest
    message: |
      When creating Cline rules:

      1. Always place rule files in PROJECT_ROOT/.clinerules/:
         ```
         .clinerules/
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
         ├── .clinerules/
         │   ├── your-rule-name-rule.md
         │   └── ...
         └── ...
         ```

      4. Never place rule files:
         - In the project root
         - In subdirectories outside .clinerules
         - In any other location

examples:
  - input: |
      # Bad: Rule file in wrong location
      rules/my-rule-rule.md
      my-rule-rule.md
      .rules/my-rule-rule.md

      # Bad: Incorrect file naming
      .clinerules/my_rule.md
      .clinerules/myrule.md
      .clinerules/my-rule.md

      # Good: Rule file in correct location with correct naming
      .clinerules/my-rule-rule.md
    output: "Correctly placed Cline rule file with proper naming"

metadata:
  priority: high
  version: 1.0
</rule> 