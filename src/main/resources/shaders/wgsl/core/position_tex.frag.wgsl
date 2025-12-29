// Position-Texture fragment shader

@group(0) @binding(0)
var Sampler0: texture_2d<f32>;

@group(0) @binding(1)
var Sampler0Sampler: sampler;

@fragment
fn main(@location(0) tex_coord: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(Sampler0, Sampler0Sampler, tex_coord);
    if (color.a < 0.01) {
        discard;
    }
    return color;
}
