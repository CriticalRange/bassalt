// World border vertex shader
// Converted from rendertype_world_border.vsh

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv0: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord0: vec2<f32>,
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

@group(0) @binding(0)
var<uniform> dynamic_transforms: DynamicTransforms;

@group(0) @binding(1)
var<uniform> projection: Projection;

@vertex
fn main_vs(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let pos = in.position + dynamic_transforms.ModelOffset;
    out.position = projection.ProjMat * dynamic_transforms.ModelViewMat * vec4<f32>(pos, 1.0);
    out.tex_coord0 = (dynamic_transforms.TextureMat * vec4<f32>(in.uv0, 0.0, 1.0)).xy;
    return out;
}
