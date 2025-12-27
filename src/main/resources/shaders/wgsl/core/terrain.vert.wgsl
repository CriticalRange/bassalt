// Terrain vertex shader
// Converted from terrain.vsh

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv0: vec2<f32>,
    @location(3) uv2: vec2<f32>,
    @location(4) normal: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) spherical_vertex_distance: f32,
    @location(1) cylindrical_vertex_distance: f32,
    @location(2) vertex_color: vec4<f32>,
    @location(3) tex_coord0: vec2<f32>,
}

struct DynamicTransforms {
    ModelViewMat: mat4x4<f32>,
    ColorModulator: vec4<f32>,
    ModelOffset: vec3<f32>,
    _padding: f32,
    TextureMat: mat4x4<f32>,
}

struct Projection {
    ProjMat: mat4x4<f32>,
}

struct Globals {
    CameraBlockPos: vec3<i32>,
    _padding0: i32,
    CameraOffset: vec3<f32>,
    _padding1: f32,
    ScreenSize: vec2<f32>,
    GlintAlpha: f32,
    GameTime: f32,
    MenuBlurRadius: i32,
    UseRgss: i32,
}

struct ChunkSection {
    ChunkPosition: vec3<f32>,
    _padding0: f32,
    ChunkVisibility: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
    TextureSize: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> dynamic_transforms: DynamicTransforms;

@group(0) @binding(1)
var<uniform> projection: Projection;

@group(0) @binding(5)
var<uniform> globals: Globals;

@group(0) @binding(6)
var<uniform> chunk_section: ChunkSection;

// Fog utility functions
fn fog_spherical_distance(pos: vec3<f32>) -> f32 {
    return length(pos);
}

fn fog_cylindrical_distance(pos: vec3<f32>) -> f32 {
    let dist_xz = length(pos.xz);
    let dist_y = abs(pos.y);
    return max(dist_xz, dist_y);
}

@vertex
fn main_vs(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let camera_block_pos = vec3<f32>(f32(globals.CameraBlockPos.x), f32(globals.CameraBlockPos.y), f32(globals.CameraBlockPos.z));
    let pos = in.position + (chunk_section.ChunkPosition - camera_block_pos) + globals.CameraOffset;
    out.position = projection.ProjMat * dynamic_transforms.ModelViewMat * vec4<f32>(pos, 1.0);
    out.spherical_vertex_distance = fog_spherical_distance(pos);
    out.cylindrical_vertex_distance = fog_cylindrical_distance(pos);
    // Simplified: use vertex color directly
    out.vertex_color = in.color;
    out.tex_coord0 = in.uv0;
    return out;
}
