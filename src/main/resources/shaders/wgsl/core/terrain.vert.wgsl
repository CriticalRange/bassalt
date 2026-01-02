// Terrain vertex shader
// Converted from Minecraft GLSL with proper types
//
// All bindings in group 0 to match Bassalt's single bind group approach

struct DynamicTransforms_t {
    ModelViewMat: mat4x4<f32>,  // offset 0,  size 64
    ColorModulator: vec4<f32>,   // offset 64, size 16
    ModelOffset: vec3<f32>,      // offset 80, size 12
    _pad0: f32,                  // offset 92, size 4  (padding)
    TextureMat: mat4x4<f32>,     // offset 96, size 64
}  // total: 160 bytes

struct Projection_t {
    ProjMat: mat4x4<f32>,        // offset 0, size 64
}  // total: 64 bytes

// ChunkSection matches GLSL std140 layout exactly (96 bytes)
struct ChunkSection {
    ModelViewMat: mat4x4<f32>,    // offset 0,  size 64
    ChunkVisibility: f32,         // offset 64, size 4
    _pad0: vec2<f32>,             // offset 68, size 8  (padding for alignment - GLSL uses ivec2 here)
    ChunkPosition: vec3<i32>,     // offset 80, size 12 (GLSL uses ivec3)
    _pad1: f32,                   // offset 92, size 4  (padding to 96)
}

// Globals matches GLSL std140 layout exactly:
// ivec3 CameraBlockPos (12+4 pad) + vec3 CameraOffset (12+4 pad) + vec2 ScreenSize (8+8 pad)
// + 4 floats (16) = 64 bytes
struct Globals {
    CameraBlockPos: vec3<i32>,   // offset 0,  size 12
    _pad0: i32,                  // offset 12, size 4  (padding to 16)
    CameraOffset: vec3<f32>,      // offset 16, size 12
    _pad1: f32,                  // offset 28, size 4  (padding to 32)
    ScreenSize: vec2<f32>,        // offset 32, size 8
    _pad2: vec2<f32>,            // offset 40, size 8  (padding to 48)
    GlintAlpha: f32,             // offset 48, size 4
    GameTime: f32,               // offset 52, size 4
    MenuBlurRadius: i32,         // offset 56, size 4
    UseRgss: i32,                // offset 60, size 4
}  // total: 64 bytes

// Fog matches GLSL std140 layout exactly:
// vec4 FogColor (16) + 6 floats (24) = 40 bytes, rounded to 16-byte boundary = 48 bytes
struct Fog_t {
    FogColor: vec4<f32>,              // offset 0,  size 16
    FogEnvironmentalStart: f32,       // offset 16, size 4
    FogEnvironmentalEnd: f32,         // offset 20, size 4
    FogRenderDistanceStart: f32,      // offset 24, size 4
    FogRenderDistanceEnd: f32,        // offset 28, size 4
    FogSkyEnd: f32,                  // offset 32, size 4
    FogCloudsEnd: f32,               // offset 36, size 4
    _pad3: f32,                      // offset 40, size 4  (padding)
    _pad4: f32,                      // offset 44, size 4  (padding to 48)
}  // total: 48 bytes

// All bindings in group 0 with different binding indices
@group(0) @binding(0) var Sampler0: texture_2d<f32>;
@group(0) @binding(1) var Sampler0Sampler: sampler;
@group(0) @binding(2) var Sampler2: texture_2d<f32>;
@group(0) @binding(3) var Sampler2Sampler: sampler;
@group(0) @binding(4) var<uniform> DynamicTransforms: DynamicTransforms_t;
@group(0) @binding(5) var<uniform> Projection: Projection_t;
@group(0) @binding(6) var<uniform> chunk_section: ChunkSection;
@group(0) @binding(7) var<uniform> globals: Globals;
@group(0) @binding(8) var<uniform> Fog: Fog_t;

// Vertex format: POSITION_COLOR_TEX_TEX_NORMAL
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv0: vec2<f32>,
    @location(3) uv2: vec2<f32>,  // Lightmap coordinates (stored as floats in buffer)
    @location(4) normal: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) vertex_color: vec4<f32>,
    @location(2) spherical_dist: f32,
    @location(3) cylindrical_dist: f32,
}

fn fog_spherical_distance(pos: vec3<f32>) -> f32 {
    return length(pos);
}

fn fog_cylindrical_distance(pos: vec3<f32>) -> f32 {
    let distXZ = length(pos.xz);
    let distY = abs(pos.y);
    return max(distXZ, distY);
}

fn minecraft_sample_lightmap(uv: vec2<f32>) -> vec4<f32> {
    let uv_clamped = clamp((uv / 256.0) + 0.5 / 16.0, vec2<f32>(0.5 / 16.0), vec2<f32>(15.5 / 16.0));
    return textureSampleLevel(Sampler2, Sampler2Sampler, uv_clamped, 0.0);
}

@vertex
fn main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let pos = in.position + vec3<f32>(chunk_section.ChunkPosition - globals.CameraBlockPos) + globals.CameraOffset;
    out.position = Projection.ProjMat * DynamicTransforms.ModelViewMat * vec4<f32>(pos, 1.0);

    out.spherical_dist = fog_spherical_distance(pos);
    out.cylindrical_dist = fog_cylindrical_distance(pos);

    let lightmapColor = minecraft_sample_lightmap(in.uv2);
    out.vertex_color = in.color * lightmapColor;

    out.tex_coord = in.uv0;

    return out;
}
