// Text background see-through fragment shader (GUI without fog/lightmap)

struct DynamicTransforms_t {
    ModelViewMat: mat4x4<f32>,
    ColorModulator: vec4<f32>,
    ModelOffset: vec3<f32>,
    _pad0: f32,
    TextureMat: mat4x4<f32>,
}

struct Projection_t {
    ProjMat: mat4x4<f32>,
}

// Group 0 bindings
@group(0) @binding(4) var<uniform> DynamicTransforms: DynamicTransforms_t;
@group(0) @binding(5) var<uniform> Projection: Projection_t;

@fragment
fn main(@location(0) vertex_color: vec4<f32>) -> @location(0) vec4<f32> {
    let color = vertex_color * DynamicTransforms.ColorModulator;
    if (color.a < 0.01) { discard; }
    return color;
}
