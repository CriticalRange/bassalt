// Panorama vertex shader - cubemap rendering
// Matches MC's panorama.vsh
//
// All bindings in group 0 to match Bassalt's single bind group approach

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

// Group 0 bindings
@group(0) @binding(4) var<uniform> transforms: DynamicTransforms;
@group(0) @binding(5) var<uniform> projection: Projection;

struct VertexInput {
    @location(0) position: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec3<f32>,  // 3D for cubemap sampling
}

@vertex
fn main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = projection.ProjMat * transforms.ModelViewMat * vec4<f32>(in.position, 1.0);
    out.tex_coord = in.position;  // Use position as cubemap direction
    return out;
}
