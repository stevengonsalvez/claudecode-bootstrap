#!/usr/bin/env bash
#
# validate-manifests.sh - Validate all SKILL.md, WORKFLOW.md, and AGENT.md manifests
#
# Usage: ./scripts/validate-manifests.sh [--verbose]
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
VERBOSE="${1:-}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

errors=0
warnings=0
validated=0

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
    ((warnings++))
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
    ((errors++))
}

log_verbose() {
    if [[ "$VERBOSE" == "--verbose" ]]; then
        echo "  $1"
    fi
}

# Check if yq is available for YAML parsing
check_dependencies() {
    if ! command -v yq &> /dev/null; then
        log_error "yq is required but not installed. Install with: brew install yq"
        exit 1
    fi
}

# Validate a manifest file
validate_manifest() {
    local file="$1"
    local type="$2"  # SKILL, WORKFLOW, or AGENT
    local required_fields=("name" "version" "description")
    local has_errors=false

    log_verbose "Validating: $file"

    # Check file exists
    if [[ ! -f "$file" ]]; then
        log_error "File not found: $file"
        return 1
    fi

    # Extract frontmatter (between --- markers)
    local frontmatter
    frontmatter=$(sed -n '/^---$/,/^---$/p' "$file" | sed '1d;$d')

    if [[ -z "$frontmatter" ]]; then
        log_error "$file: No YAML frontmatter found"
        return 1
    fi

    # Validate required fields
    for field in "${required_fields[@]}"; do
        local value
        value=$(echo "$frontmatter" | yq -r ".$field // empty" 2>/dev/null || echo "")
        if [[ -z "$value" ]]; then
            log_error "$file: Missing required field '$field'"
            has_errors=true
        fi
    done

    # Check for license field
    local license
    license=$(echo "$frontmatter" | yq -r ".license // empty" 2>/dev/null || echo "")
    if [[ -z "$license" ]]; then
        log_warn "$file: Missing 'license' field (recommended)"
    fi

    # Type-specific validations
    case "$type" in
        SKILL)
            # Check for allowed-tools in skills
            local tools
            tools=$(echo "$frontmatter" | yq -r '."allowed-tools" // empty' 2>/dev/null || echo "")
            if [[ -z "$tools" ]]; then
                log_verbose "$file: No 'allowed-tools' specified (optional)"
            fi
            ;;
        WORKFLOW)
            # Check for pipeline definition
            local pipeline
            pipeline=$(echo "$frontmatter" | yq -r '.pipeline // empty' 2>/dev/null || echo "")
            if [[ -z "$pipeline" ]]; then
                log_warn "$file: No 'pipeline' defined for workflow"
            fi
            ;;
        AGENT)
            # Check for allowed-tools in agents
            local tools
            tools=$(echo "$frontmatter" | yq -r '."allowed-tools" // empty' 2>/dev/null || echo "")
            if [[ -z "$tools" ]]; then
                log_warn "$file: No 'allowed-tools' specified for agent"
            fi
            ;;
    esac

    if [[ "$has_errors" == true ]]; then
        return 1
    fi

    ((validated++))
    return 0
}

main() {
    log_info "Validating manifests in $ROOT_DIR/packages..."
    check_dependencies

    # Validate Skills
    log_info "Validating SKILL.md files..."
    while IFS= read -r -d '' file; do
        validate_manifest "$file" "SKILL" || true
    done < <(find "$ROOT_DIR/packages/skills" -name "SKILL.md" -print0 2>/dev/null)

    # Validate Workflows
    log_info "Validating WORKFLOW.md files..."
    while IFS= read -r -d '' file; do
        validate_manifest "$file" "WORKFLOW" || true
    done < <(find "$ROOT_DIR/packages/workflows" -name "WORKFLOW.md" -print0 2>/dev/null)

    # Validate Agents
    log_info "Validating AGENT.md files..."
    while IFS= read -r -d '' file; do
        validate_manifest "$file" "AGENT" || true
    done < <(find "$ROOT_DIR/packages/agents" -name "AGENT.md" -print0 2>/dev/null)

    # Summary
    echo ""
    log_info "Validation Summary:"
    echo "  Validated: $validated"
    echo "  Warnings:  $warnings"
    echo "  Errors:    $errors"

    if [[ $errors -gt 0 ]]; then
        log_error "Validation failed with $errors error(s)"
        exit 1
    fi

    log_info "All manifests validated successfully!"
}

main "$@"
