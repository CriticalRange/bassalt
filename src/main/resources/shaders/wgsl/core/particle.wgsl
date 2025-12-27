// Stub shader - GLSL conversion failed
// This file contains both vertex and fragment stages

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv0: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) uv2: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord0: vec2<f32>,
    @location(1) vertex_color: vec4<f32>,
    @location(2) tex_coord2: vec2<f32>,
}

struct DynamicTransforms {
    ModelViewMat: mat4x4<f32>,
    ColorModulator: vec4<f32>,
    ModelOffset: vec3<f32>,
    TextureMat: mat4x4<f32>,
}

struct Projection {
    ProjMat: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> dynamic_transforms: DynamicTransforms;

@group(0) @binding(1)
var<uniform> projection: Projection;

@vertex
fn main_vs(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = projection.ProjMat * dynamic_transforms.ModelViewMat * vec4<f32>(in.position, 1.0);
    out.tex_coord0 = in.uv0;
    out.vertex_color = in.color;
    out.tex_coord2 = in.uv2;
    return out;
}

struct FragmentInput {
    @location(0) tex_coord0: vec2<f32>,
    @location(1) vertex_color: vec4<f32>,
    @location(2) tex_coord2: vec2<f32>,
}

@fragment
fn main_fs(in: FragmentInput) -> @location(0) vec4<f32> {
    return in.vertex_color * dynamic_transforms.ColorModulator;
}
