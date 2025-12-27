// Text intensity see-through fragment shader
// Converted from rendertype_text_intensity_see_through.fsh

struct FragmentInput {
    @location(0) vertex_color: vec4<f32>,
    @location(1) tex_coord0: vec2<f32>,
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

@group(0) @binding(2)
var texture0: texture_2d<f32>;

@group(0) @binding(3)
var sampler0: sampler;

@fragment
fn main_fs(in: FragmentInput) -> @location(0) vec4<f32> {
    let tex_sample = textureSample(texture0, sampler0, in.tex_coord0);
    // Use red channel for all components (intensity mode)
    var color = vec4<f32>(tex_sample.r, tex_sample.r, tex_sample.r, tex_sample.r) * in.vertex_color;
    if (color.a < 0.1) {
        discard;
    }
    return color * dynamic_transforms.ColorModulator;
}
