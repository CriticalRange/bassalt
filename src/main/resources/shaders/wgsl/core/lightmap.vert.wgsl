// Lightmap vertex shader

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
}

@vertex
fn main_vs(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    
    // Generate fullscreen quad
    let uv = vec2<f32>(
        f32((vertex_index << 1u) & 2u),
        f32(vertex_index & 2u)
    );
    out.position = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    out.tex_coord = uv;
    
    return out;
}
