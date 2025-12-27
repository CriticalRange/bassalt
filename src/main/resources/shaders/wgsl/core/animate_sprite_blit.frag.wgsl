// Animate sprite blit fragment shader
// Converted from animate_sprite_blit.fsh

struct FragmentInput {
    @location(0) tex_coord0: vec2<f32>,
}

@group(0) @binding(0)
var sprite_texture: texture_2d<f32>;

@group(0) @binding(1)
var sprite_sampler: sampler;

@fragment
fn main_fs(in: FragmentInput) -> @location(0) vec4<f32> {
    return textureSample(sprite_texture, sprite_sampler, in.tex_coord0);
}
