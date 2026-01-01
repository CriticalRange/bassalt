// Position vertex shader
// Uniform layout:
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

@group(1) @binding(0) var<uniform> transforms: DynamicTransforms;
@group(2) @binding(0) var<uniform> projection: Projection;

struct VertexInput {
    @location(0) position: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
}

@vertex
fn main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = projection.ProjMat * transforms.ModelViewMat * vec4<f32>(in.position, 1.0);
    return out;
}
