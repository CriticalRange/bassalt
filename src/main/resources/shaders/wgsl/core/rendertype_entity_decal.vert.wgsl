// Entity decal vertex shader

struct DynamicUniforms {
    model_view: mat4x4<f32>,
    color_mod: vec4<f32>,
    model_offset: vec3<f32>,
    _pad0: f32,
    texture_mat: mat4x4<f32>,
}

struct ProjectionUniform {
    proj_mat: mat4x4<f32>,
}

@group(1) @binding(0) var<uniform> uniforms: DynamicUniforms;
@group(2) @binding(0) var<uniform> projection: ProjectionUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) tex_coord: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) vertex_color: vec4<f32>,
    @location(1) tex_coord: vec2<f32>,
}

@vertex
fn main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = projection.proj_mat * uniforms.model_view * vec4<f32>(in.position, 1.0);
    out.vertex_color = in.color;
    out.tex_coord = in.tex_coord;
    return out;
}
