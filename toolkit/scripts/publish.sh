#!/usr/bin/env bash
#
# publish.sh - Publish packages to marketplace
#
# Usage: ./scripts/publish.sh [--dry-run] [component-name]
#
# Examples:
#   ./scripts/publish.sh --dry-run              # Dry run for all
#   ./scripts/publish.sh webapp-testing         # Publish single skill
#   ./scripts/publish.sh --dry-run multi-agent  # Dry run for workflow
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

DRY_RUN=""
COMPONENT=""

# Parse arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run)
            DRY_RUN="--dry-run"
            shift
            ;;
        *)
            COMPONENT="$1"
            shift
            ;;
    esac
done

log_info() {
    echo "[INFO] $1"
}

log_success() {
    echo "[OK] $1"
}

log_warn() {
    echo "[WARN] $1"
}

# Validate before publish
validate_first() {
    log_info "Running validation before publish..."
    if ! "$SCRIPT_DIR/validate-manifests.sh"; then
        echo "[ERROR] Validation failed. Fix errors before publishing."
        exit 1
    fi
}

# Package a component for publishing
package_component() {
    local type="$1"  # skill, workflow, agent
    local path="$2"
    local name
    name=$(basename "$path")

    log_info "Packaging $type: $name"

    local package_dir="/tmp/publish-$name-$$"
    mkdir -p "$package_dir"

    # Copy component files
    cp -r "$path"/* "$package_dir/"

    # Create package metadata
    local manifest_file
    case "$type" in
        skill) manifest_file="SKILL.md" ;;
        workflow) manifest_file="WORKFLOW.md" ;;
        agent) manifest_file="AGENT.md" ;;
    esac

    if [[ -f "$path/$manifest_file" ]]; then
        # Extract and create package.json
        local frontmatter
        frontmatter=$(sed -n '/^---$/,/^---$/p' "$path/$manifest_file" | sed '1d;$d')

        local name version description
        name=$(echo "$frontmatter" | yq -r '.name // ""')
        version=$(echo "$frontmatter" | yq -r '.version // "1.0.0"')
        description=$(echo "$frontmatter" | yq -r '.description // ""' | head -c 200)

        cat > "$package_dir/package.json" << EOF
{
  "name": "@claudecode/$name",
  "version": "$version",
  "description": "$description",
  "type": "$type",
  "main": "$manifest_file",
  "repository": "https://github.com/stevengonsalvez/ai-coder-rules",
  "license": "Apache-2.0"
}
EOF
    fi

    if [[ -n "$DRY_RUN" ]]; then
        log_info "Would package: $package_dir"
        log_info "Contents:"
        ls -la "$package_dir"
        rm -rf "$package_dir"
    else
        # Create tarball
        local tarball="$ROOT_DIR/dist/${type}-${name}-$(date +%Y%m%d).tar.gz"
        mkdir -p "$ROOT_DIR/dist"
        tar -czf "$tarball" -C "$(dirname "$package_dir")" "$(basename "$package_dir")"
        rm -rf "$package_dir"
        log_success "Created: $tarball"
    fi
}

# Publish all or single component
publish() {
    validate_first

    if [[ -n "$COMPONENT" ]]; then
        # Find and publish single component
        local found=false

        # Check skills
        if [[ -d "$ROOT_DIR/packages/skills/$COMPONENT" ]]; then
            package_component "skill" "$ROOT_DIR/packages/skills/$COMPONENT"
            found=true
        fi

        # Check workflows
        if [[ -d "$ROOT_DIR/packages/workflows/$COMPONENT" ]]; then
            package_component "workflow" "$ROOT_DIR/packages/workflows/$COMPONENT"
            found=true
        fi

        # Check agents (need to search recursively)
        local agent_path
        agent_path=$(find "$ROOT_DIR/packages/agents" -type d -name "$COMPONENT" 2>/dev/null | head -1)
        if [[ -n "$agent_path" && -d "$agent_path" ]]; then
            package_component "agent" "$agent_path"
            found=true
        fi

        if [[ "$found" == false ]]; then
            echo "[ERROR] Component not found: $COMPONENT"
            exit 1
        fi
    else
        # Publish all
        log_info "Publishing all components..."

        # Skills
        for skill in "$ROOT_DIR/packages/skills"/*; do
            [[ -d "$skill" ]] && package_component "skill" "$skill"
        done

        # Workflows
        for workflow in "$ROOT_DIR/packages/workflows"/*; do
            [[ -d "$workflow" ]] && package_component "workflow" "$workflow"
        done

        log_success "All components packaged successfully!"
    fi
}

main() {
    log_info "Publish mode: ${DRY_RUN:-live}"
    [[ -n "$COMPONENT" ]] && log_info "Component: $COMPONENT"

    publish
}

main
