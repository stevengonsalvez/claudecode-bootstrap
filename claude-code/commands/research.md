# Research

You are tasked with conducting comprehensive research across multiple sources - codebase, web, and documentation - by spawning parallel sub-agents and synthesizing their findings.

## Initial Setup

When this command is invoked, respond with:
```
I'm ready to conduct comprehensive research. Please provide your research question or area of interest.

I can research:
- Codebase: Find implementations, patterns, and architecture
- Documentation: Discover existing docs and decisions
- Web: External resources, best practices, and solutions (if requested)

What would you like me to investigate?
```

Then wait for the user's research query.

## Research Process

### Step 1: Read Mentioned Files First

**CRITICAL**: If the user mentions specific files, read them FULLY first:
- Use the Read tool WITHOUT limit/offset parameters
- Read these files yourself in the main context before spawning any sub-tasks
- This ensures you have full context before decomposing the research

### Step 2: Analyze and Decompose

- Break down the query into composable research areas
- Think deeply about underlying patterns, connections, and architectural implications
- Identify specific components, patterns, or concepts to investigate
- Create a research plan using TodoWrite to track all subtasks
- Consider which directories, files, or architectural patterns are relevant

### Step 3: Spawn Parallel Research Tasks

Create multiple Task agents to research different aspects concurrently. Think deeply about the query to determine which types of research are needed.

**Research Types to Consider:**

**A. Codebase Research (always do this):**
```
Task: "Find all files related to [topic]"
- Search for relevant source files, configs, tests
- Identify main implementation files
- Find usage examples and patterns
- Return specific file:line references

Task: "Analyze how [system/feature] works"
- Understand current implementation
- Trace data flow and dependencies
- Identify conventions and patterns
- Return detailed explanations with code references

Task: "Find similar implementations of [pattern]"
- Look for existing examples to model after
- Identify reusable components
- Find test patterns to follow
```

**B. Documentation Research (if relevant):**
```
Task: "Find existing documentation about [topic]"
- Search README files, docs directories
- Look for architecture decision records (ADRs)
- Find API documentation
- Check inline code comments for important notes

Task: "Extract insights from documentation"
- Synthesize key decisions and rationale
- Identify constraints and requirements
- Find historical context
```

**C. Web Research (if explicitly requested or needed):**
```
Task: "Research best practices for [technology/pattern]"
- Find official documentation
- Discover community solutions
- Identify common pitfalls and solutions
- MUST return specific URLs with findings

Task: "Find external resources about [topic]"
- Look for tutorials, guides, examples
- Find relevant Stack Overflow discussions
- Discover blog posts or articles
- MUST include links for reference
```

**D. Test and Quality Research:**
```
Task: "Analyze test coverage for [component]"
- Find existing tests
- Identify testing patterns
- Check for missing test cases
- Return test file locations
```

**Spawning Strategy:**
- Run 3-6 focused tasks in parallel for efficiency
- Each task should have a clear, specific goal
- Provide enough context for agents to be effective
- Request concrete outputs (file paths, code snippets, URLs)

### Step 4: Wait and Synthesize

- **IMPORTANT**: Wait for ALL sub-agent tasks to complete
- Compile all sub-agent results
- Prioritize live codebase findings as primary source of truth
- Connect findings across different components
- Include specific file paths and line numbers for reference
- Highlight patterns, connections, and architectural decisions
- Answer the user's specific questions with concrete evidence

### Step 5: Generate Research Document

Create a document with the following structure:

```markdown
# Research: [User's Question/Topic]

**Date**: [Current date and time]
**Repository**: [Repository name]
**Branch**: [Current branch name]
**Commit**: [Current commit hash]
**Research Type**: [Codebase | Documentation | Web | Comprehensive]

## Research Question
[Original user query]

## Executive Summary
[2-3 sentence high-level answer to the question]

## Key Findings
- [Most important discovery]
- [Second key insight]
- [Third major finding]

## Detailed Findings

### Codebase Analysis
#### [Component/Area 1]
- Current implementation: [file.ext:line]
- How it works: [explanation]
- Dependencies: [what it relies on]
- Usage patterns: [how it's used elsewhere]

#### [Component/Area 2]
...

### Documentation Insights
- [Key documentation found]
- [Important decisions or constraints]
- [Historical context if relevant]

### External Research (if applicable)
- [Best practices from official docs] ([URL])
- [Community solutions] ([URL])
- [Relevant discussions] ([URL])

## Code References
- `path/to/file.py:123` - Main implementation of [feature]
- `another/file.ts:45-67` - Usage example
- `test/file.test.js:89` - Test demonstrating behavior

## Architecture Insights
- **Pattern**: [Pattern name] used for [purpose]
- **Convention**: [Convention observed] across [components]
- **Design Decision**: [Why something was built this way]

## Recommendations
Based on this research:
1. [Actionable recommendation]
2. [Another suggestion]
3. [Potential improvement]

## Open Questions
- [Area needing more investigation]
- [Unresolved question]

## References
- Internal docs: [paths to relevant docs]
- External resources: [URLs to external resources]
- Related research: [paths to other research docs]
```

Save to: `research/YYYY-MM-DD_HH-MM-SS_topic.md`

### Step 6: Add GitHub Permalinks (if applicable)

- Check if on main branch: `git branch --show-current` and `git status`
- If on main/master or pushed, generate GitHub permalinks:
  - Get repo info: `gh repo view --json owner,name`
  - Create permalinks: `https://github.com/{owner}/{repo}/blob/{commit}/{file}#L{line}`
- Replace local file references with permalinks in the document

### Step 7: Present Findings

- Present a concise summary of findings to the user
- Include key file references for easy navigation
- Ask if they have follow-up questions or need clarification

### Step 8: Handle Follow-up Questions

- If the user has follow-up questions, append to the same research document
- Add a new section: `## Follow-up Research [timestamp]`
- Spawn new sub-agents as needed for additional investigation
- Continue updating the document

## Important Notes

- Always use parallel Task agents to maximize efficiency
- Always run fresh codebase research - never rely solely on existing documents
- Focus on finding concrete file paths and line numbers
- Research documents should be self-contained with all necessary context
- Each sub-agent prompt should be specific and focused on read-only operations
- Consider cross-component connections and architectural patterns
- Include temporal context (when the research was conducted)
- Link to GitHub when possible for permanent references
- Keep the main agent focused on synthesis, not deep file reading

## Critical Ordering

1. ALWAYS read mentioned files first before spawning sub-tasks
2. ALWAYS wait for all sub-agents to complete before synthesizing
3. ALWAYS gather metadata before writing the document
4. NEVER write the research document with placeholder values

## Sub-task Best Practices

When spawning research sub-tasks:

1. **Spawn multiple tasks in parallel** for efficiency
2. **Each task should be focused** on a specific area
3. **Provide clear instructions** including:
   - What to search for
   - Which directories to focus on
   - What information to extract
   - Expected output format
4. **Request specific file:line references** in responses
5. **Wait for all tasks to complete** before synthesizing
6. **Verify sub-task results** - spawn follow-up tasks if needed