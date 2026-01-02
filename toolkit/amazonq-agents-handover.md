# Amazon Q Custom Agents Implementation - Handover Document

## Session Context
- **Date**: 2025-08-05
- **Task**: Implement custom agents for Amazon Q following TDD principles
- **Current State**: Research completed, plan created, ready for implementation

## Background & Research Summary

### Key Differences Discovered

#### Claude Code Agents
- **Format**: Markdown files with YAML frontmatter
- **Structure**: 
  ```yaml
  ---
  name: agent-name
  description: MUST BE USED for...
  tools: Write, Read, Edit, Bash, Grep
  ---
  # Agent system prompt in markdown
  ```
- **Location**: `.claude/agents/` or `~/.claude/agents/`
- **Tools**: High-level tools like Write, Read, Edit, Bash, Grep, WebSearch

#### Amazon Q Agents
- **Format**: JSON configuration files
- **Structure**:
  ```json
  {
    "description": "Agent purpose",
    "tools": ["fs_read", "fs_write", "execute_bash"],
    "allowedTools": ["fs_read"],
    "toolsSettings": {...},
    "mcpServers": {...}
  }
  ```
- **Location**: Should follow pattern `.amazonq/agents/` (inferred from rules location)
- **Tools**: Lower-level tools like fs_read, fs_write, execute_bash, use_aws

### Files Analyzed
1. `/amazonq/mcp.json` - MCP server configurations
2. `/amazonq/amazonq.md` - Amazon Q system prompt (same as Claude)
3. `/amazonq/q-rulestore-rule.md` - Shows Amazon Q file organization pattern
4. `/claude-code/agents/meta/agentmaker.md` - Claude Code agent example

## Implementation Options Evaluated

### Option 1: Direct Converter System (RECOMMENDED)
**Goal**: Transform existing Claude Code agents to Amazon Q format automatically

**Advantages**:
- Reuses existing agent definitions
- Maintains single source of truth initially
- Quick to implement and test
- Clear migration path

### Option 2: Amazon Q Agent Manager
**Goal**: Create dedicated tooling for Amazon Q agents (like agentmaker for Claude)

**Advantages**:
- Purpose-built for Amazon Q
- Can leverage Amazon Q-specific features
- Clean separation of concerns

### Option 3: Unified Agent System
**Goal**: Single definition that generates both formats

**Advantages**:
- True single source of truth
- Consistent agent behavior across tools
- Future-proof for additional AI tools

## Recommended Implementation Plan (Option 1)

### Phase 1: Tool Name Mapping
**First Test** (`test_tool_mapper.py`):
```python
def test_maps_claude_read_to_amazonq_fs_read():
    mapper = ToolMapper()
    assert mapper.map_tool("Read") == "fs_read"

def test_maps_claude_write_to_amazonq_fs_write():
    mapper = ToolMapper()
    assert mapper.map_tool("Write") == "fs_write"

def test_maps_unknown_tool_to_none():
    mapper = ToolMapper()
    assert mapper.map_tool("UnknownTool") is None
```

**Implementation Notes**:
- Start with simple dictionary mapping
- Handle case sensitivity
- Return None for unmapped tools
- Consider warning logs for unmapped tools

**Tool Mapping Reference**:
```
Claude Code → Amazon Q
Read        → fs_read
Write       → fs_write
Edit        → fs_write (with special handling)
Bash        → execute_bash
Grep        → execute_bash (grep command)
LS          → execute_bash (ls command)
WebSearch   → (no direct mapping, skip or use MCP)
WebFetch    → (no direct mapping, skip or use MCP)
Task        → (no equivalent)
```

### Phase 2: YAML Frontmatter Parser
**Test Cases**:
- Parse valid YAML frontmatter
- Handle missing frontmatter
- Handle malformed YAML
- Extract name, description, tools

### Phase 3: JSON Generator
**Test Cases**:
- Generate minimal valid JSON
- Include all required fields
- Handle optional fields (toolsSettings, mcpServers)
- Validate against Amazon Q schema

### Phase 4: File Operations
**Test Cases**:
- Create .amazonq/agents/ directory if missing
- Write JSON files with correct naming
- Handle file conflicts
- Preserve existing files

### Phase 5: Batch Converter
**Test Cases**:
- Convert single agent
- Convert all agents in directory
- Generate conversion report
- Handle errors gracefully

## Directory Structure
```
amazonq-agent-converter/
├── tests/
│   ├── __init__.py
│   ├── test_tool_mapper.py
│   ├── test_yaml_parser.py
│   ├── test_json_generator.py
│   ├── test_file_operations.py
│   └── test_converter.py
├── src/
│   ├── __init__.py
│   ├── tool_mapper.py
│   ├── yaml_parser.py
│   ├── json_generator.py
│   ├── file_operations.py
│   └── converter.py
├── README.md
└── pyproject.toml
```

## Next Session Tasks

### Immediate Actions
1. Create project directory structure
2. Initialize Python project with uv
3. Write first failing test for tool mapper
4. Implement minimal tool mapper to pass test
5. Commit: "feat: add tool name mapping for Claude to Amazon Q"

### TDD Cycle Reminders
- **RED**: Write test first, see it fail
- **GREEN**: Write minimal code to pass
- **REFACTOR**: Improve code if needed
- **COMMIT**: Save working state immediately

### Testing Strategy
- Use pytest for test runner
- Aim for 100% behavior coverage
- Test edge cases and error conditions
- Use factory pattern for test data

## Technical Decisions

### Language: Python
- Matches existing hook implementations
- Good YAML/JSON support
- Easy testing with pytest

### Dependencies
- `pyyaml` - YAML parsing
- `pytest` - Testing framework
- `pathlib` - File operations
- No other dependencies initially

### Error Handling
- Fail gracefully with helpful messages
- Log warnings for unmapped tools
- Create backup before overwriting

## Open Questions for Next Session
1. Should we support custom tool mappings via config?
2. How to handle tools with no Amazon Q equivalent?
3. Should converter be bidirectional (Q→Claude)?
4. Integration with create-rule.js workflow?

## Session Metrics
- Research completed: ✅
- Plan created: ✅
- TDD approach defined: ✅
- Ready for implementation: ✅

## Handover Notes
The next session should start by:
1. Creating the project structure
2. Writing the first test
3. Following TDD strictly
4. Committing after each GREEN phase

Remember: No production code without a failing test first!