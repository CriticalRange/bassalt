// Glint vertex shader
// Converted from glint.vsh

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv0: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) spherical_vertex_distance: f32,
    @location(1) cylindrical_vertex_distance: f32,
    @location(2) tex_coord0: vec2<f32>,
}

struct DynamicTransforms {
    ModelViewMat: mat4x4<f32>,
    ColorModulator: vec4<f32>,
    ModelOffset: vec3<f32>,
    _padding: f32,
    TextureMat: mat4x4<f32>,
}

struct Projection {
    ProjMat: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> dynamic_transforms: DynamicTransforms;

@group(0) @binding(1)
var<uniform> projection: Projection;

fn fog_spherical_distance(pos: vec3<f32>) -> f32 {
    return length(pos);
}

fn fog_cylindrical_distance(pos: vec3<f32>) -> f32 {
    let dist_xz = length(pos.xz);
    let dist_y = abs(pos.y);
    return max(dist_xz, dist_y);
}

@vertex
fn main_vs(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = projection.ProjMat * dynamic_transforms.ModelViewMat * vec4<f32>(in.position, 1.0);
    out.spherical_vertex_distance = fog_spherical_distance(in.position);
    out.cylindrical_vertex_distance = fog_cylindrical_distance(in.position);
    out.tex_coord0 = (dynamic_transforms.TextureMat * vec4<f32>(in.uv0, 0.0, 1.0)).xy;
    return out;
}
