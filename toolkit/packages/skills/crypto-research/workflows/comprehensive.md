# Comprehensive Crypto Research Workflow

This workflow executes comprehensive cryptocurrency research by orchestrating 12 specialized agents across 3 model types (haiku, sonnet, opus) in parallel.

## When to Use

- User needs deep, multi-perspective analysis
- Investment decision requires thorough research
- Comparing multiple viewpoints on same cryptocurrency
- Maximum confidence in analysis needed

## Parameters

- **TICKER**: Cryptocurrency symbol (default: "BTC")
  - Examples: BTC, ETH, SOL, ADA, DOT, AVAX, MATIC, LINK
  - Used by: coin-analyzer agents

## Agent Groups

### Market Data Agents (3 models)
- crypto-market-agent-haiku
- crypto-market-agent-sonnet
- crypto-market-agent-opus

### Coin Analysis Agents (3 models)
- crypto-coin-analyzer-haiku (receives TICKER)
- crypto-coin-analyzer-sonnet (receives TICKER)
- crypto-coin-analyzer-opus (receives TICKER)

### Macro Correlation Agents (3 models)
- macro-crypto-correlation-scanner-haiku
- macro-crypto-correlation-scanner-sonnet
- macro-crypto-correlation-scanner-opus

### Investment Plays Agents (3 models)
- crypto-investment-plays-haiku
- crypto-investment-plays-sonnet
- crypto-investment-plays-opus

**Total: 12 agents** providing diverse perspectives

## Execution Steps

1. **Extract ticker symbol** from user query or use "BTC" as default
2. **Generate timestamp** using `date +"%Y-%m-%d_%H-%M-%S"`
3. **Create output directory structure**:
   ```bash
   outputs/<timestamp>/
   ├── crypto_market/
   ├── crypto_analysis/
   ├── crypto_macro/
   └── crypto_plays/
   ```
4. **Launch all 12 agents in parallel** using Task tool
5. **Write each agent's complete output** to its designated file
6. **Report completion** with output directory path and success count

## Critical Requirements

### Complete Output Preservation
**CRITICAL**: Write each agent's complete response directly to its file with:
- NO modifications
- NO summarization
- NO changes whatsoever

### Parallel Execution
All agents must run in parallel using a single message with multiple Task invocations for maximum performance.

## Success Criteria

- All 12 agents execute successfully
- Output directory created with timestamp
- All subdirectories exist
- Each agent has corresponding output file
- Completion report shows success count
