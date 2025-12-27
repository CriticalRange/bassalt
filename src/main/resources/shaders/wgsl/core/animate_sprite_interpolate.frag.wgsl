// Animate sprite interpolate fragment shader
// Converted from animate_sprite_interpolate.fsh

struct FragmentInput {
    @location(0) animation_progress: f32,
    @location(1) tex_coord0: vec2<f32>,
}

@group(0) @binding(1)
var current_sprite_texture: texture_2d<f32>;

@group(0) @binding(2)
var next_sprite_texture: texture_2d<f32>;

@group(0) @binding(3)
var sprite_sampler: sampler;

@fragment
fn main_fs(in: FragmentInput) -> @location(0) vec4<f32> {
    let current_color = textureSample(current_sprite_texture, sprite_sampler, in.tex_coord0);
    let next_color = textureSample(next_sprite_texture, sprite_sampler, in.tex_coord0);
    return mix(current_color, next_color, in.animation_progress);
}
