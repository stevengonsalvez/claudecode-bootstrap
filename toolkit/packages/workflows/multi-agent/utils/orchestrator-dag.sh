#!/bin/bash

# DAG (Directed Acyclic Graph) Utility
# Handles dependency resolution and wave calculation
# Compatible with bash 3.x (macOS default)

set -euo pipefail

STATE_DIR="${HOME}/.claude/orchestration/state"

# topological_sort <dag_file>
# Returns nodes in topological order (waves)
# Uses pure jq implementation for bash 3.x compatibility
topological_sort() {
    local dag_file="$1"

    # Use jq to perform Kahn's algorithm
    jq -r '
        .nodes as $nodes |
        .edges as $edges |
        ($nodes | keys) as $all_nodes |

        # Calculate initial in-degrees
        (reduce $all_nodes[] as $n (
            {};
            . + {($n): ([$edges[] | select(.to == $n)] | length)}
        )) as $initial_indegrees |

        # Kahn algorithm
        {
            indegrees: $initial_indegrees,
            waves: [],
            remaining: $all_nodes
        } |
        until(.remaining | length == 0;
            .indegrees as $ind |
            # Find nodes with indegree 0 from remaining
            [.remaining[] | select($ind[.] == 0)] as $wave_nodes |

            if ($wave_nodes | length) == 0 then
                error("Cycle detected in DAG")
            else
                # Add wave
                .waves += [$wave_nodes] |

                # Remove processed nodes from remaining
                .remaining = (.remaining - $wave_nodes) |

                # Update indegrees for dependent nodes
                reduce ($wave_nodes[]) as $processed (.;
                    reduce ([$edges[] | select(.from == $processed) | .to][]) as $dep (.;
                        .indegrees[$dep] = (.indegrees[$dep] - 1)
                    )
                )
            end
        ) |
        .waves | to_entries | map("Wave \(.key + 1): \(.value | join(", "))") | .[]
    ' "$dag_file"
}

# calculate_waves <dag_file>
# Returns JSON array of waves with their nodes
calculate_waves() {
    local dag_file="$1"

    jq '
        .nodes as $nodes |
        .edges as $edges |
        ($nodes | keys) as $all_nodes |

        # Calculate initial in-degrees
        (reduce $all_nodes[] as $n (
            {};
            . + {($n): ([$edges[] | select(.to == $n)] | length)}
        )) as $initial_indegrees |

        # Kahn algorithm to group into waves
        {
            indegrees: $initial_indegrees,
            waves: [],
            remaining: $all_nodes
        } |
        until(.remaining | length == 0;
            .indegrees as $ind |
            # Find nodes with indegree 0 from remaining
            [.remaining[] | select($ind[.] == 0)] as $wave_nodes |

            if ($wave_nodes | length) == 0 then
                error("Cycle detected in DAG")
            else
                # Create wave object
                {
                    wave_number: ((.waves | length) + 1),
                    nodes: $wave_nodes,
                    status: "pending"
                } as $wave |

                # Add wave to results
                .waves += [$wave] |

                # Remove processed nodes from remaining
                .remaining = (.remaining - $wave_nodes) |

                # Update indegrees for dependent nodes
                reduce ($wave_nodes[]) as $processed (.;
                    reduce ([$edges[] | select(.from == $processed) | .to][]) as $dep (.;
                        .indegrees[$dep] = (.indegrees[$dep] - 1)
                    )
                )
            end
        ) |
        .waves
    ' "$dag_file"
}

# check_dependencies <dag_file> <node_id>
# Checks if all dependencies for a node are satisfied
check_dependencies() {
    local dag_file="$1"
    local node_id="$2"

    jq -r --arg n "$node_id" '
        .edges as $edges |
        .nodes as $nodes |

        # Get all dependencies (nodes that must complete before this one)
        [$edges[] | select(.to == $n) | .from] as $deps |

        if ($deps | length) == 0 then
            "true"
        else
            # Check if all dependencies are complete
            if ([$deps[] | $nodes[.].status] | all(. == "complete")) then
                "true"
            else
                "false"
            end
        end
    ' "$dag_file"
}

# get_next_wave <dag_file>
# Gets the next wave of nodes ready to execute
get_next_wave() {
    local dag_file="$1"

    jq -r '
        .edges as $edges |
        .nodes as $nodes |

        # Get pending nodes
        [$nodes | to_entries[] | select(.value.status == "pending") | .key] as $pending |

        # Filter to those with all dependencies complete
        [
            $pending[] |
            . as $n |
            [$edges[] | select(.to == $n) | .from] as $deps |
            if ($deps | length) == 0 then
                $n
            elif ([$deps[] | $nodes[.].status] | all(. == "complete")) then
                $n
            else
                empty
            end
        ] | join(" ")
    ' "$dag_file"
}

# validate_dag <dag_file>
# Validates DAG structure
validate_dag() {
    local dag_file="$1"

    # Check file exists
    if [ ! -f "$dag_file" ]; then
        echo "Error: DAG file not found: $dag_file" >&2
        return 1
    fi

    # Validate JSON structure
    if ! jq -e '.nodes and .edges' "$dag_file" &>/dev/null; then
        echo "Error: Invalid DAG structure (missing nodes or edges)" >&2
        return 1
    fi

    # Check for cycles (if topo-sort succeeds, no cycles)
    if ! calculate_waves "$dag_file" &>/dev/null; then
        echo "Error: DAG contains cycles" >&2
        return 1
    fi

    echo "DAG is valid"
    return 0
}

case "${1:-}" in
    topo-sort)
        topological_sort "$2"
        ;;
    calculate-waves)
        calculate_waves "$2"
        ;;
    check-deps)
        check_dependencies "$2" "$3"
        ;;
    next-wave)
        get_next_wave "$2"
        ;;
    validate)
        validate_dag "$2"
        ;;
    *)
        echo "Usage: orchestrator-dag.sh <command> [args...]"
        echo ""
        echo "Commands:"
        echo "  topo-sort <dag_file>           - Output nodes in topological order"
        echo "  calculate-waves <dag_file>     - Calculate execution waves (JSON)"
        echo "  check-deps <dag_file> <node>   - Check if node dependencies met"
        echo "  next-wave <dag_file>           - Get next ready-to-execute nodes"
        echo "  validate <dag_file>            - Validate DAG structure"
        exit 1
        ;;
esac
