#!/bin/bash
# RenderDoc Attach Script for Bassalt
# This script attaches RenderDoc to an already-running Minecraft process
# NOTE: Only works on Windows! Linux/macOS users should use renderdoc_capture.sh instead

set -e

echo "======================================"
echo "  Bassalt RenderDoc Attach Script"
echo "======================================"
echo ""

# Check platform
if [[ "$OSTYPE" != "linux-gnu"* && "$OSTYPE" != "darwin"* ]]; then
    # Windows or other platform - continue
    :
else
    echo "❌ Error: RenderDoc injection is not supported on Linux/macOS!"
    echo ""
    echo "On Linux and macOS, you must launch the application with RenderDoc from the start."
    echo ""
    echo "Please use the capture script instead:"
    echo "  ./renderdoc_capture.sh"
    echo ""
    echo "Or manually launch with:"
    echo "  renderdoccmd --capture-file bassalt-capture.rdc ./gradlew runClient -Dbassalt.enabled=true"
    echo ""
    exit 1
fi

echo "Instructions:"
echo "1. Start Minecraft normally first:"
echo "   ./gradlew runClient -Dbassalt.enabled=true"
echo ""
echo "2. Once in-game, run this script to attach RenderDoc"
echo ""

# Check if RenderDoc is installed
RENDERDOC_CMD=""
if command -v renderdoccmd &> /dev/null; then
    RENDERDOC_CMD="renderdoccmd"
fi

RENDERDOC_GUI=""
if command -v qrenderdoc &> /dev/null; then
    RENDERDOC_GUI="qrenderdoc"
fi

if [ -z "$RENDERDOC_CMD" ]; then
    echo "❌ Error: RenderDoc not found!"
    echo "Install with: sudo apt install renderdoc"
    exit 1
fi

# Find the java process running Minecraft
# Look for processes with bassalt.enabled or fabric devlaunchinjector
echo "🔍 Looking for Minecraft Java process..."

# Try multiple patterns to find the Minecraft process
# Prioritize the actual game process over Gradle
JAVA_PID=$(
    pgrep -f "net.fabricmc.devlaunchinjector.Main" 2>/dev/null | head -1 ||
    pgrep -f "devlaunchinjector.Main" 2>/dev/null | head -1 ||
    pgrep -f "fabric.dli.env=client" 2>/dev/null | head -1 ||
    pgrep -f "bassalt.enabled=true.*fabric" 2>/dev/null | head -1
)

if [ -z "$JAVA_PID" ]; then
    echo "❌ No Minecraft process found!"
    echo ""
    echo "Let me check what Java processes are running..."
    echo ""
    ps aux | grep java | grep -v grep | head -5
    echo ""
    echo "Start Minecraft first with:"
    echo "  ./gradlew runClient -Dbassalt.enabled=true"
    exit 1
fi

echo "✅ Found Minecraft process (PID: $JAVA_PID)"
echo ""
echo "Process details:"
ps -p "$JAVA_PID" -o pid,cmd --no-headers | head -1
echo ""

# Confirm with user
read -p "Attach RenderDoc to this process? (y/N) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Cancelled."
    exit 0
fi

echo ""
echo "📡 Injecting RenderDoc into PID $JAVA_PID..."
echo ""

# Inject RenderDoc into the running process
"$RENDERDOC_CMD" inject --PID=$JAVA_PID

echo "✅ RenderDoc injected!"
echo ""
echo "📊 How to capture frames:"
echo "  1. In Minecraft, press F12 to toggle RenderDoc overlay"
echo "  2. Press F11 to capture a frame"
echo "  3. Captures are saved to: ~/RenderDoc/"
echo ""
echo "📂 To view captures:"
if [ -n "$RENDERDOC_GUI" ]; then
    echo "   Run: qrenderdoc"
    echo "   Then open your capture file from ~/RenderDoc/"
else
    echo "   Install qrenderdoc to view captures graphically"
    echo "   Or use: renderdoccmd replay <capture-file.rdc>"
fi
echo ""
echo "💡 Tip: Look for the debug markers (Terrain Rendering, etc.) in the Event Browser!"
