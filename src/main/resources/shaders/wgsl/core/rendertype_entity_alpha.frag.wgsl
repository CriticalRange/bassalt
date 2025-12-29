// Entity variant fragment shader
@group(0) @binding(0) var entity_texture: texture_2d<f32>;
@group(0) @binding(1) var entity_sampler: sampler;
struct FragmentInput {
    @location(0) vertex_color: vec4<f32>,
    @location(1) tex_coord: vec2<f32>,
}
@fragment
fn main(in: FragmentInput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(entity_texture, entity_sampler, in.tex_coord);
    let color = tex_color * in.vertex_color;
    if (color.a < 0.01) { discard; }
    return color;
}
