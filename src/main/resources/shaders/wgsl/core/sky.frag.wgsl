// Sky fragment shader
//
// All bindings in group 0 to match Bassalt's single bind group approach

struct DynamicTransforms_t {
    ModelViewMat: mat4x4<f32>,
    ColorModulator: vec4<f32>,
    ModelOffset: vec3<f32>,
    _pad0: f32,
    TextureMat: mat4x4<f32>,
}

// Group 0 bindings
@group(0) @binding(4) var<uniform> DynamicTransforms: DynamicTransforms_t;

@fragment
fn main() -> @location(0) vec4<f32> {
    // Use ColorModulator for sky color (MC passes sky color through this)
    return DynamicTransforms.ColorModulator;
}
