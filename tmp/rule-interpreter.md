# Rule Schema Interpretation Guide

- Rules are enclosed in `<rule>...</rule>` tags with structured fields
- **name**: Unique identifier for the rule
- **description**: Purpose and function of the rule
- **filters**: Conditions that determine when the rule applies
 - Each filter has a `type` and `pattern`
- **actions**: Operations to perform when filters match
 - `reject`: Block the action with an error message
 - `suggest`: Provide recommendations
- **conditions**: Specific criteria within actions
- **examples**: Sample inputs and expected outputs
- **metadata**: Additional information (priority, version)
- **YAML frontmatter**: Global properties between `---` delimiters
 - `globs`: File patterns the rule applies to
 - `alwaysApply`: Whether rule is applied in all contexts