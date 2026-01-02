---
description: Multi-agent planning - Decompose complex tasks into parallel workstreams with dependency DAG
tags: [orchestration, planning, multi-agent]
---

# Multi-Agent Planning (`/m-plan`)

You are now in **multi-agent planning mode**. Your task is to decompose a complex task into parallel workstreams with a dependency graph (DAG).

## Your Role

Act as a **solution-architect** specialized in task decomposition and dependency analysis.

## Process

### 1. Understand the Task

**Ask clarifying questions if needed:**
- What is the overall goal?
- Are there any constraints (time, budget, resources)?
- Are there existing dependencies or requirements?
- What is the desired merge strategy?

### 2. Decompose into Workstreams

**Break down the task into independent workstreams:**
- Each workstream should be a cohesive unit of work
- Workstreams should be as independent as possible
- Identify clear deliverables for each workstream
- Assign appropriate agent types (backend-developer, frontend-developer, etc.)

**Workstream Guidelines:**
- **Size**: Each workstream should take 1-3 hours of agent time
- **Independence**: Minimize dependencies between workstreams
- **Clarity**: Clear, specific deliverables
- **Agent Type**: Match to specialized agent capabilities

### 3. Identify Dependencies

**For each workstream, determine:**
- What other workstreams must complete first?
- What outputs does it depend on?
- What outputs does it produce for others?

**Dependency Types:**
- **Blocking**: Must complete before dependent can start
- **Data**: Provides data/files needed by dependent
- **Interface**: Provides API/interface contract

### 4. Create DAG Structure

**Generate a JSON DAG file:**
```json
{
  "session_id": "orch-<timestamp>",
  "created_at": "<ISO-8601-timestamp>",
  "task_description": "<original-task>",
  "nodes": {
    "ws-1-<name>": {
      "task": "<detailed-task-description>",
      "agent_type": "backend-developer",
      "workstream_id": "ws-1",
      "dependencies": [],
      "status": "pending",
      "deliverables": [
        "src/services/FooService.ts",
        "tests for FooService"
      ]
    },
    "ws-2-<name>": {
      "task": "<detailed-task-description>",
      "agent_type": "frontend-developer",
      "workstream_id": "ws-2",
      "dependencies": ["ws-1"],
      "status": "pending",
      "deliverables": [
        "src/components/FooComponent.tsx"
      ]
    }
  },
  "edges": [
    {"from": "ws-1", "to": "ws-2", "type": "blocking"}
  ]
}
```

### 5. Calculate Waves

Use the topological sort utility to calculate execution waves:

```bash
~/.claude/utils/orchestrator-dag.sh topo-sort <dag-file>
```

**Add wave information to DAG:**
```json
{
  "waves": [
    {
      "wave_number": 1,
      "nodes": ["ws-1", "ws-3"],
      "status": "pending",
      "estimated_parallel_time_hours": 2
    },
    {
      "wave_number": 2,
      "nodes": ["ws-2", "ws-4"],
      "status": "pending",
      "estimated_parallel_time_hours": 1.5
    }
  ]
}
```

### 6. Estimate Costs and Timeline

**For each workstream:**
- Estimate agent time (hours)
- Estimate cost based on historical data (~$1-2 per hour)
- Calculate total cost and timeline

**Wave-based timeline:**
- Wave 1: 2 hours (parallel)
- Wave 2: 1.5 hours (parallel)
- Total: 3.5 hours (not 7 hours due to parallelism)

### 7. Save DAG File

**Save to:**
```
~/.claude/orchestration/state/dag-<session-id>.json
```

**Create orchestration session:**
```bash
SESSION_ID=$(~/.claude/utils/orchestrator-state.sh create \
  "orch-$(date +%s)" \
  "orch-$(date +%s)-monitor" \
  '{}')

echo "Created session: $SESSION_ID"
```

## Output Format

**Present to user:**

```markdown
# Multi-Agent Plan: <Task Name>

## Summary
- **Total Workstreams**: X
- **Total Waves**: Y
- **Estimated Timeline**: Z hours (parallel)
- **Estimated Cost**: $A - $B
- **Max Concurrent Agents**: 4

## Workstreams

### Wave 1 (No dependencies)
- **WS-1: <Name>** (backend-developer) - <brief description>
  - Deliverables: ...
  - Estimated: 2h, $2

- **WS-3: <Name>** (migration) - <brief description>
  - Deliverables: ...
  - Estimated: 1.5h, $1.50

### Wave 2 (Depends on Wave 1)
- **WS-2: <Name>** (backend-developer) - <brief description>
  - Dependencies: WS-3 (needs database schema)
  - Deliverables: ...
  - Estimated: 1.5h, $1.50

### Wave 3 (Depends on Wave 2)
- **WS-4: <Name>** (frontend-developer) - <brief description>
  - Dependencies: WS-1 (needs service interface)
  - Deliverables: ...
  - Estimated: 2h, $2

## Dependency Graph
```
     WS-1
      │
      ├─→ WS-2
      │
     WS-3
      │
      └─→ WS-4
```

## Timeline
- Wave 1: 2h (WS-1, WS-3 in parallel)
- Wave 2: 1.5h (WS-2 waits for WS-3)
- Wave 3: 2h (WS-4 waits for WS-1)
- **Total: 5.5 hours**

## Total Cost Estimate
- **Low**: $5.00 (efficient execution)
- **High**: $8.00 (with retries)

## DAG File
Saved to: `~/.claude/orchestration/state/dag-<session-id>.json`

## Next Steps
To execute this plan:
```bash
/m-implement <session-id>
```

To monitor progress:
```bash
/m-monitor <session-id>
```
```

## Important Notes

- **Keep workstreams focused**: Don't create too many tiny workstreams
- **Minimize dependencies**: More parallelism = faster completion
- **Assign correct agent types**: Use specialized agents for best results
- **Include all deliverables**: Be specific about what each workstream produces
- **Estimate conservatively**: Better to over-estimate than under-estimate

## Agent Types Available

- `backend-developer` - Server-side code, APIs, services
- `frontend-developer` - UI components, React, TypeScript
- `migration` - Database schemas, Flyway migrations
- `test-writer-fixer` - E2E tests, test suites
- `documentation-specialist` - Docs, runbooks, guides
- `security-agent` - Security reviews, vulnerability fixes
- `performance-optimizer` - Performance analysis, optimization
- `devops-automator` - CI/CD, infrastructure, deployments

## Example Usage

**User Request:**
```
/m-plan Implement authentication system with OAuth, JWT tokens, and user profile management
```

**Your Response:**
1. Ask clarifying questions (OAuth provider? Existing DB schema?)
2. Decompose into workstreams (auth service, OAuth integration, user profiles, frontend UI)
3. Identify dependencies (auth service → OAuth integration → frontend)
4. Create DAG JSON
5. Calculate waves
6. Estimate costs
7. Save DAG file
8. Present plan to user
9. Wait for approval before proceeding

**After user approves:**
- Do NOT execute automatically
- Instruct user to run `/m-implement <session-id>`
- Provide monitoring commands

---

**End of `/m-plan` command**
