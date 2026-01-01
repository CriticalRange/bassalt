// GUI fragment shader - solid color rendering

struct DynamicTransforms {
    ModelViewMat: mat4x4<f32>,
    ColorModulator: vec4<f32>,
    ModelOffset: vec3<f32>,
    _pad0: f32,
    TextureMat: mat4x4<f32>,
}

@group(1) @binding(0) var<uniform> transforms: DynamicTransforms;

@fragment
fn main(@location(0) vertex_color: vec4<f32>) -> @location(0) vec4<f32> {
    var color = vertex_color;
    if (color.a == 0.0) {
        discard;
    }
    return color * transforms.ColorModulator;
}
