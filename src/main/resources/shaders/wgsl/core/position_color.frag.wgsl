// Position-Color fragment shader

struct DynamicUniforms {
    model_view: mat4x4<f32>,
    color_mod: vec4<f32>,
    model_offset: vec3<f32>,
    _pad0: f32,
    texture_mat: mat4x4<f32>,
}

@group(1) @binding(0) var<uniform> uniforms: DynamicUniforms;

@fragment
fn main(@location(0) vertex_color: vec4<f32>) -> @location(0) vec4<f32> {
    if (vertex_color.a < 0.01) {
        discard;
    }
    return vertex_color * uniforms.color_mod;
}
