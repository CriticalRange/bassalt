// Clouds vertex shader - uses vertex_index for empty vertex format
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) vertex_color: vec4<f32>,
}

@vertex
fn main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = f32((vertex_index & 1u) ^ ((vertex_index >> 1u) & 1u));
    let y = f32((vertex_index >> 1u) & 1u);
    out.position = vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
    out.vertex_color = vec4<f32>(1.0, 1.0, 1.0, 0.8);
    return out;
}
