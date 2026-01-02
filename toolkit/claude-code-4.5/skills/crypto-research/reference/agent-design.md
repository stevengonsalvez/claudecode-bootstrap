# Agent Design Reference

## Agent Types

### Market Agent
- Overall crypto market conditions
- Sector performance
- Sentiment indicators

### Coin Analyzer
- Specific cryptocurrency analysis
- Technical and fundamental analysis
- Receives ticker parameter

### Macro Correlation Scanner
- Correlation with traditional markets
- Impact of macro factors
- Risk sentiment

### Investment Plays
- 3 specific opportunities
- Entry/exit strategies
- Risk assessment

## Model Selection

- **Haiku**: Fast, cost-effective
- **Sonnet**: Balanced, comprehensive
- **Opus**: Deep, highest quality

## Agent Requirements

All agents must:
1. Timestamp their analysis
2. Use 5+ tool invocations for data gathering
3. Follow structured output format
4. Provide complete analysis (no placeholders)

## Parallel Execution

Agents run in parallel using multiple Task invocations in a single message for maximum performance.
