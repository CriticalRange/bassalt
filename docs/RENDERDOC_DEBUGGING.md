# RenderDoc Debugging Guide for Bassalt Renderer

## Overview

This guide explains how to use RenderDoc to debug rendering issues in the Bassalt Renderer when the game shows a black screen or no visible GUI elements.

## Prerequisites

- Windows (RenderDoc on Linux/Wayland has limited support)
- Bassalt built with debug symbols: `cd bassalt-native && cargo build`
- Minecraft 26.1-snapshot-1 with Bassalt enabled

## Capturing a Frame

### Option 1: Launch through RenderDoc GUI

1. Open RenderDoc
2. File → Launch Application
3. Configure:
   - **Executable**: Path to Java (e.g., `C:\Program Files\Java\jdk-25\bin\java.exe`)
   - **Working Directory**: Your Bassalt project directory
   - **Command Line Args**:
     ```
     -Dbassalt.enabled=true -Djava.library.path=build\natives -cp "build\classes\java\main;build\resources\main" net.fabricmc.devlaunchinjector.Main
     ```
   - **Capture frame(s)**: Choose "F12" to capture
4. Click Launch
5. When the game loads (or when you see the black screen), press F12 to capture a frame

### Option 2: Inject into Running Process

1. Start Minecraft with Bassalt enabled
2. In RenderDoc: File → Inject into Process
3. Select the java.exe process
4. Press F12 when you want to capture

## Analysis Workflow

### Step 1: Verify the Capture

1. **Event Browser** (left panel) - Check that events are listed
2. **Timeline** (bottom) - Should show a timeline of GPU commands
3. **API Inspector** (right panel) - Shows the WebGPU/Vulkan calls

### Step 2: Check Final Output

1. Scroll to the **bottom** of the Event Browser
2. Find the last `Present` or queue submit event
3. Click on it
4. Check the **Output** texture in the Texture Viewer (bottom panel)
5. **What you see**:
   - Background color only → Blit failed or render pass didn't write
   - Black screen → Clear is black, same issue
   - Visible GUI → Rendering works (not your case)

### Step 3: Analyze the Blit Pass (if exists)

1. Look for a render pass named "blit" or similar before the Present
2. Click on a `Draw` call inside the blit pass
3. **Check in Pipeline State** (right panel):
   - **Input Layout**: What vertex format?
   - **Shaders**: Click fragment shader to view code
   - **Framebuffer**: What's the output?
4. **Check in Mesh Viewer** (bottom panel):
   - Do you see a fullscreen quad/triangle?
   - If NO → Blit geometry is broken
5. **Check in Textures** (bottom panel):
   - Is the source texture bound?
   - Is the destination (swapchain) bound?

### Step 4: Analyze the Main Render Pass (CRITICAL)

1. Scroll up to find the **FIRST render pass** (before blit)
2. Click on the first `DrawIndexed` or `Draw` event
3. **Mesh Viewer** (bottom panel) - **MOST IMPORTANT CHECK**:
   - **Do you see geometry?**
   - If YES → Vertices processed correctly
     - Check if within viewport bounds
     - Check if colors are correct
   - If NO → Problem found! Continue to next checks

### Step 5: Diagnose the Root Cause

Based on what Mesh Viewer shows:

#### Case A: No Geometry Visible

**Check Vertex Buffer** (Pipeline State → Input Assembly):
1. Click on vertex buffer binding
2. **Look at raw data**:
   - Are values non-zero?
   - Valid position data (Float32x3)?
   - Valid color data (Float32x4)?
3. **If all zeros/NaN** → Vertex data not uploaded correctly

**Check Uniform Buffer - Projection Matrix** (Pipeline State → Descriptors):
1. Find "Projection" or "DynamicTransforms" binding
2. Click to view data
3. **Check first 16 floats** (4x4 matrix):
   ```
   Valid orthographic example:
   2.0/width, 0.0,       0.0, 0.0,
   0.0,       2.0/height, 0.0, 0.0,
   0.0,       0.0,       1.0, 0.0,
   -1.0,      -1.0,      0.0, 1.0
   ```
4. **If all zeros/NaN/invalid** → **This is your problem!**
   - The projection matrix transforms vertices off-screen
   - Fix: Check uniform buffer upload in Java layer

#### Case B: Geometry Visible but Wrong Position

**Check Viewport** (Pipeline State → Output Merger):
- Should be: `x: 0, y: 0, width: 854, height: 480` (or your resolution)
- If (0,0,0,0) → **Viewport bug!** (should be fixed by our viewport change)
- If wrong dimensions → Viewport not set correctly

**Check Scissor** (Pipeline State → Output Merger):
- Is scissor enabled?
- If enabled, does it cover the viewport?
- If (0,0,0,0) → **Scissor clipping everything!**

#### Case C: Geometry Visible and Positioned Correctly

**Check Render Target** (Textures panel):
1. Click on the color attachment output
2. What do you see after the draw?
3. If empty → Fragment shader not writing
4. If shows content → Rendering works! Issue is elsewhere

### Step 6: Check Depth State

1. In Pipeline State → Depth Stencil
2. Check if depth test is enabled
3. Check depth compare operation
4. **If depth always fails** → Nothing renders
5. **If depth write enabled** → Check depth buffer content in Texture Viewer

### Step 7: Verify Pipeline Configuration

1. **Pipeline State** panel shows all bound resources
2. Check each section:
   - **Vertex Input**: Buffers bound?
   - **Shaders**: Compiled successfully?
   - **Render Targets**: Format matches?
   - **Blend State**: Correct for transparency?
   - **Rasterizer**: Culling mode correct?

## Common Issues and Fixes

### Issue 1: Viewport is (0,0,0,0)

**Symptom**: Mesh Viewer shows geometry but output is empty

**Fix Applied**: Added automatic viewport setup in `render_pass.rs:194-239`

```rust
// Set default viewport to full render target
state.commands.push(RenderCommand::SetViewport {
    x: 0.0,
    y: 0.0,
    width: width as f32,
    height: height as f32,
    min_depth: 0.0,
    max_depth: 1.0,
});
```

### Issue 2: Projection Matrix All Zeros

**Symptom**: Mesh Viewer shows geometry at origin (0,0,0)

**Fix Needed**: Check uniform buffer upload in `BassaltDevice.java` or wherever projection matrix is set

**Debug**: Add logging to verify matrix data before upload

### Issue 3: Vertex Buffer Empty

**Symptom**: Mesh Viewer shows nothing, vertex buffer data is all zeros

**Fix Needed**: Check vertex buffer upload in Minecraft's rendering code

**Debug**: Verify `GpuBuffer.write()` is being called with correct data

### Issue 4: Scissor Rect Clipping

**Symptom**: Viewport is correct but scissor is (0,0,0,0)

**Fix**: Ensure scissor is set to viewport size or disabled

### Issue 5: Wrong Texture Bound for Blit

**Symptom**: Render pass works, swapchain is black

**Fix**: Check that main framebuffer texture ID is correct and not stale

**Debug**: Add logging for texture/view ID mapping

## Reporting Bugs

When reporting a rendering bug, include:

1. **Screenshots**:
   - RenderDoc Event Browser
   - Mesh Viewer (showing what you see)
   - Texture Viewer (output texture)
   - Pipeline State (relevant sections)

2. **Data**:
   - First 16 floats of projection matrix
   - Viewport and scissor values
   - Vertex buffer format and sample data

3. **Logs**:
   - `run/logs/latest.log` around the frame capture
   - Any validation errors from RenderDoc

## Tips and Tricks

### Keyboard Shortcuts

- `F` in Texture Viewer → Fit to window
- `A` in Mesh Viewer → Reset camera
- `Ctrl+D` → Toggle between draw calls
- `Ctrl+Tab` → Next event

### Useful Views

- **Texture Viewer**: View any texture at any point in the frame
- **Mesh Viewer**: See what geometry looks like after vertex shader
- **Shader Viewer**: View disassembly and debug shaders
- **API Inspector**: See exact WebGPU/Vulkan calls

### Breakdown View

Click the "Breakdown" button (grid icon) to see a heat map of GPU cost per draw call. Helps identify expensive operations.

## Additional Resources

- [RenderDoc Official Documentation](https://renderdoc.org/docs/)
- [WebGPU Specification](https://www.w3.org/TR/webgpu/)
- [Bassalt CLAUDE.md](../CLAUDE.md) - Project architecture
- [Bassalt Shader Documentation](../src/main/resources/shaders/wgsl/) - WGSL shaders

## Current Known Issues

- **Viewport not set automatically** → Fixed in `render_pass.rs:194-239`
- **Load vs Clear for render passes** → First frame may load uninitialized data
- **Texture view ID mapping** → May become stale across frames

## Next Steps After Capturing

Once you identify the issue:

1. **Verify the fix** in code
2. **Rebuild** the native library: `cd bassalt-native && cargo build --release`
3. **Rebuild** the mod: `./gradlew build`
4. **Test** the fix
5. **Capture again** to verify the fix worked

## Troubleshooting RenderDoc

### Capture is empty

- Make sure you're capturing after the game loads
- Try capturing at different points (menu vs in-game)
- Check that Bassalt is actually being used (check logs)

### Can't see shaders

- Make sure debug symbols are available
- Rebuild with `cargo build` (not `--release`)

### Inject fails

- Run as administrator
- Try launching through RenderDoc instead of injecting

---

**Last Updated**: 2025-01-03
**Related Files**: `bassalt-native/src/render_pass.rs`, `docs/RENDERDOC_DEBUGGING.md`
