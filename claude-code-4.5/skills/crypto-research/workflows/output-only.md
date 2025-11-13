# Output-Only Crypto Research Workflow

Silent cryptocurrency research writing results to files without interactive output. Ideal for automation and background research.

## When to Use

- Automated/scheduled research
- Background analysis
- Batch processing
- Silent monitoring systems

## Parameters

- **TICKER**: Cryptocurrency symbol
- **MODE**: "comprehensive" or "lightweight" (default: lightweight)

## Execution

1. Parse parameters or use defaults
2. Generate timestamp
3. Create output directory
4. Launch agents silently
5. Write outputs to files
6. Return only: Directory path and agent count

## Output Response

Minimal response:
```
Research complete.
Location: outputs/2025-01-08_14-30-45/
Agents: 4/4 successful
```

No agent output displayed.
