// Crumbling fragment shader
@group(0) @binding(0) var crumbling_texture: texture_2d<f32>;
@group(0) @binding(1) var crumbling_sampler: sampler;
@fragment
fn main(@location(0) tex_coord: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(crumbling_texture, crumbling_sampler, tex_coord);
    if (color.a < 0.01) { discard; }
    return color;
}
