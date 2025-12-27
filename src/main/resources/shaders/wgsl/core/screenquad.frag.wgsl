// Screen quad fragment shader stub (just passes through)
// Used by screenquad.vsh paired shaders

struct FragmentInput {
    @location(0) tex_coord: vec2<f32>,
}

@group(0) @binding(2)
var texture0: texture_2d<f32>;

@group(0) @binding(3)
var sampler0: sampler;

@fragment
fn main_fs(in: FragmentInput) -> @location(0) vec4<f32> {
    return textureSample(texture0, sampler0, in.tex_coord);
}
