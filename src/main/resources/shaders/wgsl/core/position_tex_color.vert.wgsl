// Position-Texture-Color vertex shader
// Multi-bind-group layout:
// Group 0: Textures/samplers
// Group 1: DynamicTransforms (uniform buffer - MC creates UNIFORM buffers)
// Group 2: Projection (uniform buffer)

// DynamicTransforms - matches Minecraft's 256-byte layout
// std140 layout: mat4(64) + vec4(16) + vec3(16 padded) + mat4(64) = 160 bytes
// MC sends 256 bytes, so there's additional data or padding
struct DynamicUniforms {
    model_view: mat4x4<f32>,     // 64 bytes (offset 0)
    color_mod: vec4<f32>,        // 16 bytes (offset 64)
    model_offset: vec3<f32>,     // 12 bytes (offset 80)
    _pad0: f32,                  // 4 bytes  (offset 92)
    texture_mat: mat4x4<f32>,    // 64 bytes (offset 96)
    // Additional padding to reach 256 bytes
    _reserved: array<vec4<f32>, 6>,  // 96 bytes (offset 160)
}

// Projection uniform
struct ProjectionUniform {
    proj_mat: mat4x4<f32>,
}

// Group 0: Textures (texture at binding 0, sampler at binding 1)
@group(0) @binding(0) var Sampler0: texture_2d<f32>;
@group(0) @binding(1) var Sampler0Sampler: sampler;

// Group 1: DynamicTransforms as uniform buffer (MC creates UNIFORM buffers)
@group(1) @binding(0) var<uniform> uniforms: DynamicUniforms;

// Group 2: Projection as uniform buffer
@group(2) @binding(0) var<uniform> projection: ProjectionUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) vertex_color: vec4<f32>,
}

@vertex
fn main(in: VertexInput, @builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    
    // DEBUG: Output fullscreen triangle
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0)
    );
    let pos = positions[vertex_index % 3u];
    out.position = vec4<f32>(pos, 0.0, 1.0);
    out.tex_coord = in.tex_coord;
    
    // DEBUG: Show first column of model_view + add 0.2 baseline to verify shader runs
    let col0 = uniforms.model_view[0];
    out.vertex_color = vec4<f32>(
        abs(col0.x) + 0.2,  // R = mv[0][0] + baseline (expect 1.2 for identity)
        abs(col0.y) + 0.2,  // G = mv[0][1] + baseline (expect 0.2)
        abs(col0.z) + 0.2,  // B = mv[0][2] + baseline (expect 0.2)
        1.0
    );
    return out;
}
