// Lightmap fragment shader
@group(0) @binding(0) var lightmap_texture: texture_2d<f32>;
@group(0) @binding(1) var lightmap_sampler: sampler;
@fragment
fn main(@location(0) tex_coord: vec2<f32>) -> @location(0) vec4<f32> {
    return textureSample(lightmap_texture, lightmap_sampler, tex_coord);
}
