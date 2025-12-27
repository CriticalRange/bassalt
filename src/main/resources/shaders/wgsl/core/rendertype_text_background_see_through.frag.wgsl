// Text background see-through fragment shader
// Converted from rendertype_text_background_see_through.fsh

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
    let color = in.vertex_color;
    if (color.a < 0.1) {
        discard;
    }
    return color * dynamic_transforms.ColorModulator;
}
