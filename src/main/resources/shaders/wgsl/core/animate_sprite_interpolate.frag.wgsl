// Animate sprite interpolate fragment shader (animate_sprite_interpolate)
//
// Matches GLSL: uniform sampler2D CurrentSprite; uniform sampler2D NextSprite;
//
// All bindings in group 0 to match Bassalt's single bind group approach

// Group 0 bindings
@group(0) @binding(0) var CurrentSprite: texture_2d<f32>;
@group(0) @binding(1) var CurrentSpriteSampler: sampler;
@group(0) @binding(2) var NextSprite: texture_2d<f32>;
@group(0) @binding(3) var NextSpriteSampler: sampler;

struct FragmentInput {
    @location(0) tex_coord: vec2<f32>,
}

@fragment
fn main(in: FragmentInput) -> @location(0) vec4<f32> {
    // Matches GLSL: mix(textureLod(CurrentSprite, texCoord0, MipMapLevel), textureLod(NextSprite, texCoord0, MipMapLevel), fAnimationProgress);
    let current_color = textureSampleLevel(CurrentSprite, CurrentSpriteSampler, in.tex_coord, 0.0);
    let next_color = textureSampleLevel(NextSprite, NextSpriteSampler, in.tex_coord, 0.0);
    // TODO: Need fAnimationProgress as a uniform
    return current_color;
}
