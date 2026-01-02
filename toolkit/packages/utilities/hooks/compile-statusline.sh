#!/bin/bash

# Compile statusline.js to a single executable for maximum performance
echo "Compiling statusline.js with Bun for optimal performance..."

cd "$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"

# Compile to standalone binary
bun build ./statusline.js --compile --outfile statusline-bin

# Make it executable
chmod +x statusline-bin

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"

echo "Compilation complete! Binary created at statusline-bin"
echo ""
echo "To use the compiled version, update your settings.json to:"
echo "  \"statusline\": \"$SCRIPT_DIR/statusline-bin\""
echo ""
echo "Performance comparison:"
echo "Original: bun statusline.js"
time echo '{"cwd":"'"$PWD"'","model":"claude-3-5-sonnet","session_id":"test"}' | bun statusline.js > /dev/null

echo ""
echo "Compiled: ./statusline-bin"
time echo '{"cwd":"'"$PWD"'","model":"claude-3-5-sonnet","session_id":"test"}' | ./statusline-bin > /dev/null
