// Animate sprite fragment shader (stub for blit operation)

struct FragmentInput {
    @location(0) animation_progress: f32,
    @location(1) tex_coord0: vec2<f32>,
}

@group(0) @binding(1)
var sprite_texture: texture_2d<f32>;

@group(0) @binding(2)
var sprite_sampler: sampler;

@fragment
fn main_fs(in: FragmentInput) -> @location(0) vec4<f32> {
    return textureSample(sprite_texture, sprite_sampler, in.tex_coord0);
}
