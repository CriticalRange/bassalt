// Text intensity see-through fragment shader
@group(0) @binding(0) var font_texture: texture_2d<f32>;
@group(0) @binding(1) var font_sampler: sampler;
struct FragmentInput {
    @location(0) vertex_color: vec4<f32>,
    @location(1) tex_coord: vec2<f32>,
}
@fragment
fn main(in: FragmentInput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(font_texture, font_sampler, in.tex_coord);
    let color = vec4<f32>(in.vertex_color.rgb, in.vertex_color.a * tex_color.r);
    if (color.a < 0.01) { discard; }
    return color;
}
