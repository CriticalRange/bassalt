// Stars fragment shader
//
// All bindings in group 0 to match Bassalt's single bind group approach

struct DynamicTransforms_t {
    ModelViewMat: mat4x4<f32>,
    ColorModulator: vec4<f32>,
    ModelOffset: vec3<f32>,
    _pad0: f32,
    TextureMat: mat4x4<f32>,
}

struct FragmentOutput {
    @location(0) fragColor: vec4<f32>,
}

@group(0) @binding(4) var<uniform> DynamicTransforms: DynamicTransforms_t;

@fragment
fn main() -> FragmentOutput {
    let fragColor = DynamicTransforms.ColorModulator;
    return FragmentOutput(fragColor);
}
