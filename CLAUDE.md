# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Bassalt Renderer is a Minecraft Fabric mod targeting Minecraft version 26.1-snapshot-1. It implements a custom WebGPU-based rendering backend using wgpu-core (Rust) with a JNI bridge to Java.

**Key Architecture Decision**: This mod uses Rust + wgpu-core directly (not wgpu-native's C API) for memory safety and cleaner JNI integration. The native library is compiled as a cdylib and loaded via JNI.

**Important**: The package name is `com.criticalrange.bassalt` (note: bassalt, not basalt).

## Requirements

- **Java 25** (required for compilation and runtime)
- **Rust** (latest stable, for building native library)
- **Cargo** (comes with Rust)
- **Minecraft 26.1-snapshot-1** (Fabric)

## Build Commands

```bash
# Build the complete mod (Rust native library + Java code)
./gradlew build

# Build only the Rust native library (development)
cd bassalt-native && cargo build --release

# Build debug version of native library
cd bassalt-native && cargo build

# Build shader converter tool only
cd bassalt-native && cargo build --release --bin shader_converter

# Clean all build artifacts
./gradlew clean

# Run the mod with Bassalt enabled
./gradlew runClient -Dbassalt.enabled=true

# Run with mixin debugging (exports transformed classes to run/.mixin.out/)
./gradlew runClient -Dbassalt.enabled=true -Dmixin.debug.export=true
```

**Build Process Flow**:
1. `buildShaderConverter` - Builds the shader_converter Rust binary
2. `convertShaders` - Converts GLSL shaders from `src/main/resources/shaders/` to WGSL in `src/main/resources/shaders/wgsl/`
3. `buildNative` - Compiles the Rust native library (bassalt-native)
4. `copyNativeLibrary` - Copies the built library to `src/main/resources/native/` and `META-INF/native/`
5. `processResources` - Processes resources including converted shaders
6. `jar` / `build` - Packages everything into the final JAR

**Important**: The native library (.so/.dll/.dylib) is automatically built via cargo and included in the JAR under `META-INF/native/`. The build will fail if cargo is not available or if the Rust code doesn't compile.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        Minecraft 26.1                           │
│                    (GpuBackend/GpuDevice API)                   │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Bassalt Renderer                             │
├─────────────────────────────────────────────────────────────────┤
│  Java Layer (JNI Bridge)                                        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐   │
│  │ BassaltBackend│  │ BassaltDevice │  │ Resource Wrappers    │   │
│  │              │  │              │  │ (Buffer, Texture,    │   │
│  │ - init()     │  │ - create*()  │  │  Sampler, Pipeline)  │   │
│  │ - createDevice│ │ - draw*()    │  │                      │   │
│  └──────┬───────┘  └──────┬───────┘  └──────────────────────┘   │
│         │                  │                                       │
└─────────┼──────────────────┼───────────────────────────────────────┘
          │                  │
          ▼                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                    JNI Boundary (jlong handles)                  │
└─────────────────────────────────────────────────────────────────┘
          │                  │
          ▼                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                   Rust Native Library                            │
│                      (bassalt-native/)                             │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌──────────────────────┐     │
│  │   lib.rs    │  │  device.rs  │  │  Resource Modules    │     │
│  │ (JNI exports│  │ (core GPU   │  │  - buffer.rs         │     │
│  │  & entry)   │  │  state)     │  │  - texture.rs        │     │
│  └─────────────┘  └─────────────┘  │  - sampler.rs        │     │
│  ┌─────────────┐  ┌─────────────┐  │  - pipeline.rs       │     │
│  │   jni/      │  │   shader.rs │  │  - command.rs        │     │
│  │ (JNI utils) │  │ (naga GLSL→ │  └──────────────────────┘     │
│  └─────────────┘  │  WGSL)      │                                │
│  ┌─────────────┐  └─────────────┘                                │
│  │  context.rs │                                                    │
│  │  surface.rs │  ┌─────────────┐                                │
│  │  error.rs   │  │ wgpu-core   │                                │
│  └─────────────┘  │ 27.0        │                                │
│                   └─────────────┘                                │
└─────────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────┐
│                    wgpu-core + wgpu-hal                          │
│           (Vulkan / DX12 / Metal / OpenGL ES)                    │
└─────────────────────────────────────────────────────────────────┘
```

## Package Structure

### Java Layer (`src/main/java/com/criticalrange/bassalt/`)

```
com.criticalrange.bassalt/
├── Bassaltrenderer.java              # Main mod entry point
├── backend/
│   ├── BassaltBackend.java           # GpuBackend implementation
│   └── BassaltDevice.java            # GpuDevice implementation
├── shader/
│   └── WgslCompiler.java             # GLSL to WGSL shader translation
├── buffer/
│   └── BassaltBuffer.java            # Buffer wrapper
├── texture/
│   ├── BassaltTexture.java
│   ├── BassaltTextureView.java
│   └── BassaltSampler.java
├── pipeline/
│   ├── BassaltCommandEncoder.java
│   └── BassaltRenderPass.java
└── mixin/
    └── BackendSwapMixin.java         # Injects Bassalt into backend array
```

### Rust Native Library (`bassalt-native/src/`)

```
bassalt-native/src/
├── lib.rs                  # JNI exports and global state management
├── jni/
│   ├── mod.rs              # JNI utility module (logging, error conversion)
│   ├── env.rs              # JNIEnv wrapper
│   ├── strings.rs          # Java/Rust string conversion
│   └── handles.rs          # Handle management (jlong <-> pointers)
├── context.rs              # Global wgpu instance/context
├── adapter.rs              # GPU adapter selection
├── surface.rs              # Window surface integration
├── device.rs               # Core GPU device wrapper
├── buffer.rs               # Buffer management
├── texture.rs              # Texture and texture view
├── texture_and_view.rs     # Extended texture support
├── sampler.rs              # Sampler creation
├── pipeline.rs             # Render and compute pipelines
├── pipeline_registry.rs    # Pipeline caching system
├── shader.rs               # GLSL to WGSL translation (naga)
├── command.rs              # Command encoding
├── render_pass.rs          # Render pass state and recording
├── bind_group.rs           # Bind group creation and management
├── bind_group_layouts.rs   # Shared bind group layouts
├── resource_handles.rs     # Handle storage and validation
├── range_allocator.rs      # GPU resource range allocation
├── atlas.rs                # Texture atlas support
├── render_bundle.rs        # Render bundle encoding
├── timestamp_queries.rs    # GPU timestamp queries
├── msaa.rs                 # Multisample anti-aliasing
├── java_logger.rs          # Java logging bridge
└── error.rs                # Error types
```

## Key Implementation Details

### Backend Injection (Mixin System)

The `BackendSwapMixin` intercepts Minecraft's initialization and injects Bassalt into the GPU backend array:

```java
@Mixin(Minecraft.class)
public class BackendSwapMixin {
    @ModifyArg(method = "<init>", ...)
    private static GpuBackend[] bassalt$addBassaltBackend(GpuBackend[] original) {
        if (Boolean.getBoolean("bassalt.enabled")) {
            return new GpuBackend[]{
                new BassaltBackend(),  // Try Bassalt first
                new GlBackend()        // Fallback to OpenGL
            };
        }
        return original;
    }
}
```

**Enable Bassalt with**: `-Dbassalt.enabled=true`

### JNI Bridge Pattern

All Java objects use `jlong` handles to reference opaque Rust pointers:

```java
// Java side
private long ptr;  // Pointer to Rust struct

static native long createBuffer(long ptr, long size, int usage);
```

```rust
// Rust side
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_backend_BassaltDevice_createBuffer(
    mut env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    size: jlong,
    usage: jint,
) -> jlong {
    let device = unsafe { &*(ptr as *const BassaltDevice) };
    // ... create buffer and return pointer as jlong
}
```

### Shader Translation Pipeline

Minecraft uses pre-converted WGSL shaders stored in resource packs:

**Shader Location**: `src/main/resources/assets/bassaltrenderer/shaders/wgsl/`

**Shader Naming**: `<shader_name>_<type>.wgsl` (e.g., `position_tex_color_vs.wgsl`, `position_tex_color_fs.wgsl`)

**Shader Converter**: The Gradle build automatically runs `bassalt-native/src/bin/shader_converter.rs` to convert all GLSL shaders to WGSL. Manual conversion:
```bash
cd bassalt-native
cargo run --bin shader_converter -- ../src/main/resources/shaders ../src/main/resources/shaders/wgsl
```

The shader converter:
- Scans `src/main/resources/shaders/core/` and `src/main/resources/shaders/post/` for `.fsh`/`.vsh` files
- Preprocesses GLSL (removes `#version`, `#moj_import`, precision qualifiers)
- Converts to WGSL using naga
- Outputs to `src/main/resources/shaders/wgsl/core/` and `src/main/resources/shaders/wgsl/post/`

**Shader Reflection**: When creating render pipelines, naga parses the WGSL to extract:
- Binding layout (which slot expects texture/sampler/uniform)
- Binding types for type-safe bind group creation
- Uniform buffer struct sizes

### Bind Group Layout System

The bind group system uses shader reflection to ensure correct resource binding:

```rust
// resource_handles.rs
pub enum BindingLayoutType {
    Texture,
    Sampler,
    UniformBuffer,
    StorageBuffer,
}

pub struct BindingLayoutEntry {
    pub binding: u32,       // Slot number
    pub ty: BindingLayoutType, // Expected resource type
}

pub struct RenderPipelineInfo {
    pub id: id::RenderPipelineId,
    pub bind_group_layout_id: id::BindGroupLayoutId,
    pub binding_layouts: Vec<BindingLayoutEntry>, // What each slot expects
}
```

**Workflow**:
1. Pipeline creation extracts binding info from shader via naga reflection
2. `RenderPipelineInfo` stores the expected binding types
3. When creating bind groups, resources are matched to slots by type
4. Textures → Texture slots, Samplers → Sampler slots, Uniforms → Uniform slots

### Pipeline Caching

The `PipelineCache` in `pipeline_registry.rs` provides automatic caching of shader modules and render pipelines:

```rust
pub struct PipelineCache {
    shader_modules: RwLock<HashMap<u64, CachedShaderModule>>,
    render_pipelines: RwLock<HashMap<RenderPipelineKey, CachedRenderPipeline>>,
    stats: RwLock<CacheStats>,
}
```

**Benefits**:
- Shader modules compiled once and reused
- Redundant pipeline creations eliminated
- Faster startup with common pipelines cached
- Cache statistics for monitoring effectiveness

**Cache Key**: `RenderPipelineKey` includes:
- Vertex/fragment shader hashes
- Primitive topology
- Depth configuration (test, write, compare)
- Blend state
- Target format and depth format

### MSAA (Multisample Anti-Aliasing)

MSAA support is implemented in `msaa.rs` with the following features:

```rust
pub struct MSAAConfig {
    pub framebuffer_view_id: id::TextureViewId,
    pub framebuffer_texture_id: id::TextureId,
    pub sample_count: u32,  // 1, 2, 4, 8, or 16
    pub format: wgt::TextureFormat,
    pub width: u32,
    pub height: u32,
}
```

**Java API**:
```java
// Query maximum supported samples
int maxSamples = device.getMaxSupportedSamples(BassaltBackend.FORMAT_BGRA8);

// Create MSAA configuration
long msaaConfig = device.createMSAAConfig(width, height, format, sampleCount);

// Get actual sample count (may be clamped to max supported)
int actualSamples = device.getMSAASampleCount(msaaConfig);

// Destroy when done
device.destroyMSAAConfig(msaaConfig);
```

### Render Bundles

Render bundles allow recording commands once and replaying them multiple times:

```rust
// Create a render bundle encoder
let encoder = create_simple_encoder(&context, device_id, color_format, sample_count)?;

// Record commands (pipeline, vertex buffers, draw calls, etc.)
// ...

// Finish to get a bundle
let bundle = encoder.finish(device_id)?;
```

**Use cases**:
- Repeated rendering of the same geometry
- Optimized UI rendering
- Reduced CPU overhead for common draw patterns

### Timestamp Queries

GPU timestamp queries for performance profiling:

```java
// Create a query pool
long queryPool = device.createQueryPool();

// Write timestamps at various points
device.writeTimestamp(commandEncoder, queryPool, 0);  // Start
// ... do work ...
device.writeTimestamp(commandEncoder, queryPool, 1);  // End

// Resolve and read timestamps
long[] timestamps = device.resolveQueryPool(queryPool, 2);
```

### Depth Mode Tracking System

Bassalt implements a sophisticated depth mode tracking system that optimizes depth buffer usage based on the pipelines being rendered:

```rust
enum DepthMode {
    Unknown,   // Initial state, determined by first pipeline
    ReadOnly,  // Depth test enabled, no writes (transparent objects)
    Writable,  // Full depth test and write (opaque geometry)
    NoDepth,   // No depth attachment needed (GUI, post-processing)
}
```

**How it works**:

1. **First Pipeline Determines Mode**: When `record_set_pipeline()` is called for the first time in a render pass, the depth mode is locked based on the pipeline's depth configuration:
   - If `has_depth_output = false` → `NoDepth` (no depth attachment needed)
   - If `depth_write_enabled = true` → `Writable` (full depth testing)
   - If `depth_test_enabled = true` but `depth_write_enabled = false` → `ReadOnly` (depth-only)

2. **Subsequent Pipeline Validation**: Later pipelines are validated for compatibility with the established depth mode. Incompatible pipelines trigger warnings but don't fail (letting wgpu-core validate).

3. **Render Pass Depth Attachment**: When the render pass begins, the depth attachment is configured based on the tracked mode:
   ```rust
   let depth_read_only = matches!(self.depth_mode, DepthMode::ReadOnly);
   let (depth_load_op, depth_store_op) = if depth_read_only {
       (None, None)  // Read-only: no load/store operations
   } else {
       (Some(depth_load_op), Some(wgpu_core::command::StoreOp::Store))
   };
   ```

**Benefits**:
- **Optimized Depth Usage**: Transparent objects can render with read-only depth, allowing proper depth sorting
- **No Depth for GUI**: Post-processing and GUI rendering doesn't waste resources on unused depth attachments
- **Validation**: Early warnings about incompatible pipeline combinations

**Example Render Flow**:
```
1. Render opaque geometry (depth_mode = Writable)
   - Depth test: ON
   - Depth write: ON
   - Result: Proper depth buffer filled

2. Render transparent objects (depth_mode = ReadOnly, validated)
   - Depth test: ON (reads from existing depth buffer)
   - Depth write: OFF
   - Result: Correct back-to-front sorting

3. Render UI overlay (depth_mode = NoDepth, new render pass)
   - No depth attachment
   - Result: UI drawn on top without depth testing
```

### macOS-Specific Implementation

**Surface Creation**: macOS requires special handling for Metal:

```rust
// device.rs - macOS surface creation
#[cfg(target_os = "macos")]
{
    // GLFW returns NSWindow*, but wgpu/Metal needs NSView*
    // Must get contentView from NSWindow using Objective-C interop
    let ns_view = unsafe {
        use objc2::{msg_send, runtime::AnyObject};
        let ns_window = window_ptr as *mut AnyObject;
        let content_view: *mut AnyObject = msg_send![ns_window, contentView];
        content_view as *mut std::ffi::c_void
    };
    
    // Check if already on main thread to avoid deadlock
    let is_main: bool = unsafe { msg_send![class!(NSThread), isMainThread] };
    if is_main {
        // Create surface directly
    } else {
        // Dispatch to main queue synchronously
        dispatch::Queue::main().exec_sync(|| { ... });
    }
}
```

**Dependencies for macOS** (in Cargo.toml):
```toml
[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "0.5"
dispatch = "0.2"
```

### Type Mapping: Minecraft → WebGPU

The following constants are defined in `BassaltBackend.java`:

| Constant | WebGPU Format |
|----------|---------------|
| FORMAT_RGBA8 | TextureFormat::Rgba8UnormSrgb |
| FORMAT_BGRA8 | TextureFormat::Bgra8UnormSrgb |
| FORMAT_RGB8 | TextureFormat::Rgb8UnormSrgb |
| FORMAT_RG8 | TextureFormat::Rg8Unorm |
| FORMAT_R8 | TextureFormat::R8Unorm |
| FORMAT_RGBA16F | TextureFormat::Rgba16Float |
| FORMAT_RGBA32F | TextureFormat::Rgba32Float |
| FORMAT_DEPTH24 | TextureFormat::Depth24Plus |
| FORMAT_DEPTH32F | TextureFormat::Depth32Float |
| FORMAT_DEPTH24_STENCIL8 | TextureFormat::Depth24PlusStencil8 |

## External Dependencies and References

### Minecraft Source (`~/26.1-unobfuscated/`)

Fully decompiled Minecraft 26.1 source code for reference (if available):

**Key GPU Abstraction Classes:**
- `com.mojang.blaze3d.systems.GpuBackend` - Backend interface
- `com.mojang.blaze3d.systems.GpuDevice` - Device interface
- `com.mojang.blaze3d.systems.RenderSystem` - Render state management
- `com.mojang.blaze3d.shaders.ShaderSource` - Shader management
- `net.minecraft.client.renderer.*` - Main rendering classes

**Use this to understand:**
- Minecraft's GPU abstraction layer
- How resources are created and managed
- Rendering pipeline structure
- Shader format and requirements

## Development Guidelines

### Adding New Native Methods

1. **Java side**: Add native method declaration to appropriate class:
```java
public native long newNativeMethod(long ptr, int param);
```

2. **Rust side**: Add JNI export following naming convention in `bassalt-native/src/lib.rs`:
```rust
#[no_mangle]
pub extern "system" fn Java_com_criticalrange_bassalt_<ClassName>_<MethodName>(
    mut env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    param: jint,
) -> jlong {
    // Implementation
}
```

**Note**: JNI function names must match the full Java class path with underscores replacing dots.
- `com.criticalrange.bassalt.backend.BassaltBackend` becomes `Java_com_criticalrange_bassalt_backend_BassaltBackend`

### Adding New Mixins

1. Create mixin class in `com.criticalrange.bassalt.mixin`
2. Update `src/main/resources/basaltrenderer.mixins.json` (note: "basalt", not "bassalt"):
```json
{
  "required": true,
  "package": "com.criticalrange.bassalt.mixin",
  "compatibilityLevel": "JAVA_25",
  "mixins": [],
  "client": [
    "BackendSwapMixin",
    "YourNewMixin"
  ]
}
```

**Note**: The mixin file name is `basaltrenderer.mixins.json` (with one 's'), while the package uses `bassalt` (with double 's'). This is intentional and matches the mod ID `bassaltrenderer`.

### Debugging Native Code

```bash
# Enable Rust debug logging (via BASALT_DEBUG env var)
BASALT_DEBUG=1 ./gradlew runClient -Dbassalt.enabled=true

# Or use RUST_LOG for more control
RUST_LOG=debug ./gradlew runClient -Dbassalt.enabled=true

# Build with debug symbols
cd bassalt-native && cargo build

# Use gdb/lldb for native debugging
gdb --args java ... -Dbassalt.enabled=true
```

### Common Build Issues

1. **Shader converter not found**: Run `cd bassalt-native && cargo build --release --bin shader_converter` first
2. **Native library not found at runtime**: Check that `META-INF/native/libbassalt_native.so` (or .dll/.dylib) is in the JAR
3. **macOS deployment target**: The build sets `MACOSX_DEPLOYMENT_TARGET=10.15` - if building for older macOS, modify build.gradle
4. **Gradle daemon memory**: Default is 1GB (see gradle.properties `org.gradle.jvmargs=-Xmx1G`) - increase if builds fail with OOM
5. **JNI signature mismatch**: Ensure JNI function names exactly match the full package path with underscores

## Version Management

Mod versions and dependencies are managed in `gradle.properties`:
- `minecraft_version` - Target Minecraft version (26.1-snapshot-1)
- `loader_version` - Fabric Loader version (>=0.18.4)
- `loom_version` - Fabric Loom plugin version
- `mod_version` - Mod version string
- `fabric_version` - Fabric API version

Rust dependencies are in `bassalt-native/Cargo.toml`:
- `wgpu-core = "27.0"` - Core WebGPU implementation
- `wgpu-hal = "27.0"` - Hardware abstraction layer
- `naga = "27.0"` - Shader translation
- `jni = "0.21"` - JNI bindings (note: 0.21, not 0.22)
- `objc2 = "0.5"` - macOS Objective-C interop (macOS only)
- `dispatch = "0.2"` - macOS Grand Central Dispatch (macOS only)

**Rust Feature Flags** (in Cargo.toml):
```toml
default = ["metal", "vulkan", "glsl", "wgsl"]

# Backend features
vulkan = ["wgpu-core/vulkan"]  # Linux/Windows
metal = ["wgpu-core/metal"]    # macOS/iOS
dx12 = ["wgpu-core/dx12"]      # Windows
gles = ["wgpu-core/gles"]      # OpenGL ES fallback

# Shader language features
spirv = ["naga/spv-in", "wgpu-core/spirv"]
glsl = ["naga/glsl-in", "wgpu-core/glsl"]
wgsl = ["wgpu-core/wgsl"]
```

To build with different backends:
```bash
cd bassalt-native
cargo build --release --no-default-features --features "vulkan,glsl,wgsl"  # Vulkan only
cargo build --release --no-default-features --features "dx12,glsl,wgsl"   # DirectX 12 only
```

## Current Implementation Status

### ✅ Working Features
- **Device Creation**: Works on macOS (Metal), Linux (Vulkan), Windows (DX12)
- **Surface Creation**: Proper window handle extraction (NSView on macOS)
- **Buffer Creation**: Vertex, index, and uniform buffers
- **Texture/Sampler Creation**: 2D textures with samplers
- **Render Pipeline Creation**: From pre-converted WGSL shaders
- **Pipeline Caching**: Shader modules and render pipelines cached by hash
- **Shader Reflection**: Extracts binding layout from naga modules
- **Bind Group System**: Type-aware binding matches shader expectations
- **Multi-Bind-Group Support**: Separate groups for textures (0), uniforms (1, 2)
- **Render Pass Recording**: Commands recorded and submitted
- **Frame Presentation**: Swapchain acquire/present cycle
- **Per-Pipeline Depth Format Tracking**: `RenderPipelineInfo` tracks expected depth format
- **Conditional Depth State**: Pipelines without depth output get `depth_stencil: None`
- **Depth Mode Tracking**: Sophisticated depth write mode tracking per render pass
  - `ReadOnly`: Depth test enabled, no writes (transparent objects)
  - `Writable`: Full depth test and write (opaque geometry)
  - `NoDepth`: No depth attachment needed (GUI, post-processing)
- **Depth Texture Auto-Creation**: Automatically creates depth textures when MC doesn't provide them
- **Read-Only Depth Attachments**: Proper `load_op: None, store_op: None` for read-only depth
- **Pipeline Depth Validation**: Warns when pipelines have incompatible depth modes
- **Buffer Size Clamping**: Uniform buffers clamped to 64KB limit
- **Storage Buffer Fallback**: Large buffers (>64KB) use storage buffers
- **Shader Depth Detection**: `shader_writes_depth()` function analyzes naga module for FragDepth output
- **Index Buffer Validation**: Validates index count doesn't exceed buffer size
- **Texture Dimension Re-viewing**: Automatic view creation for dimension mismatches
- **MSAA Support**: Multisample anti-aliasing with dynamic sample count query
- **Render Bundles**: Command bundle recording for optimized replay
- **Timestamp Queries**: GPU timestamp queries for profiling
- **Depth-Only Render Passes**: Support for shadow rendering and depth pre-passes

### ⚠️ Known Issues (Non-Fatal)
1. **Buffer Size Mismatch**: Some shaders may expect larger uniform buffers than Minecraft provides
   - Error: `BindingSizeTooSmall(shader_size: X, bound_size: Y)`
   - Cause: WGSL uniform struct may be larger than Minecraft's actual data
   - Mitigation: Bindings with undersized buffers are skipped (not bound)
   - Fix: Reduce shader uniform sizes to match Minecraft's buffers, or pad buffers

### ✅ Recently Fixed
1. **Pipeline Depth State Mismatch** (2024): Fixed by returning `None` for pipelines without depth output
   - Root cause: Pipelines without depth were creating "no-op" depth_stencil state with `Depth32Float`
   - Impact: Format mismatch with render passes that had no depth attachment
   - Solution: `create_depth_stencil_state()` now returns `None` when `PipelineDepthFormat::None`
   - Result: GUI shaders (no depth output) no longer cause validation errors

2. **Empty Color Attachment Check** (2024): Fixed to allow depth-only render passes
   - Root cause: Render pass rejected when `color_attachments.is_empty()`
   - Impact: Shadow rendering and depth pre-passes failed
   - Solution: Only reject when BOTH color and depth attachments are empty
   - Result: Depth-only passes now work correctly

3. **Index Buffer Validation** (2024): Added bounds checking for draw calls
   - Validates `first_index + index_count` doesn't exceed buffer size
   - Validates offset is within buffer bounds
   - Prevents out-of-bounds GPU reads

4. **Texture View Creation Error Handling** (2024): Improved failure handling
   - When view creation fails, binding is now skipped instead of using wrong-dimension view
   - Prevents validation errors from dimension mismatches

5. **Depth Stencil Mismatch** (earlier): Fixed by ensuring pipeline and render pass agree on depth
   - `shader_writes_depth()` detects `@builtin(frag_depth)` in fragment shaders
   - Pipelines without depth output get `depth_stencil: None`

6. **Texture Dimension Mismatch** (earlier): Fixed by creating new views with expected dimension
   - Shader expects Cube texture but provided D2Array (6 layer cubemap)
   - `expected_dimension` added to `BindingLayoutEntry`
   - `build_with_layout()` creates new view if dimension mismatches

7. **Buffer Range Too Large** (earlier): Large buffers now use storage buffer binding type
   - Buffers >64KB are bound as read-only storage buffers instead of uniform buffers

8. **Animation Sprite Shaders** (earlier): Fixed MipMapLevel support
   - `animate_sprite_blit.frag.wgsl` and `animate_sprite_interpolate.frag.wgsl` now use `textureSampleLevel`

9. **Post-Processing Shaders** (earlier): All post-processing effects implemented
   - Implemented: blit, invert, bits, transparency, color_convolve, entity_sobel, rotscale, spiderclip

### 🔲 Not Yet Implemented
- Compute shaders (infrastructure exists, not fully integrated)
- Dynamic uniform buffer slicing (uses 64KB clamping instead)
- Ray tracing (WebGPU extension)
- RGSS (Rotated Grid Super-Sampling) for terrain shader (currently simplified)
- Pipeline disk caching for faster startup across runs

## Shader Implementation Status

### Core Shaders (✅ Complete)
All core rendering shaders have been implemented:
- Position-based: `position`, `position_color`, `position_tex`, `position_tex_color`
- Entity rendering: `entity`, `block`, `particle`, `gui`, `glint`
- World rendering: `terrain`, `sky`, `stars`, `rendertype_clouds`
- Special effects: `rendertype_end_portal`, `rendertype_beacon_beam`, `rendertype_lightning`
- Text rendering: `rendertype_text*`, `rendertype_text_intensity*`, `rendertype_text_background*`
- Transparency: `rendertype_entity_alpha`, `rendertype_translucent_moving_block`
- Utility: `rendertype_lines`, `rendertype_outline`, `rendertype_leash`, `rendertype_water_mask`
- Animation: `animate_sprite`, `animate_sprite_blit`, `animate_sprite_interpolate`

### Post-Processing Shaders (✅ Complete)
All post-processing effects implemented:
- `box_blur` - Gaussian blur for menus and effects
- `entity_outline_box_blur` - Entity outline rendering
- `blit` - Simple texture copy with color modulation
- `invert` - Color inversion effect
- `bits` - Pixelization and bit-depth reduction
- `transparency` - Multi-layer depth-based compositing
- `color_convolve` - Color matrix transformation
- `entity_sobel` - Edge detection using Sobel filter
- `rotscale` / `spiderclip` - Spider vision effects with rotation/scaling/vignette

### Known Shader Limitations
1. **Terrain RGSS**: The terrain shader uses simplified nearest-neighbor sampling instead of full RGSS (Rotated Grid Super-Sampling)
   - Original uses complex multi-sample anti-aliasing with derivative-based mip selection
   - Current implementation provides basic functionality without RGSS overhead
2. **Compute Shaders**: Infrastructure exists but not fully integrated for terrain generation

## Known Limitations

1. **Bindless Resources**: Uses bind groups (not bindless textures)
2. **Uniform Buffer Size**: Minecraft uniform buffers may not match shader expectations (mitigated by skipping undersized bindings)

## Important Notes and Naming Conventions

### Critical Naming Gotchas
- **Mod ID vs Package**: Mod ID is `bassaltrenderer` (in fabric.mod.json), package is `com.criticalrange.bassalt`
- **Mixin file**: `basaltrenderer.mixins.json` (one 's' in basalt) vs package `com.criticalrange.bassalt` (double 's')
- **Main class**: Entry point is `com.criticalrange.Bassaltrenderer` (capital B, note the package mismatch)
- **Native library**: The Rust library is named `libbassalt_native` (not bassaltrenderer)

### Technical Details
- **Package naming**: The Java package is `com.criticalrange.bassalt` but the native library is named `libbassalt_native`
- **Resource ID pattern**: wgpu-core uses resource IDs (like `BufferId`, `TextureId`) which are converted to/from `jlong` handles
- **Arc usage**: The global context uses `Arc` for reference counting; when extracting from raw pointers, remember to re-clone or forget as appropriate
- **GLSL preprocessing**: Minecraft's shader format requires preprocessing (removing `#version`, `#moj_import`, precision qualifiers) before naga translation
- **Binding slots**: Always check shader reflection to determine correct binding slot types - don't assume order
- **Buffer binding sizes**: Use explicit sizes (not `size: None`) to allow binding smaller buffers than shader declares

## Future Enhancements

- Full compute shader integration for terrain generation
- Ray tracing support (WebGPU extension)
- Async compute pipelines
- True dynamic uniform buffer slicing (currently uses 64KB clamping)
- Bindless textures for improved performance
- Pipeline disk caching for faster startup across runs

