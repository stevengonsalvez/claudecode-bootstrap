# AI Coder Rules CLI

## CLI Usage

This project provides a CLI tool to help you set up and manage rule files for various AI coding tools (AmazonQ, Cline, Roo, Cursor, Claude Desktop, etc.).

### How to Use

1. **Install dependencies:**
   ```sh
   npm install
   ```

2. **Run the CLI (Interactive - Recommended):**
   ```sh
   node create-rule.js
   ```
   Then select your tool from the dropdown menu.

3. **Or run non-interactively (for automation):**
   ```sh
   node create-rule.js --tool=gemini
   node create-rule.js --tool=amazonq
   node create-rule.js --tool=claude-code
   ```

4. **Follow the prompts:**
   - For project-specific tools, enter the target project folder
   - Select any additional general rules you want to include

### What the CLI Does

**For claude-code (Home Directory Installation):**
- Copies complete tool setup to your home directory (`~/.claude`)
- No project folder required - installs globally for the tool

**For gemini/amazonq (Project-Specific Installation):**
- Prompts for target project folder
- Creates workspace-relative directories for tool configuration
- Generates a `rule-registry.json` describing all copied rules and their metadata

**For other tools (Project-Specific Installation):**
- Copies the tool's rulestore rule, `rule-interpreter-rule.md`, and `rulestyle-rule.md` to your project's rules directory
- Prompts for target project folder
- Generates a `rule-registry.json` describing all copied rules and their metadata

### Tool-Specific Behavior

**claude-code**: 
- Copies entire `claude-code/` directory to `~/.claude/`
- Includes: commands, guides, templates, docs, and main CLAUDE.md
- Status line uses Bun runtime for ultra-fast JS execution with built-in caching

**gemini**: 
- Copies all shared content from `claude-code/` to `PROJECT/.gemini/`
- Adds tool-specific `GEMINI.md` file
- Result: Same functionality as claude-code but project-specific
- Aligns with Gemini CLI's workspace-relative configuration

**amazonq**:
- Copies rule files to `PROJECT/.amazonq/rules/` (follows AmazonQ's rules pattern)
- Adds `AmazonQ.md` to project root
- Includes: `q-rulestore-rule.md`, `rule-interpreter-rule.md`, `rulestyle-rule.md`
- Follows AmazonQ's workspace-relative `.amazonq/rules/**/*.md` pattern

**Other tools**: Project-specific installation to your chosen directory

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

---

## Spec-Driven Development (SDD)

Use the Spec Kit workflow to drive a spec → plan → tasks process via slash commands (Claude Code) or manual scripts.

Quickstart (simple):
- Ensure Git and bash are available (macOS/Linux/WSL). Initialize a repo in your project if needed.
- Install SDD assets into a project (clones Spec Kit automatically to a temp folder):
  - `node create-rule.js --sdd --targetFolder=<project>`
  - Optional: set `SPEC_KIT_REPO` (and `SPEC_KIT_REF`) to point to a fork/branch.
- Claude Code users: open the project and run `/specify "Your feature"`. Commands are installed at `.claude/commands`.
- Or run manually:
  - `bash scripts/create-new-feature.sh --json "Your feature"`
  - `bash scripts/setup-plan.sh --json` (must be on `^[0-9]{3}-` feature branch)
  - `bash scripts/check-task-prerequisites.sh --json`

Artifacts are created under `specs/<feature-branch>/`:
- `spec.md`, `plan.md`, `research.md`, `data-model.md`, `contracts/`, `quickstart.md`, and `tasks.md`.

Troubleshooting:
- Not on feature branch: `/plan` and `/tasks` require a `^[0-9]{3}-` branch (the scripts will error clearly).
- Scripts not found: Ensure you ran `--sdd` and that `scripts/` exists in your project.
- Templates missing: Re-run `--sdd`. Existing files with different content are backed up as `.bak`.
- JSON outputs: All SDD scripts support `--json` and print stable JSON with key paths for automation.
