#!/bin/bash
# RenderDoc Capture Script for Bassalt
# This script launches Minecraft with Bassalt under RenderDoc profiling

set -e

# Project directory
PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$PROJECT_DIR"

# Check if RenderDoc is installed
if ! command -v qrenderdoc &> /dev/null && ! command -v renderdoccmd &> /dev/null; then
    echo "❌ Error: RenderDoc not found!"
    echo "Install with: sudo apt install renderdoc"
    exit 1
fi

# Set up RenderDoc capture environment
# The Vulkan layer requires ENABLE_VULKAN_RENDERDOC_CAPTURE=1 to activate
export ENABLE_VULKAN_RENDERDOC_CAPTURE=1
export RENDERDOC_CAPTUREFILE="$PROJECT_DIR/bassalt-capture.rdc"

# Enable Bassalt
export BASALT_ENABLED=true

# Build native library first
echo "Building Bassalt native library..."
cd bassalt-native
cargo build --release
cd ..

# Launch with RenderDoc Vulkan layer
echo "Launching Minecraft with RenderDoc capture..."
echo "Press F12 to toggle overlay, F11 to capture a frame"
echo ""
echo "To verify RenderDoc is loaded, look for:"
echo "  - A 'RenderDoc' overlay in the top-left corner (toggle with F12)"
echo "  - Captures are saved to: /tmp/RenderDoc/"
echo ""

# Run normally - RenderDoc will inject via Vulkan layer
./gradlew runClient -Dbassalt.enabled=true "$@"

echo ""
echo "Game exited. Checking for captures in /tmp/RenderDoc/..."
CAPTURES=$(ls -1 /tmp/RenderDoc/*.rdc 2>/dev/null | wc -l)
if [ "$CAPTURES" -gt 0 ]; then
    echo "✅ Found $CAPTURES capture(s):"
    ls -lh /tmp/RenderDoc/*.rdc 2>/dev/null | tail -5
    echo ""
    echo "To view a capture:"
    echo "  qrenderdoc /tmp/RenderDoc/<filename>.rdc"
    LATEST=$(ls -t /tmp/RenderDoc/*.rdc 2>/dev/null | head -1)
    if [ -n "$LATEST" ]; then
        echo ""
        echo "Quick view latest capture:"
        echo "  qrenderdoc $LATEST"
    fi
else
    echo "ℹ️  No captures found in /tmp/RenderDoc/"
    echo "   Make sure to press F11 while the overlay is visible (F12)"
fi
