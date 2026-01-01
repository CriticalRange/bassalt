// Panorama fragment shader - samples cubemap texture

@group(0) @binding(0) var panorama_texture: texture_cube<f32>;
@group(0) @binding(1) var panorama_sampler: sampler;

@fragment
fn main(@location(0) tex_coord: vec3<f32>) -> @location(0) vec4<f32> {
    return textureSample(panorama_texture, panorama_sampler, tex_coord);
}
