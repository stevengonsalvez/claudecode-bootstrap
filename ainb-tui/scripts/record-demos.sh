#!/bin/bash
set -e

echo "🎬 Recording TUI demos with VHS..."

# Check if VHS is installed
if ! command -v vhs &> /dev/null; then
    echo "❌ VHS is not installed. Please install it:"
    echo "   macOS: brew install vhs"
    echo "   Linux: go install github.com/charmbracelet/vhs@latest"
    exit 1
fi

# Build release binary first
echo "🔨 Building release binary..."
cargo build --release

# Create recordings directory
mkdir -p tests/recordings

# Record all tapes
for tape in tests/tapes/*.tape; do
    name=$(basename "$tape" .tape)
    echo "📹 Recording: $name"
    vhs "$tape"
done

echo "✅ All recordings complete!"
echo "📂 Recordings saved to: tests/recordings/"
ls -lh tests/recordings/
