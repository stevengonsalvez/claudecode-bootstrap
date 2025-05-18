---
description: Guide for interpreting the rule schema format.
globs:
  - "*-rule.md"
  - ".*-rule.md"
  - "*-rule.mdc"
  - ".*-rule.mdc"
alwaysApply: true
---
# Rule Schema Interpretation Guide

<rule>
name: rule_schema_interpretation_guide
description: Provides a guide on how to interpret the structure and fields of a rule file.

filters:
  - type: file_extension
    pattern: "\.(md|mdc)$" # Applies when viewing any markdown rule file
  - type: content_contains # Could also be triggered if user asks about rule structure
    pattern: "(what is the rule format|how are rules structured|explain rule schema)"

actions:
  - type: suggest
    message: |
      Rules are defined with the following structure and fields:

      - **YAML Frontmatter**: Located at the very beginning of the file, enclosed by `---` delimiters.
        - `description`: A brief summary of the rule's purpose.
        - `globs`: A list of file patterns (e.g., `*.js`, `src/**/*.ts`) that the rule should be applied to.
        - `alwaysApply`: A boolean (`true` or `false`) indicating if the rule should be considered in all contexts, regardless of specific triggers.

      - **Main Title**: Usually an H1 heading (e.g., `# My Awesome Rule`) that provides a human-readable title for the rule.

      - **`<rule>...</rule>` Block**: The core definition of the rule, enclosed in XML-like tags.
        - `name`: (Required) A unique, machine-readable identifier for the rule (e.g., `enforce_strict_equality`).
        - `description`: (Required) A more detailed explanation of what the rule does, its purpose, and its function.
        - `filters`: (Required) A list of conditions that determine when the rule becomes active or applicable. Each filter typically has:
          - `type`: The kind of filter (e.g., `file_extension`, `content_matches`, `event_type`).
          - `pattern`: The specific value or regex pattern to match for the filter type (e.g., `\.ts$`, `console\.log`).
        - `actions`: (Required) A list of operations to perform if all filters match. Each action has a `type` and other relevant fields:
          - `type: reject`: Blocks the current operation or code if conditions are met.
            - `message`: The error message to display.
            - `conditions` (optional): Further specific criteria within this action.
          - `type: suggest`: Provides recommendations, code snippets, or informational messages.
            - `message`: The suggestion to display.
            - `code_snippet` (optional): A piece of code to suggest.
            - `conditions` (optional): Further specific criteria within this action.
        - `examples` (optional): A list of sample inputs and their expected outputs or behaviors under this rule. This helps in understanding the rule's impact.
          - `input`: Description or sample of input.
          - `output`: Description or sample of the expected result.
        - `metadata` (optional): Additional, non-functional information about the rule.
          - `priority`: (e.g., `low`, `medium`, `high`) Indicates the rule's importance.
          - `version`: The version of the rule definition (e.g., `1.0`, `1.0.1`).

examples:
  - input: |
      A user asking: "How do I understand the structure of these .md rule files?"
    output: |
      The content of this rule (rule_schema_interpretation_guide) would be presented as a suggestion.

metadata:
  priority: high
  version: 1.0
</rule>