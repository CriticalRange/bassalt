// Panorama fragment shader - samples cubemap texture
// Matches MC's panorama.fsh

@group(0) @binding(0)
var Sampler0: texture_cube<f32>;  // Cubemap texture

@group(0) @binding(1)
var Sampler0Sampler: sampler;

@fragment
fn main(@location(0) tex_coord: vec3<f32>) -> @location(0) vec4<f32> {
    return textureSample(Sampler0, Sampler0Sampler, tex_coord);
}
