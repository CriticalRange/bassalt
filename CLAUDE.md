# CLAUDE.md

Summary
We've made major progress implementing the stub features:

✅ Successfully Fixed:
Pipeline-BindGroup Layout Matching - Bind groups now use the pipeline's expected layout
Binding Type Matching - Textures, samplers, and uniforms are now bound to correct slots based on shader reflection
macOS Threading - Surface creation works properly on main thread
⚠️ Remaining Issues (Non-Fatal):
Buffer Size Mismatches - Some Minecraft uniform buffers don't match shader expectations
Depth Stencil Attachment Mismatches - Some pipelines expect different depth formats
Buffer Too Large - Some buffers exceed uniform binding limits
These issues cause some draw calls to fail, but the game continues running. They're deeper architectural issues that require:

Proper buffer memory management
Dynamic uniform buffers with slicing
Per-pipeline depth stencil configuration

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Bassalt Renderer is a Minecraft Fabric mod targeting Minecraft version 26.1-snapshot-1. It implements a custom WebGPU-based rendering backend using wgpu-core (Rust) with a JNI bridge to Java.

**Key Architecture Decision**: This mod uses Rust + wgpu-core directly (not wgpu-native's C API) for memory safety and cleaner JNI integration. The native library is compiled as a cdylib and loaded via JNI.

**Important**: The package name is `com.criticalrange.bassalt` (note: bassalt, not basalt).

## Build Commands

```bash
# Build the complete mod (Rust native library + Java code)
./gradlew build

# Build only the Rust native library (development)
cd bassalt-native && cargo build --release

# Build debug version of native library
cd bassalt-native && cargo build

# Clean all build artifacts
./gradlew clean

# Run the mod with Bassalt enabled
./gradlew runClient -Dbassalt.enabled=true
```

**Important**: The build process automatically compiles the Rust native library via cargo before packaging the JAR. The native library (.so/.dll/.dylib) is included in the JAR under `META-INF/native/`.

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
├── lib.rs           # JNI exports and global state management
├── jni/
│   ├── mod.rs       # JNI utility module (logging, error conversion)
│   ├── env.rs       # JNIEnv wrapper
│   ├── strings.rs   # Java/Rust string conversion
│   └── handles.rs   # Handle management (jlong <-> pointers)
├── context.rs       # Global wgpu instance/context
├── adapter.rs       # GPU adapter selection
├── surface.rs       # Window surface integration
├── device.rs        # Core GPU device wrapper
├── buffer.rs        # Buffer management
├── texture.rs       # Texture and texture view
├── sampler.rs       # Sampler creation
├── pipeline.rs      # Render and compute pipelines
├── shader.rs        # GLSL to WGSL translation (naga)
├── command.rs       # Command encoding
└── error.rs         # Error types
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

**Shader Converter**: `bassalt-native/src/bin/shader_converter.rs` converts GLSL to WGSL:
```bash
cargo run --bin shader_converter -- <input_glsl> <output_wgsl> --type vertex|fragment
```

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
2. Update `src/main/resources/bassaltrenderer.mixins.json`:
```json
{
  "mixins": [
    "ExistingMixin",
    "NewMixin"
  ]
}
```

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
- `objc2 = "0.5"` - macOS Objective-C interop

## Current Implementation Status

### ✅ Working Features
- **Device Creation**: Works on macOS (Metal), Linux (Vulkan), Windows (DX12)
- **Surface Creation**: Proper window handle extraction (NSView on macOS)
- **Buffer Creation**: Vertex, index, and uniform buffers
- **Texture/Sampler Creation**: 2D textures with samplers
- **Render Pipeline Creation**: From pre-converted WGSL shaders
- **Shader Reflection**: Extracts binding layout from naga modules
- **Bind Group System**: Type-aware binding matches shader expectations
- **Render Pass Recording**: Commands recorded and submitted
- **Frame Presentation**: Swapchain acquire/present cycle

### ⚠️ Known Issues (Non-Fatal)
1. **Buffer Size Mismatch**: Some shaders expect larger uniform buffers than Minecraft provides
   - Error: `BindingSizeTooSmall(shader_size: 160, bound_size: 56)`
   - Cause: WGSL uniform struct is larger than Minecraft's actual data
   - Workaround: Non-fatal, draw call skipped
   
2. **Buffer Range Too Large**: Some vertex/instance buffers exceed uniform limits
   - Error: `BufferRangeTooLarge { given: 147712, limit: 65536 }`
   - Cause: Large buffers bound to uniform slots
   - Fix needed: Dynamic uniform buffer management
   
3. **Depth Stencil Mismatch**: Some pipelines expect different depth formats
   - Error: `IncompatibleDepthStencilAttachment { expected: None, actual: Some(Depth32Float) }`
   - Cause: Pipeline created without depth state, but render pass has depth attachment
   - Fix needed: Per-pipeline depth stencil configuration
   
4. **No Main Framebuffer Detection**: Present() sometimes has nothing to show
   - Warning: `No main framebuffer detected - nothing to present`
   - Cause: Render pass targeting non-swapchain texture
   - Fix needed: Better main render target tracking

### 🔲 Not Yet Implemented
- Compute shaders
- Multisampling (MSAA)
- Dynamic uniform buffer slicing
- Pipeline caching
- Ray tracing

## Known Limitations

1. **Shader Coverage**: Pre-converted WGSL shaders must be provided for each Minecraft shader
2. **Compute Shaders**: Not yet implemented in native layer
3. **Multisampling**: Basic MSAA support only
4. **Bindless Resources**: Uses bind groups (not bindless textures)
5. **Uniform Buffer Size**: Minecraft uniform buffers may not match shader expectations

## Important Notes

- **Package naming**: The Java package is `com.criticalrange.bassalt` but the native library is named `libbassalt_native`
- **Resource ID pattern**: wgpu-core uses resource IDs (like `BufferId`, `TextureId`) which are converted to/from `jlong` handles
- **Arc usage**: The global context uses `Arc` for reference counting; when extracting from raw pointers, remember to re-clone or forget as appropriate
- **GLSL preprocessing**: Minecraft's shader format requires preprocessing (removing `#version`, `#moj_import`, precision qualifiers) before naga translation
- **Binding slots**: Always check shader reflection to determine correct binding slot types - don't assume order

## Future Enhancements

- Compute shader support for terrain generation
- Ray tracing support (WebGPU extension)
- Async compute pipelines
- Pipeline caching for faster startup
- Dynamic uniform buffer management for proper sizing
- Per-pipeline depth stencil configuration
