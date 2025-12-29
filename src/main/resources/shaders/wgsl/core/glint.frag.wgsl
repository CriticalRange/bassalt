// Glint fragment shader
@group(0) @binding(0) var glint_texture: texture_2d<f32>;
@group(0) @binding(1) var glint_sampler: sampler;
@fragment
fn main(@location(0) tex_coord: vec2<f32>) -> @location(0) vec4<f32> {
    return textureSample(glint_texture, glint_sampler, tex_coord);
}
