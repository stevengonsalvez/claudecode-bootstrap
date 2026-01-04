#!/bin/bash
# ABOUTME: Claude CLI convenience commands for interactive use

# Create convenient aliases for different Claude interaction modes
alias claude-print='claude --print'
alias claude-script='claude --script'
alias claude-start='claude'

# Create functions for better user experience
claude-ask() {
    if [ $# -eq 0 ]; then
        echo "Usage: claude-ask \"your question here\""
        echo "Example: claude-ask \"How do I create a React component?\""
        return 1
    fi
    claude --print "$*"
}

claude-help() {
    echo "🤖 Claude CLI Commands"
    echo ""
    echo "Interactive modes:"
    echo "  claude-start          # Start interactive Claude CLI"
    echo "  claude                # Standard Claude CLI"
    echo ""
    echo "Direct query modes:"
    echo "  claude-ask \"question\" # Ask a single question"
    echo "  claude-print \"query\"  # Same as claude-ask"
    echo "  claude-script         # Read from stdin, useful for piping"
    echo ""
    echo "Examples:"
    echo "  claude-ask \"What files are in the current directory?\""
    echo "  echo \"Explain this code\" | claude-script"
    echo "  cat README.md | claude-script"
}

# Export functions so they're available in bash sessions
export -f claude-ask claude-help
