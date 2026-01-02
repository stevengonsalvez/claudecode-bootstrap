#!/bin/bash

# Compile statusline.js to a single executable for maximum performance
echo "Compiling statusline.js with Bun for optimal performance..."

cd /Users/stevengonsalvez/d/git/ai-coder-rules/claude-code/hooks

# Compile to standalone binary
bun build ./statusline.js --compile --outfile statusline-bin

# Make it executable
chmod +x statusline-bin

echo "Compilation complete! Binary created at statusline-bin"
echo ""
echo "To use the compiled version, update your settings.json to:"
echo '  "statusline": "/Users/stevengonsalvez/d/git/ai-coder-rules/claude-code/hooks/statusline-bin"'
echo ""
echo "Performance comparison:"
echo "Original: bun statusline.js"
time echo '{"cwd":"/Users/stevengonsalvez/d/git/ai-coder-rules","model":"claude-3-5-sonnet","session_id":"test"}' | bun statusline.js > /dev/null

echo ""
echo "Compiled: ./statusline-bin"
time echo '{"cwd":"/Users/stevengonsalvez/d/git/ai-coder-rules","model":"claude-3-5-sonnet","session_id":"test"}' | ./statusline-bin > /dev/null