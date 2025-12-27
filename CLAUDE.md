# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Bassalt Renderer is a Minecraft Fabric mod targeting Minecraft version 26.1-snapshot-1. It implements a custom WebGPU-based rendering backend using wgpu-core (Rust) with a JNI bridge to Java.

**Key Architecture Decision**: This mod uses Rust + wgpu-core directly (not wgpu-native's C API) for memory safety and cleaner JNI integration. The native library is compiled as a cdylib and loaded via JNI.

## Build Commands

```bash
# Build the complete mod (Rust native library + Java code)
./gradlew build

# Build only the Rust native library (development)
cd bassalt-native && cargo build --release

# Clean all build artifacts
./gradlew clean

# Build with sources JAR
./gradlew build
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
│   ├── BassaltDevice.java            # GpuDevice implementation
│   └── ...
├── shader/
│   └── WgslCompiler.java            # GLSL to WGSL shader translation
├── mixin/
│   └── BackendSwapMixin.java        # Injects Bassalt into backend array
└── resources/
    ├── BassaltBuffer.java
    ├── BassaltTexture.java
    ├── BassaltTextureView.java
    ├── BassaltSampler.java
    ├── BassaltRenderPass.java
    └── BassaltCommandEncoder.java
```

### Rust Native Library (`bassalt-native/src/`)

```
bassalt-native/src/
├── lib.rs           # JNI exports and global state management
├── jni/
│   ├── mod.rs       # JNI utility module
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

Minecraft GLSL shaders are translated to WGSL via naga:

1. **Preprocess**: Remove `#version`, `#moj_import`, precision qualifiers
2. **Translate**: naga converts GLSL to WGSL
3. **Builtin Conversion**: Map GLSL builtins to WGSL equivalents
   - `gl_Position` -> `builtin(position)`
   - `gl_VertexID` -> `builtin(vertex_index)`
   - `gl_FragColor` -> return value

### Type Mapping: Minecraft → WebGPU

| Minecraft Format | WebGPU Format |
|-----------------|---------------|
| FormatRGBA | TextureFormat::Rgba8UnormSrgb |
| FormatRGB | TextureFormat::Rgb8UnormSrgb |
| FormatRG | TextureFormat::Rg8Unorm |
| FormatR | TextureFormat::R8Unorm |
| FormatRGBA16F | TextureFormat::Rgba16Float |
| FormatDepth32 | TextureFormat::Depth32Float |
| FormatDepth24Stencil8 | TextureFormat::Depth24PlusStencil8 |

## External Dependencies and References

### Minecraft Source (`~/26.1-unobfuscated/`)

Fully decompiled Minecraft 26.1 source code for reference:

**Key GPU Abstraction Classes:**
- `com.mojang.blaze3d.systems.GpuBackend` - Backend interface
- `com.mojang.blaze3d.systems.GpuDevice` - Device interface
- `com.mojang.blaze3d.systems.RenderSystem` - Render state management
- `net.minecraft.client.renderer.*` - Main rendering classes

**Use this to understand:**
- Minecraft's GPU abstraction layer
- How resources are created and managed
- Rendering pipeline structure
- Shader format and requirements

### wgpu-native (`~/wgpu-native/`)

Reference implementation for WebGPU in C:

**Key Files:**
- `ffi/wgpu.h` - C API header (interface reference)
- `examples/triangle/main.c` - Example usage
- `Cargo.toml` - Dependency versions (wgpu-core, naga)

**Use this for:**
- Understanding WebGPU API patterns
- Surface integration patterns
- Resource lifecycle management

## Development Guidelines

### Adding New Native Methods

1. **Java side**: Add native method declaration to appropriate class:
```java
public native long newNativeMethod(long ptr, int param);
```

2. **Rust side**: Add JNI export following naming convention:
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

3. **Register in lib.rs**: All JNI exports are in `bassalt-native/src/lib.rs`

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
# Enable Rust logging
RUST_LOG=debug ./gradlew runClient

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
- `naga = "27.0"` - Shader translation
- `jni = "0.22"` - JNI bindings

## Known Limitations

1. **Shader Coverage**: Not all Minecraft GLSL features are translated yet
2. **Compute Shaders**: Not yet implemented in native layer
3. **Multisampling**: Basic MSAA support only
4. **Bindless Resources**: Uses bind groups (not bindless textures)

## Future Enhancements

- Compute shader support for terrain generation
- Ray tracing support (WebGPU extension)
- Async compute pipelines
- Pipeline caching for faster startup
