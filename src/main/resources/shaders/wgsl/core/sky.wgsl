// Stub fragment shader - GLSL conversion failed
struct FragmentInput {
    @builtin(position) position: vec4<f32>,
}

@fragment
fn main_fs(in: FragmentInput) -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 1.0, 1.0); // Magenta for visibility
}
