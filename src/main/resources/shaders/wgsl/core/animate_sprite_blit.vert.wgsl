// Animate sprite blit vertex shader

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
