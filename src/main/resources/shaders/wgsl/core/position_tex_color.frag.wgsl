// Position-Texture-Color fragment shader

struct FragmentInput {
    @location(0) tex_coord: vec2<f32>,
    @location(1) vertex_color: vec4<f32>,
}

@group(0) @binding(0)
var Sampler0: texture_2d<f32>;

@group(0) @binding(1)
var Sampler0Sampler: sampler;

@fragment
fn main(in: FragmentInput) -> @location(0) vec4<f32> {
    // DEBUG: Output vertex color which encodes MVP-transformed coordinates
    // Expected for correct coords: gradient colors (not solid black/white/saturated)
    // R=0.5,G=0.5 = center of screen in clip space
    // Solid white/black/saturated = coords way outside -1 to 1 range
    return in.vertex_color;
}
