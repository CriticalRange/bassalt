// Blit post-processing fragment shader
// Matches Minecraft GLSL: fragColor = texture(InSampler, texCoord) * ColorModulate;

struct BlitConfig {
    ColorModulate: vec4<f32>,
}

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;
@group(0) @binding(2) var<uniform> config: BlitConfig;

@fragment
fn main(@location(0) tex_coord: vec2<f32>) -> @location(0) vec4<f32> {
    let sampled = textureSample(input_texture, input_sampler, tex_coord);
    return sampled * config.ColorModulate;
}
