// Blit post-processing fragment shader
@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;

@fragment
fn main(@location(0) tex_coord: vec2<f32>) -> @location(0) vec4<f32> {
    // DEBUG: Output solid magenta to verify blit pipeline works
    // If you see magenta, blit works. If you see clear color, blit is broken.
    return vec4<f32>(1.0, 0.0, 1.0, 1.0);
    
    // Original code:
    // let sampled = textureSample(input_texture, input_sampler, tex_coord);
    // return sampled;
}
