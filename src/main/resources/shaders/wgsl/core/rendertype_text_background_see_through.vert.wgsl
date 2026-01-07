// Text background see-through vertex shader (GUI without fog/lightmap)
//
// All bindings in group 0 to match Bassalt's single bind group approach
// Vertex format: POSITION_COLOR_LIGHTMAP (position, color, uv2)
// Memory layout: Position[12] + Color[4] + UV2[8] = 24 bytes
// Note: uv2 is present in vertex format but not used for GUI

struct DynamicTransforms_t {
    ModelViewMat: mat4x4<f32>,
    ColorModulator: vec4<f32>,
    ModelOffset: vec3<f32>,
    _pad0: f32,
    TextureMat: mat4x4<f32>,
}

struct Projection_t {
    ProjMat: mat4x4<f32>,
}

// Group 0 bindings
@group(0) @binding(4) var<uniform> DynamicTransforms: DynamicTransforms_t;
@group(0) @binding(5) var<uniform> Projection: Projection_t;

// Vertex format includes uv2 at location 2 even though we don't use it for GUI
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv2: vec2<f32>,  // Lightmap (not used for GUI)
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) vertex_color: vec4<f32>,
}

@vertex
fn main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = Projection.ProjMat * DynamicTransforms.ModelViewMat * vec4<f32>(in.position, 1.0);
    out.vertex_color = in.color;
    return out;
}
