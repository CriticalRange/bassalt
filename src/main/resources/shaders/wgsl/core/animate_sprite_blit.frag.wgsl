// Animate sprite blit fragment shader (animate_sprite_blit)
//
// Matches GLSL: uniform sampler2D Sprite;
//
// All bindings in group 0 to match Bassalt's single bind group approach

// Group 0 bindings
@group(0) @binding(0) var Sprite: texture_2d<f32>;
@group(0) @binding(1) var SpriteSampler: sampler;

struct FragmentInput {
    @location(0) tex_coord: vec2<f32>,
}

@fragment
fn main(in: FragmentInput) -> @location(0) vec4<f32> {
    // Matches GLSL: fragColor = textureLod(Sprite, texCoord0, MipMapLevel);
    return textureSampleLevel(Sprite, SpriteSampler, in.tex_coord, 0.0);
}
