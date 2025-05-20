---
description: Rule Format Standards
globs: *-rule.md
alwaysApply: true
---
# Rule Format Standards

Standards for creating properly formatted rules.

<rule>
name: rule_format_standard
description: Ensures all rules follow the required format and structure
filters:
  - type: file_extension
    pattern: "\\-rule\\.md$"
  - type: event
    pattern: "(file_create|file_modify)"

actions:
  - type: reject
    conditions:
      - pattern: "^((?!<rule>).)*$"
        message: "Rules must be enclosed in <rule>...</rule> tags"
      - pattern: "<rule>\\s*(?!.*name:)"
        message: "Rules must include a 'name' field"
      - pattern: "<rule>\\s*(?!.*description:)"
        message: "Rules must include a 'description' field"
      - pattern: "<rule>\\s*(?!.*filters:)"
        message: "Rules must include a 'filters' section"
      - pattern: "<rule>\\s*(?!.*actions:)"
        message: "Rules must include an 'actions' section"
      - pattern: "^(?!.*\\-rule\\.md$)"
        message: "Rule filenames must follow the '<rulename>-rule.md' format"

  - type: suggest
    message: |
      When creating rules, follow this structure and naming convention:
      
      1. Name files as `<rulename>-rule.md`
      
      ```
      ---
      description: Brief rule purpose
      globs: Pattern of files this applies to
      alwaysApply: true/false
      ---
      # Title
      
      <rule>
      name: unique_rule_name
      description: Detailed explanation of the rule
      
      filters:
        - type: [file_extension|content|event]
          pattern: "regex_pattern"
      
      actions:
        - type: [reject|suggest]
          conditions:
            - pattern: "regex_pattern"
              message: "Error or suggestion message"
      
      examples:
        - input: |
            Sample input
          output: "Expected result"
      
      metadata:
        priority: [low|medium|high]
        version: x.y
      </rule>
      ```

examples:
  - input: |
      rule.md
    output: "Rejected: Filename must follow '<rulename>-rule.md' format"
  
  - input: |
      formatting-rule.md
    output: "Accepted: Properly formatted filename"

metadata:
  priority: high
  version: 1.0
</rule>