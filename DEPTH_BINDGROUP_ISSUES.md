# Bassalt Rendering Issues - Progress Report

## Date: January 1, 2026

## Summary
Debugging WebGPU rendering issues in Bassalt (Minecraft WebGPU renderer). Two main issues have been identified and partially solved.

---

## Issue 1: Depth Attachment Mismatch ✅ SOLVED

### Problem
WebGPU requires exact match between pipeline's `depth_stencil` and render pass's `depth_stencil_attachment`. Minecraft decides these independently:
- `RenderTarget.useDepth` → determines if render pass has depth attachment
- `Pipeline.depthTestFunction` → determines if pipeline needs depth testing

MC uses `NO_DEPTH_TEST` pipelines with depth-enabled render targets, causing `IncompatibleDepthStencilAttachment` errors.

### Solution Implemented
1. **Pipelines always have `depth_stencil`** with `Depth32Float` format
   - For `NO_DEPTH_TEST`: use `depth_compare: Always`, `depth_write_enabled: false`
   - This effectively disables depth testing while maintaining format compatibility

2. **Dimension-based depth texture cache** in `BasaltDevice`
   - `depth_texture_cache: HashMap<(u32, u32), (TextureId, TextureViewId)>`
   - `get_or_create_depth_view(width, height)` creates/reuses depth textures
   - When MC doesn't provide depth texture, we create one matching color texture size

### Files Modified
- `bassalt-native/src/device.rs` - Added depth texture cache
- `bassalt-native/src/lib.rs` - Pipeline always has depth_stencil, render pass always gets depth view

---

## Issue 2: MissingBindGroup { index: 0 } ⚠️ IN PROGRESS

### Problem
When drawing, WebGPU reports `MissingBindGroup { index: 0 }`. Pipelines with `groups: 3` require all 3 bind groups to be set, but some code paths only set bind group 0.

### Root Cause Analysis
Two code paths for creating bind groups:

1. **`createMultiBindGroups`** - Creates and sets all 3 bind groups (0, 1, 2) ✅ Works
2. **`createBindGroup0`** - Creates only bind group 0 ❌ Missing groups 1 and 2

When `createMultiBindGroups` fails or returns 0, the fallback to `createBindGroup0` only sets bind group 0, but pipelines still expect bind groups at indices 1 and 2.

### Partial Fix Applied
Modified `createBindGroup0` to also create and set empty bind groups for indices 1 and 2 when pipeline expects them. However, the error persists in some render passes.

### Remaining Issue
Some render passes (e.g., texture atlas operations) still only have bind group 0 set. Need to investigate:
1. Which code path is creating these bind groups (handles 762-779 in logs)
2. Why empty bind groups for indices 1 and 2 aren't being created/set
3. Whether the bind group layout is compatible with the pipeline's expected layout

### Log Evidence
```
# Working render passes - all 3 bind groups set:
Set bind group 0 with handle 95 on render pass
Set bind group 1 with handle 97 on render pass
Set bind group 2 with handle 96 on render pass

# Problematic render passes - only bind group 0:
beginRenderPass: color_view_handle=237
Set bind group 0 with handle 762 on render pass
Set bind group 0 with handle 763 on render pass
... (no bind groups 1 or 2)

# Error:
MissingBindGroup { index: 0, pipeline: "Basalt Render Pipeline" }
```

---

## Next Steps

1. Trace which Java code creates bind groups for handles 762+
2. Ensure all code paths that create bind groups also set indices 1 and 2
3. Verify bind group layouts match pipeline expectations
4. Consider creating pipelines with fewer bind group layouts for simple operations

---

## Technical Details

### WebGPU Constraint
Pipeline's bind group layouts and render pass's set bind groups must match exactly:
- If pipeline has 3 bind group layouts, all 3 must be set before draw
- Bind group at index N must use the layout defined at pipeline.bind_group_layouts[N]

### MC Source Analysis
- `RenderPipelines.java` - GUI pipelines use `NO_DEPTH_TEST`
- `RenderTarget.java` - `useDepth` field determines depth texture creation
- `PostPass.java` - Checks `RenderTarget.useDepth` before including depth in render pass
- These are **independent** decisions, not coordinated
