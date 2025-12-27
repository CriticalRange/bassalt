// Blit screen fragment shader
// Converted from blit_screen.fsh

struct FragmentInput {
    @location(0) tex_coord: vec2<f32>,
}

@group(0) @binding(2)
var in_sampler_texture: texture_2d<f32>;

@group(0) @binding(3)
var in_sampler: sampler;

@fragment
fn main_fs(in: FragmentInput) -> @location(0) vec4<f32> {
    return textureSample(in_sampler_texture, in_sampler, in.tex_coord);
}
