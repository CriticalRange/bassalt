// Position-Texture-Color fragment shader

struct DynamicUniforms {
    model_view: mat4x4<f32>,
    color_mod: vec4<f32>,
    model_offset: vec3<f32>,
    _pad0: f32,
    texture_mat: mat4x4<f32>,
}

@group(0) @binding(0) var Sampler0: texture_2d<f32>;
@group(0) @binding(1) var Sampler0Sampler: sampler;
@group(1) @binding(0) var<uniform> uniforms: DynamicUniforms;

struct FragmentInput {
    @location(0) tex_coord: vec2<f32>,
    @location(1) vertex_color: vec4<f32>,
}

@fragment
fn main(in: FragmentInput) -> @location(0) vec4<f32> {
    let color = textureSample(Sampler0, Sampler0Sampler, in.tex_coord) * in.vertex_color;
    if (color.a < 0.01) {
        discard;
    }
    return color * uniforms.color_mod;
}
