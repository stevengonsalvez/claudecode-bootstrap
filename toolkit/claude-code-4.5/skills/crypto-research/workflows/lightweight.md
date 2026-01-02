# Lightweight Crypto Research Workflow

Fast cryptocurrency research using only haiku-model agents for quick, cost-effective analysis.

## When to Use

- Quick question about cryptocurrency
- Initial exploration before deep dive
- Cost optimization needed
- Time-sensitive quick check

## Parameters

- **TICKER**: Cryptocurrency symbol
  - Examples: BTC, ETH, SOL, ADA, DOT, AVAX
  - Used by: crypto-coin-analyzer-haiku agent

## Agent Group (Haiku Only)

- crypto-market-agent-haiku
- crypto-coin-analyzer-haiku (receives TICKER)
- macro-crypto-correlation-scanner-haiku
- crypto-investment-plays-haiku

**Total: 4 agents** for quick analysis

## Execution Steps

1. **Extract ticker symbol** from user query or use "BTC" as default
2. **Generate timestamp** using `date +"%Y-%m-%d_%H-%M-%S"`
3. **Create output directory**: `outputs/<timestamp>/haiku/`
4. **Launch all 4 haiku agents in parallel**
5. **Write complete output** to designated files
6. **Report completion** with directory path

## Performance

- **Cost**: ~75% less than comprehensive mode
- **Speed**: 3-5x faster (30-60 seconds)
- **Quality**: Good for quick insights
