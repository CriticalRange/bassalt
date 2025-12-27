// End portal vertex shader
// Converted from rendertype_end_portal.vsh

struct VertexInput {
    @location(0) position: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_proj0: vec4<f32>,
    @location(1) spherical_vertex_distance: f32,
    @location(2) cylindrical_vertex_distance: f32,
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

fn projection_from_position(position: vec4<f32>) -> vec4<f32> {
    var proj = position * 0.5;
    proj.x = proj.x + proj.w;
    proj.y = proj.y + proj.w;
    return vec4<f32>(proj.xy, position.zw);
}

@vertex
fn main_vs(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = projection.ProjMat * dynamic_transforms.ModelViewMat * vec4<f32>(in.position, 1.0);
    out.tex_proj0 = projection_from_position(out.position);
    out.spherical_vertex_distance = fog_spherical_distance(in.position);
    out.cylindrical_vertex_distance = fog_cylindrical_distance(in.position);
    return out;
}
