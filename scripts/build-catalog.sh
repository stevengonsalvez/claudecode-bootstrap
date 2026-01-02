#!/usr/bin/env bash
#
# build-catalog.sh - Generate catalog.yaml from package manifests
#
# Usage: ./scripts/build-catalog.sh [--dry-run]
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
DRY_RUN="${1:-}"

log_info() {
    echo "[INFO] $1"
}

log_success() {
    echo "[OK] $1"
}

# Check dependencies
check_dependencies() {
    if ! command -v yq &> /dev/null; then
        echo "[ERROR] yq is required but not installed. Install with: brew install yq"
        exit 1
    fi
}

# Extract version from manifest
get_version() {
    local file="$1"
    local frontmatter
    frontmatter=$(sed -n '/^---$/,/^---$/p' "$file" | sed '1d;$d')
    echo "$frontmatter" | yq -r '.version // "1.0.0"' 2>/dev/null
}

# Extract name from manifest
get_name() {
    local file="$1"
    local frontmatter
    frontmatter=$(sed -n '/^---$/,/^---$/p' "$file" | sed '1d;$d')
    echo "$frontmatter" | yq -r '.name // ""' 2>/dev/null
}

# Extract description from manifest
get_description() {
    local file="$1"
    local frontmatter
    frontmatter=$(sed -n '/^---$/,/^---$/p' "$file" | sed '1d;$d')
    echo "$frontmatter" | yq -r '.description // ""' 2>/dev/null | head -c 100
}

main() {
    log_info "Building catalog from package manifests..."
    check_dependencies

    local catalog_file="$ROOT_DIR/catalog.yaml"
    local tmp_file="/tmp/catalog-build-$$.yaml"

    # Start building catalog
    cat > "$tmp_file" << 'EOF'
apiVersion: catalog/v1
metadata:
  name: claudecode-bootstrap
  version: 2.0.0
  description: Complete AI coding agent toolkit with skills, workflows, and agents
  author: stevengonsalvez
  repository: https://github.com/stevengonsalvez/ai-coder-rules
  license: Apache-2.0

components:
  skills:
EOF

    # Add skills
    for skill_dir in "$ROOT_DIR/packages/skills"/*; do
        if [[ -d "$skill_dir" && -f "$skill_dir/SKILL.md" ]]; then
            local name version desc path
            name=$(get_name "$skill_dir/SKILL.md")
            version=$(get_version "$skill_dir/SKILL.md")
            desc=$(get_description "$skill_dir/SKILL.md")
            path="packages/skills/$(basename "$skill_dir")"

            cat >> "$tmp_file" << EOF
    - name: $name
      path: $path
      version: $version
      description: $desc
EOF
            log_info "Added skill: $name ($version)"
        fi
    done

    # Add workflows section
    cat >> "$tmp_file" << 'EOF'

  workflows:
EOF

    for workflow_dir in "$ROOT_DIR/packages/workflows"/*; do
        if [[ -d "$workflow_dir" && -f "$workflow_dir/WORKFLOW.md" ]]; then
            local name version desc path
            name=$(get_name "$workflow_dir/WORKFLOW.md")
            version=$(get_version "$workflow_dir/WORKFLOW.md")
            desc=$(get_description "$workflow_dir/WORKFLOW.md")
            path="packages/workflows/$(basename "$workflow_dir")"

            cat >> "$tmp_file" << EOF
    - name: $name
      path: $path
      version: $version
      description: $desc
EOF
            log_info "Added workflow: $name ($version)"
        fi
    done

    # Count agents
    local agent_count
    agent_count=$(find "$ROOT_DIR/packages/agents" -name "AGENT.md" | wc -l | tr -d ' ')

    cat >> "$tmp_file" << EOF

  agents:
    count: $agent_count
    categories:
      - universal
      - orchestrators
      - engineering
      - design
      - research
      - meta
EOF
    log_info "Added $agent_count agents across categories"

    # Finalize
    if [[ "$DRY_RUN" == "--dry-run" ]]; then
        log_info "Dry run - catalog would be:"
        cat "$tmp_file"
        rm "$tmp_file"
    else
        mv "$tmp_file" "$catalog_file"
        log_success "Catalog written to $catalog_file"
    fi
}

main "$@"
