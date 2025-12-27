// Animate sprite blit fragment shader
// Converted from animate_sprite_blit.fsh
// Uses bindings 2 and 3 to avoid conflict with vertex shader's uniform at binding 0
// Input matches animate_sprite.vert.wgsl outputs

struct FragmentInput {
    // Match vertex shader outputs:
    @location(0) animation_progress: f32,  // Unused but must match VS output
    @location(1) tex_coord0: vec2<f32>,
}

@group(0) @binding(2)
var sprite_texture: texture_2d<f32>;

@group(0) @binding(3)
var sprite_sampler: sampler;

@fragment
fn main_fs(in: FragmentInput) -> @location(0) vec4<f32> {
    return textureSample(sprite_texture, sprite_sampler, in.tex_coord0);
}
