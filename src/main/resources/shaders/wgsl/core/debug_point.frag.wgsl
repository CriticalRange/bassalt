// Debug point fragment shader
// Simple fragment shader for debug point rendering

struct FragmentInput {
    @location(0) vertex_color: vec4<f32>,
}

struct DynamicTransforms {
    ModelViewMat: mat4x4<f32>,
    ColorModulator: vec4<f32>,
    ModelOffset: vec3<f32>,
    _padding: f32,
    TextureMat: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> dynamic_transforms: DynamicTransforms;

@fragment
fn main_fs(in: FragmentInput) -> @location(0) vec4<f32> {
    return in.vertex_color * dynamic_transforms.ColorModulator;
}
