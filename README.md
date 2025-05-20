# AI Coder Rules CLI

## CLI Usage

This project provides a CLI tool to help you set up and manage rule files for various AI coding tools (AmazonQ, Cline, Roo, Cursor, Claude Desktop, etc.).

### How to Use

1. **Install dependencies:**
   ```sh
   npm install
   ```
2. **Run the CLI:**
   ```sh
   ./create-rule.js
   ```
3. **Follow the prompts:**
   - Select the tool (e.g., amazonq, cline, roo, cursor, claude)
   - Enter the target project folder
   - Select any additional general rules you want to include

The CLI will:
- Copy the tool's own rulestore rule, `rule-interpreter-rule.md`, and `rulestyle-rule.md` to the correct rules directory in your target project.
- Prompt you to select any additional general rules from the `general-rules/` folder.
- Generate a `rule-registry.json` in the rules directory, describing all copied rules and their metadata.

---

## Why This Rule File Structure?

While plain markdown can be used for rules, this project adopts a more structured approach:

- **XML-like <rule> tags** clearly partition each rule, making it easy for both humans and LLMs to identify the start and end of a rule, even if multiple rules exist in a single file.
- **Schematic, objective structure** (with required fields like name, description, filters, actions, etc.) ensures every rule is concrete, machine-readable, and less prone to ambiguity or accidental omission.
- **Frontmatter** provides a consistent place for metadata, making it easy to parse and index rules.
- **Reduced verbosity and ambiguity:** Markdown alone can become verbose and inconsistent, especially as rules grow in complexity. The enforced structure keeps rules concise, focused, and easy to interpret.
- **Better for LLMs and automation:** The explicit structure and partitioning make it easier for LLMs and agentic tools to reliably extract, interpret, and apply rules, compared to freeform markdown which may require complex parsing and is more error-prone.

This approach balances human readability with machine-actionability, ensuring rules are both easy to write and robust for automated enforcement.

---

## Rule Structure

Rules in this project follow a standardized format to ensure consistency and compatibility across tools and agentic IDEs.

### Key Rule Files

- **@rulestyle-rule.md**: Defines the required structure and formatting standards for all rule files. Ensures every rule includes a name, description, filters, actions, and (optionally) examples and metadata.
- **@rule-interpreter-rule.md**: Provides a guide for interpreting the rule schema format, explaining each field and how rules are structured.
- **@rulestore-rule.md**: Each tool (e.g., AmazonQ, Cline, Roo, Cursor, Claude Desktop) has its own rulestore rule, which defines where rule files for that tool should be placed and how they should be named.
- **@rule-registry.json**: An auto-generated registry file that lists all rules present in the rules directory, including their paths, globs, and alwaysApply status (parsed from each rule's frontmatter).

### Example Rule File Structure

```markdown
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

---

## The Rules Registry

The `rule-registry.json` is generated automatically by the CLI. It contains an entry for each rule file in the rules directory, with metadata extracted from the rule's frontmatter. Example:

```json
{
    "rulestore-rule": {
        "path": ".amazonq/rules/rulestore-rule.md",
        "globs": ["*-rule.md"],
        "alwaysApply": true
    },
    "rule-interpreter-rule": {
        "path": ".amazonq/rules/rule-interpreter-rule.md",
        "globs": ["*-rule.md"],
        "alwaysApply": true
    },
    ...
}
```

### Why a Rules Registry?

Some agentic IDEs (like Cursor, AmazonQ, etc.) can parse rule files directly using their frontmatter. However, other tools (such as Claude Desktop, Goose, and similar LLM-based tools) cannot natively parse frontmatter or discover rules automatically.

The `rule-registry.json` provides a machine-readable index of all rules, making it easy for these tools to:
- Load all available rules into context
- Reference the correct globs and application logic
- Prompt the agent to check the relevant rule(s) on every file edit or action

This ensures consistent rule enforcement and discoverability, regardless of the capabilities of the underlying tool or IDE.

---

## Summary

- Use the CLI to quickly scaffold and manage rules for your AI coding tools.
- All rules follow a strict, documented structure for maximum compatibility.
- The rules registry bridges the gap for tools that can't parse frontmatter, ensuring all rules are discoverable and actionable. 