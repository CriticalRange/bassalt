// End portal fragment shader
//
// All bindings in group 0 to match Bassalt's single bind group approach

struct DynamicUniforms {
    model_view: mat4x4<f32>,
    color_mod: vec4<f32>,
    model_offset: vec3<f32>,
    _pad0: f32,
    texture_mat: mat4x4<f32>,
}

// Group 0 bindings
@group(0) @binding(4) var<uniform> uniforms: DynamicUniforms;

@fragment
fn main() -> @location(0) vec4<f32> {
    // Use ColorModulator - end portal effect would need more complex shader
    return uniforms.color_mod;
}
