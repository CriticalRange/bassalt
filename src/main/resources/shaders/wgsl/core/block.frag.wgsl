// Block fragment shader

struct DynamicUniforms {
    model_view: mat4x4<f32>,
    color_mod: vec4<f32>,
    model_offset: vec3<f32>,
    _pad0: f32,
    texture_mat: mat4x4<f32>,
}

@group(0) @binding(0) var block_texture: texture_2d<f32>;
@group(0) @binding(1) var block_sampler: sampler;
@group(1) @binding(0) var<uniform> uniforms: DynamicUniforms;

struct FragmentInput {
    @location(0) vertex_color: vec4<f32>,
    @location(1) tex_coord: vec2<f32>,
}

@fragment
fn main(in: FragmentInput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(block_texture, block_sampler, in.tex_coord);
    let color = tex_color * in.vertex_color * uniforms.color_mod;
    if (color.a < 0.01) { discard; }
    return color;
}
