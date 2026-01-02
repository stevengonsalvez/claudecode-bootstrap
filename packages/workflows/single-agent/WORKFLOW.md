---
name: single-agent-workflow
version: 1.0.0
description: Sequential research-plan-implement-validate pipeline for structured development with a single agent
license: Apache-2.0

metadata:
  author: stevengonsalvez
  repository: https://github.com/stevengonsalvez/ai-coder-rules
  category: orchestration
  keywords: [workflow, planning, implementation, validation, research]

type: single-agent
pipeline:
  - command: research
    description: Comprehensive investigation of the problem space
  - command: plan
    description: Create detailed, phased implementation plan
  - command: implement
    description: Execute plan step-by-step with verification
  - command: validate
    description: Verify implementation against specifications

dependencies:
  agents: []
  utilities: []
---

# Single-Agent Workflow

Orchestrate the complete research -> plan -> implement -> validate pipeline for structured development.

## Overview

This workflow guides a single agent through a systematic development process:

1. **Research Phase**: Investigate the problem space, understand requirements, gather context
2. **Planning Phase**: Create detailed implementation plan with phases and success criteria
3. **Implementation Phase**: Execute the plan step-by-step with progress tracking
4. **Validation Phase**: Verify the implementation meets all requirements

## Usage

```
/workflow <task-description>
```

### Workflow Commands

- **`workflow start`** - Begin from research phase
- **`workflow continue`** - Resume from existing research/plan
- **`workflow skip-research`** - Start directly with planning
- **`workflow validate`** - Run validation on completed work
- **`workflow status`** - Show current progress

## Phases

### Phase 1: Research

If no existing research found in `research/` directory:

1. Conduct comprehensive multi-modal research
2. Save findings to `research/YYYY-MM-DD_HH-MM-SS_topic.md`
3. Present summary and key findings
4. Wait for approval to proceed to planning

### Phase 2: Planning

After research is complete:

1. Reference research document findings
2. Create detailed, phased implementation plan
3. Define success criteria for each phase
4. Save to `plans/descriptive_name.md`
5. Get user approval before implementation

### Phase 3: Implementation

After plan is approved:

1. Execute phase by phase
2. Track progress with TodoWrite
3. Run verification after each phase
4. Handle any issues that arise
5. Update checkmarks in plan as phases complete

### Phase 4: Validation

After implementation:

1. Execute all automated checks (tests, linting, build)
2. Verify against plan success criteria
3. Generate validation report
4. Identify any issues requiring manual verification

## Progress Tracking

Throughout the workflow, maintain a master todo list:

```
Master Workflow: [Task Name]
[ ] Research Phase
  [ ] Codebase analysis
  [ ] Documentation review
  [ ] External research (if needed)
  [ ] Synthesize findings
[ ] Planning Phase
  [ ] Review research
  [ ] Draft plan structure
  [ ] Detail each phase
  [ ] Define success criteria
[ ] Implementation Phase
  [ ] Phase 1: [Name]
  [ ] Phase 2: [Name]
  [ ] Phase 3: [Name]
[ ] Validation Phase
  [ ] Run automated tests
  [ ] Verify success criteria
  [ ] Generate report
```

## Best Practices

1. **Always start with research** unless explicitly told to skip
2. **Plans must reference research findings** to ensure alignment
3. **Implementation must follow the plan** (adapt if needed, but document deviations)
4. **Validation is not optional** for production-ready code
5. **Keep the user informed** at each phase transition
6. **Use TodoWrite** to track detailed progress

## Related Commands

This workflow internally calls:
- `/research` - For investigation phase
- `/plan` - For planning phase
- `/implement` - For execution phase
- `/validate` - For verification phase

Each command can also be run independently if needed.
