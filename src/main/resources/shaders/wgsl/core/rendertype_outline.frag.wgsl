// Outline fragment shader
@group(0) @binding(0) var outline_texture: texture_2d<f32>;
@group(0) @binding(1) var outline_sampler: sampler;
struct FragmentInput {
    @location(0) vertex_color: vec4<f32>,
    @location(1) tex_coord: vec2<f32>,
}
@fragment
fn main(in: FragmentInput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(outline_texture, outline_sampler, in.tex_coord);
    return tex_color * in.vertex_color;
}
