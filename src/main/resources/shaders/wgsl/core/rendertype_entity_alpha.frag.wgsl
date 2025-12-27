// Entity alpha fragment shader
// Converted from rendertype_entity_alpha.fsh

struct FragmentInput {
    @location(0) vertex_color: vec4<f32>,
    @location(1) tex_coord0: vec2<f32>,
}

@group(0) @binding(2)
var texture0: texture_2d<f32>;

@group(0) @binding(3)
var sampler0: sampler;

@fragment
fn main_fs(in: FragmentInput) -> @location(0) vec4<f32> {
    let color = textureSample(texture0, sampler0, in.tex_coord0);
    if (color.a < in.vertex_color.a) {
        discard;
    }
    return color;
}
