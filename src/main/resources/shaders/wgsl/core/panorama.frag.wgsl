// Panorama fragment shader
// Converted from panorama.fsh
// Note: Uses cubemap sampler - simplified to 2D for now

struct FragmentInput {
    @location(0) tex_coord0: vec3<f32>,
}

@group(0) @binding(2)
var panorama_texture: texture_cube<f32>;

@group(0) @binding(3)
var panorama_sampler: sampler;

@fragment
fn main_fs(in: FragmentInput) -> @location(0) vec4<f32> {
    return textureSample(panorama_texture, panorama_sampler, in.tex_coord0);
}
