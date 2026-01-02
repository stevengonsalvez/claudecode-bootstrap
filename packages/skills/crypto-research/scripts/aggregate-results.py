#!/usr/bin/env python3
"""
ABOUTME: Aggregates crypto research results from multiple agent outputs
"""

import json
import sys
from pathlib import Path
from datetime import datetime

def count_agent_outputs(base_dir: Path) -> dict:
    """Count agent outputs in each category."""
    counts = {
        "market": len(list((base_dir / "crypto_market").glob("*.md"))) if (base_dir / "crypto_market").exists() else 0,
        "analysis": len(list((base_dir / "crypto_analysis").glob("*.md"))) if (base_dir / "crypto_analysis").exists() else 0,
        "macro": len(list((base_dir / "crypto_macro").glob("*.md"))) if (base_dir / "crypto_macro").exists() else 0,
        "plays": len(list((base_dir / "crypto_plays").glob("*.md"))) if (base_dir / "crypto_plays").exists() else 0,
    }
    return counts

def generate_summary(base_dir: Path) -> str:
    """Generate research summary."""
    counts = count_agent_outputs(base_dir)
    total = sum(counts.values())
    
    summary = f"""# Crypto Research Summary

**Location**: {base_dir}
**Generated**: {datetime.now().isoformat()}
**Total Agents**: {total}

## Agent Counts
- Market Analysis: {counts['market']}
- Coin Analysis: {counts['analysis']}
- Macro Correlation: {counts['macro']}
- Investment Plays: {counts['plays']}
"""
    return summary

def main():
    if len(sys.argv) < 2:
        print("Usage: aggregate-results.py <research_output_directory>", file=sys.stderr)
        sys.exit(1)
    
    base_dir = Path(sys.argv[1])
    if not base_dir.exists():
        print(f"Error: Directory does not exist: {base_dir}", file=sys.stderr)
        sys.exit(1)
    
    summary = generate_summary(base_dir)
    summary_file = base_dir / "RESEARCH_SUMMARY.md"
    
    with open(summary_file, 'w') as f:
        f.write(summary)
    
    print(f"Summary: {summary_file}")
    print(f"Total agents: {sum(count_agent_outputs(base_dir).values())}")

if __name__ == "__main__":
    main()
