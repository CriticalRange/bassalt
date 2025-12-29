// Blit screen fragment shader - samples texture and outputs

@group(0) @binding(0)
var source_texture: texture_2d<f32>;

@group(0) @binding(1)
var source_sampler: sampler;

@fragment
fn main(@location(0) tex_coord: vec2<f32>) -> @location(0) vec4<f32> {
    return textureSample(source_texture, source_sampler, tex_coord);
}
