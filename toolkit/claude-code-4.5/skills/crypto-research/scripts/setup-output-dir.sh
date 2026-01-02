#!/bin/bash
# ABOUTME: Creates timestamped output directory structure for crypto research results

set -euo pipefail

TIMESTAMP=$(date +"%Y-%m-%d_%H-%M-%S")
MODE="${1:-full}"
BASE_DIR="outputs/${TIMESTAMP}"

if [ "$MODE" = "haiku" ] || [ "$MODE" = "lightweight" ]; then
    BASE_DIR="${BASE_DIR}/haiku"
fi

mkdir -p "${BASE_DIR}/crypto_market"
mkdir -p "${BASE_DIR}/crypto_analysis"
mkdir -p "${BASE_DIR}/crypto_macro"
mkdir -p "${BASE_DIR}/crypto_plays"

if [ "$MODE" = "full" ] || [ "$MODE" = "comprehensive" ]; then
    mkdir -p "${BASE_DIR}/crypto_news"
    mkdir -p "${BASE_DIR}/crypto_movers"
fi

echo "${BASE_DIR}"
exit 0
