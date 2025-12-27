// Animate sprite blit combined shader
// Vertex stage has no bindings (uses vertex_index)
// Fragment stage uses bindings 2-3 for texture/sampler

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord0: vec2<f32>,
}

@vertex
fn main_vs(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0)
    );
    
    let index = vertex_index & 7u;
    out.position = vec4<f32>(positions[index] * 2.0 - 1.0, 0.0, 1.0);
    out.tex_coord0 = positions[index];
    
    return out;
}

struct FragmentInput {
    @location(0) tex_coord0: vec2<f32>,
}

@group(0) @binding(2)
var sprite_texture: texture_2d<f32>;

@group(0) @binding(3)
var sprite_sampler: sampler;

@fragment
fn main_fs(in: FragmentInput) -> @location(0) vec4<f32> {
    return textureSample(sprite_texture, sprite_sampler, in.tex_coord0);
}
