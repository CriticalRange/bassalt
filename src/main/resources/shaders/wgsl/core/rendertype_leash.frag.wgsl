// Leash fragment shader
@fragment
fn main(@location(0) vertex_color: vec4<f32>) -> @location(0) vec4<f32> {
    return vertex_color;
}
