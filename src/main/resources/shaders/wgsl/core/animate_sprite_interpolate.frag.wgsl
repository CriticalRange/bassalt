// Animate sprite interpolate fragment shader
@group(0) @binding(0) var sprite_texture: texture_2d<f32>;
@group(0) @binding(1) var sprite_sampler: sampler;
@fragment
fn main(@location(0) tex_coord: vec2<f32>) -> @location(0) vec4<f32> {
    return textureSample(sprite_texture, sprite_sampler, tex_coord);
}
