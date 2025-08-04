Analyze the provided spec.md or prd.md file and create a hierarchical breakdown of GitHub issues: $ARGUMENTS

Follow these steps:

1. **Read the specification file**:
   - Look for `spec.md` (from brainstorm command output)
   - Or read the provided `prd.md` (product requirements document)
   - If file path is provided in $ARGUMENTS, use that specific file

2. **Identify and structure the hierarchy**:
   - **Epics**: High-level business goals or major product areas
   - **Features**: Specific functionality within each epic
   - **Subtasks**: Granular, implementable development tasks

3. **Create properly formatted GitHub issues** with:
   - Clear, descriptive titles using convention: `[EPIC/FEATURE/SUBTASK] Description`
   - Detailed descriptions with acceptance criteria
   - Appropriate labels (epic, feature, subtask, priority levels)
   - Dependencies and relationships between issues
   - Estimated effort/story points where applicable
   - Definition of done for each item

4. **Generate the output**:
   - Create issue templates ready for GitHub creation using `gh issue create`
   - Include dependency mapping between issues
   - Provide recommended creation order
   - Save the breakdown as `github-issues-plan.md`

5. **Optionally create the issues**:
   - Ask if they want to create the issues in GitHub immediately
   - If yes, use `gh issue create` for each issue in dependency order
   - Link related issues using GitHub's linking syntax

The goal is to transform high-level specifications into actionable, well-structured GitHub issues that follow project management best practices.