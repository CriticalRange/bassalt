// Particle vertex shader
// Simplified layout
//
// Group layout:
// Group 0: Textures (particle texture)
// Group 1: DynamicTransforms
// Group 2: Projection

struct DynamicTransforms {
    ModelViewMat: mat4x4<f32>,
    ColorModulator: vec4<f32>,
    ModelOffset: vec3<f32>,
    _pad0: f32,
    TextureMat: mat4x4<f32>,
}

struct Projection {
    ProjMat: mat4x4<f32>,
}

// Group 0: Textures
@group(0) @binding(0) var Sampler0: texture_2d<f32>;
@group(0) @binding(1) var Sampler0Sampler: sampler;

// Group 1: DynamicTransforms
@group(1) @binding(0) var<uniform> transforms: DynamicTransforms;

// Group 2: Projection
@group(2) @binding(0) var<uniform> projection: Projection;

// Particle vertex format: POSITION_TEX_COLOR
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv0: vec2<f32>,
    @location(2) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) vertex_color: vec4<f32>,
}

@vertex
fn main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    out.position = projection.ProjMat * transforms.ModelViewMat * vec4<f32>(in.position, 1.0);
    out.vertex_color = in.color;
    out.tex_coord = in.uv0;
    
    return out;
}
