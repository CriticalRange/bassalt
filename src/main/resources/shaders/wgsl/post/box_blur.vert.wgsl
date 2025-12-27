// Box blur vertex shader (uses screenquad)
// Generates a fullscreen quad from vertex index

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
}

@vertex
fn main_vs(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // Generate fullscreen triangle/quad UVs from vertex index
    let uv = vec2<f32>(
        f32((vertex_index << 1u) & 2u),
        f32(vertex_index & 2u)
    );
    let pos = vec4<f32>(uv * vec2<f32>(2.0, 2.0) + vec2<f32>(-1.0, -1.0), 0.0, 1.0);
    out.position = pos;
    out.tex_coord = uv;
    return out;
}
