#!/bin/bash

# DAG (Directed Acyclic Graph) Utility
# Handles dependency resolution and wave calculation

set -euo pipefail

STATE_DIR="${HOME}/.claude/orchestration/state"

# topological_sort <dag_file>
# Returns nodes in topological order (waves)
topological_sort() {
    local dag_file="$1"

    # Extract nodes and edges
    local nodes=$(jq -r '.nodes | keys[]' "$dag_file")
    local edges=$(jq -r '.edges' "$dag_file")

    # Calculate in-degree for each node
    declare -A indegree
    for node in $nodes; do
        local deps=$(jq -r --arg n "$node" '.edges[] | select(.to == $n) | .from' "$dag_file" | wc -l)
        indegree[$node]=$deps
    done

    # Topological sort using Kahn's algorithm
    local wave=1
    local result=""

    while [ ${#indegree[@]} -gt 0 ]; do
        local wave_nodes=""

        # Find all nodes with indegree 0
        for node in "${!indegree[@]}"; do
            if [ "${indegree[$node]}" -eq 0 ]; then
                wave_nodes="$wave_nodes $node"
            fi
        done

        if [ -z "$wave_nodes" ]; then
            echo "Error: Cycle detected in DAG" >&2
            return 1
        fi

        # Output wave
        echo "$wave:$wave_nodes"

        # Remove processed nodes and update indegrees
        for node in $wave_nodes; do
            unset indegree[$node]

            # Decrease indegree for dependent nodes
            local dependents=$(jq -r --arg n "$node" '.edges[] | select(.from == $n) | .to' "$dag_file")
            for dep in $dependents; do
                if [ -n "${indegree[$dep]:-}" ]; then
                    indegree[$dep]=$((indegree[$dep] - 1))
                fi
            done
        done

        ((wave++))
    done
}

# check_dependencies <dag_file> <node_id>
# Checks if all dependencies for a node are satisfied
check_dependencies() {
    local dag_file="$1"
    local node_id="$2"

    local deps=$(jq -r --arg n "$node_id" '.edges[] | select(.to == $n) | .from' "$dag_file")

    if [ -z "$deps" ]; then
        echo "true"
        return 0
    fi

    # Check if all dependencies are complete
    for dep in $deps; do
        local status=$(jq -r --arg n "$dep" '.nodes[$n].status' "$dag_file")
        if [ "$status" != "complete" ]; then
            echo "false"
            return 1
        fi
    done

    echo "true"
}

# get_next_wave <dag_file>
# Gets the next wave of nodes ready to execute
get_next_wave() {
    local dag_file="$1"

    local nodes=$(jq -r '.nodes | to_entries[] | select(.value.status == "pending") | .key' "$dag_file")

    local wave_nodes=""
    for node in $nodes; do
        if [ "$(check_dependencies "$dag_file" "$node")" = "true" ]; then
            wave_nodes="$wave_nodes $node"
        fi
    done

    echo "$wave_nodes" | tr -s ' '
}

case "${1:-}" in
    topo-sort)
        topological_sort "$2"
        ;;
    check-deps)
        check_dependencies "$2" "$3"
        ;;
    next-wave)
        get_next_wave "$2"
        ;;
    *)
        echo "Usage: orchestrator-dag.sh <command> [args...]"
        echo "Commands:"
        echo "  topo-sort <dag_file>"
        echo "  check-deps <dag_file> <node_id>"
        echo "  next-wave <dag_file>"
        exit 1
        ;;
esac
