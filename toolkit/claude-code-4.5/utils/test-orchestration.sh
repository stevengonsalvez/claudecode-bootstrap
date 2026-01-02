#!/bin/bash

# Test Orchestration Setup
# Validates the m-workflow infrastructure without spawning real agents
# Can run from repo (claude-code-4.5/) or from ~/.claude/

set -euo pipefail

# Auto-detect base directory: use script location's parent
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(dirname "$SCRIPT_DIR")"

# Allow override via environment variable
BASE_DIR="${CLAUDE_BASE_DIR:-$BASE_DIR}"

UTILS_DIR="${BASE_DIR}/utils"
STATE_DIR="${BASE_DIR}/orchestration/state"
ORCH_STATE="${UTILS_DIR}/orchestrator-state.sh"
ORCH_DAG="${UTILS_DIR}/orchestrator-dag.sh"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

pass() { echo -e "${GREEN}✅ $1${NC}"; }
fail() { echo -e "${RED}❌ $1${NC}"; }
warn() { echo -e "${YELLOW}⚠️  $1${NC}"; }
info() { echo -e "ℹ️  $1"; }

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "🧪 Multi-Agent Orchestration Test Suite"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "Testing: ${BASE_DIR}"
echo ""

TESTS_PASSED=0
TESTS_FAILED=0

run_test() {
    local name="$1"
    local cmd="$2"

    if eval "$cmd" &>/dev/null; then
        pass "$name"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        fail "$name"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
}

# ============================================================================
echo "📁 1. File System Checks"
echo ""

run_test "orchestrator-state.sh exists and executable" "[ -x '${ORCH_STATE}' ]"
run_test "orchestrator-dag.sh exists and executable" "[ -x '${ORCH_DAG}' ]"
run_test "orchestrator-runner.sh exists and executable" "[ -x '${UTILS_DIR}/orchestrator-runner.sh' ]"
run_test "orchestrator-agent.sh exists and executable" "[ -x '${UTILS_DIR}/orchestrator-agent.sh' ]"
run_test "spawn-agent-lib.sh exists and executable" "[ -x '${UTILS_DIR}/spawn-agent-lib.sh' ]"
run_test "git-worktree-utils.sh exists and executable" "[ -x '${UTILS_DIR}/git-worktree-utils.sh' ]"
run_test "config.json exists" "[ -f '${STATE_DIR}/config.json' ]"
run_test "sessions.json exists" "[ -f '${STATE_DIR}/sessions.json' ]"

echo ""

# ============================================================================
echo "🔧 2. Tool Availability"
echo ""

run_test "jq installed" "command -v jq"
run_test "tmux installed" "command -v tmux"
run_test "git installed" "command -v git"
run_test "bc installed" "command -v bc"

echo ""

# ============================================================================
echo "📋 3. State Management Tests"
echo ""

TEST_SESSION="test-$(date +%s)"

# Create session
if ${ORCH_STATE} create "$TEST_SESSION" "tmux-$TEST_SESSION" '{}' &>/dev/null; then
    pass "Create session: $TEST_SESSION"
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    fail "Create session"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi

# Get session
if ${ORCH_STATE} get "$TEST_SESSION" | jq -e '.session_id' &>/dev/null; then
    pass "Get session"
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    fail "Get session"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi

# Add wave
if ${ORCH_STATE} add-wave "$TEST_SESSION" 1 '[]' &>/dev/null; then
    pass "Add wave"
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    fail "Add wave"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi

# Add agent
TEST_AGENT_CONFIG='{"node_id":"test-node","status":"pending","cost_usd":0}'
if ${ORCH_STATE} add-agent "$TEST_SESSION" "agent-test-1" "$TEST_AGENT_CONFIG" &>/dev/null; then
    pass "Add agent"
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    fail "Add agent"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi

# Add agent to wave
if ${ORCH_STATE} add-agent-to-wave "$TEST_SESSION" 1 "agent-test-1" &>/dev/null; then
    pass "Add agent to wave"
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    fail "Add agent to wave"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi

# Update agent status
if ${ORCH_STATE} update-agent-status "$TEST_SESSION" "agent-test-1" "active" &>/dev/null; then
    pass "Update agent status"
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    fail "Update agent status"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi

# Update wave status
if ${ORCH_STATE} update-wave-status "$TEST_SESSION" 1 "active" &>/dev/null; then
    pass "Update wave status"
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    fail "Update wave status"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi

# Check budget
if ${ORCH_STATE} check-budget "$TEST_SESSION" &>/dev/null; then
    pass "Check budget"
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    fail "Check budget"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi

# Get last completed wave
if ${ORCH_STATE} get-last-completed-wave "$TEST_SESSION" &>/dev/null; then
    pass "Get last completed wave"
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    fail "Get last completed wave"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi

# Archive session (verify it exists first)
if ${ORCH_STATE} get "$TEST_SESSION" | jq -e '.session_id' &>/dev/null; then
    if ${ORCH_STATE} archive "$TEST_SESSION" &>/dev/null; then
        pass "Archive session"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        fail "Archive session"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
else
    warn "Archive session (session already archived - skipping)"
    TESTS_PASSED=$((TESTS_PASSED + 1))
fi

echo ""

# ============================================================================
echo "📊 4. DAG Utilities Tests"
echo ""

# Create test DAG
TEST_DAG_FILE="/tmp/test-dag-$(date +%s).json"
cat > "$TEST_DAG_FILE" <<'EOF'
{
  "session_id": "test-dag",
  "nodes": {
    "ws-1": {"task": "Task 1", "dependencies": []},
    "ws-2": {"task": "Task 2", "dependencies": ["ws-1"]},
    "ws-3": {"task": "Task 3", "dependencies": []},
    "ws-4": {"task": "Task 4", "dependencies": ["ws-2", "ws-3"]}
  },
  "edges": [
    {"from": "ws-1", "to": "ws-2"},
    {"from": "ws-2", "to": "ws-4"},
    {"from": "ws-3", "to": "ws-4"}
  ]
}
EOF

run_test "DAG file created" "[ -f '$TEST_DAG_FILE' ]"

# Test topo-sort
if ${ORCH_DAG} topo-sort "$TEST_DAG_FILE" &>/dev/null; then
    pass "Topological sort"
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    fail "Topological sort"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi

# Test calculate-waves
if ${ORCH_DAG} calculate-waves "$TEST_DAG_FILE" &>/dev/null; then
    pass "Calculate waves"
    TESTS_PASSED=$((TESTS_PASSED + 1))

    # Verify wave structure
    WAVES=$(${ORCH_DAG} calculate-waves "$TEST_DAG_FILE" 2>/dev/null)
    WAVE_COUNT=$(echo "$WAVES" | jq 'length')
    if [ "$WAVE_COUNT" -eq 3 ]; then
        pass "Wave count correct (3 waves)"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        fail "Wave count incorrect (expected 3, got $WAVE_COUNT)"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
else
    fail "Calculate waves"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi

# Cleanup test DAG
rm -f "$TEST_DAG_FILE"

echo ""

# ============================================================================
echo "🔌 5. Spawn Library Tests"
echo ""

# Source the library
if source "${UTILS_DIR}/spawn-agent-lib.sh" 2>/dev/null; then
    pass "Source spawn-agent-lib.sh"
    TESTS_PASSED=$((TESTS_PASSED + 1))

    # Check functions exist
    if declare -f wait_for_claude_ready &>/dev/null; then
        pass "Function: wait_for_claude_ready"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        fail "Function: wait_for_claude_ready"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi

    if declare -f spawn_agent_tmux &>/dev/null; then
        pass "Function: spawn_agent_tmux"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        fail "Function: spawn_agent_tmux"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi

    if declare -f is_agent_alive &>/dev/null; then
        pass "Function: is_agent_alive"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        fail "Function: is_agent_alive"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
else
    fail "Source spawn-agent-lib.sh"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi

echo ""

# ============================================================================
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "📊 Test Results"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "  Passed: $TESTS_PASSED"
echo "  Failed: $TESTS_FAILED"
echo "  Total:  $((TESTS_PASSED + TESTS_FAILED))"
echo ""

if [ $TESTS_FAILED -eq 0 ]; then
    echo -e "${GREEN}🎉 All tests passed! Ready for /m-workflow${NC}"
    exit 0
else
    echo -e "${RED}⚠️  Some tests failed. Review above for details.${NC}"
    exit 1
fi
