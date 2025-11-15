#!/bin/bash

# ABOUTME: Git worktree utilities for agent workspace isolation

set -euo pipefail

# Create agent worktree with isolated branch
create_agent_worktree() {
    local AGENT_ID=$1
    local BASE_BRANCH=${2:-$(git branch --show-current)}

    local WORKTREE_DIR="worktrees/agent-${AGENT_ID}"
    local BRANCH_NAME="agent/agent-${AGENT_ID}"

    # Create worktrees directory if needed
    mkdir -p worktrees

    # Create worktree with new branch
    git worktree add -b "$BRANCH_NAME" "$WORKTREE_DIR" "$BASE_BRANCH"

    echo "$WORKTREE_DIR"
}

# Remove agent worktree
cleanup_agent_worktree() {
    local AGENT_ID=$1
    local FORCE=${2:-false}

    local WORKTREE_DIR="worktrees/agent-${AGENT_ID}"
    local BRANCH_NAME="agent/agent-${AGENT_ID}"

    if [ ! -d "$WORKTREE_DIR" ]; then
        echo "❌ Worktree not found: $WORKTREE_DIR"
        return 1
    fi

    # Check for uncommitted changes
    if ! git -C "$WORKTREE_DIR" diff --quiet 2>/dev/null; then
        if [ "$FORCE" = false ]; then
            echo "⚠️  Worktree has uncommitted changes. Use --force to remove anyway."
            return 1
        fi
    fi

    # Remove worktree
    git worktree remove "$WORKTREE_DIR" $( [ "$FORCE" = true ] && echo "--force" )

    # Delete branch (only if merged or forced)
    git branch -d "$BRANCH_NAME" 2>/dev/null || \
        ( [ "$FORCE" = true ] && git branch -D "$BRANCH_NAME" )
}

# List all agent worktrees
list_agent_worktrees() {
    git worktree list | grep "worktrees/agent-" || echo "No agent worktrees found"
}

# Merge agent work into current branch
merge_agent_work() {
    local AGENT_ID=$1
    local BRANCH_NAME="agent/agent-${AGENT_ID}"

    if ! git show-ref --verify --quiet "refs/heads/$BRANCH_NAME"; then
        echo "❌ Branch not found: $BRANCH_NAME"
        return 1
    fi

    git merge "$BRANCH_NAME"
}

# Check if worktree exists
worktree_exists() {
    local AGENT_ID=$1
    local WORKTREE_DIR="worktrees/agent-${AGENT_ID}"

    [ -d "$WORKTREE_DIR" ]
}

# Main CLI
case "${1:-help}" in
    create)
        create_agent_worktree "$2" "${3:-}"
        ;;
    cleanup)
        cleanup_agent_worktree "$2" "${3:-false}"
        ;;
    list)
        list_agent_worktrees
        ;;
    merge)
        merge_agent_work "$2"
        ;;
    exists)
        worktree_exists "$2"
        ;;
    *)
        echo "Usage: git-worktree-utils.sh {create|cleanup|list|merge|exists} [args]"
        exit 1
        ;;
esac
