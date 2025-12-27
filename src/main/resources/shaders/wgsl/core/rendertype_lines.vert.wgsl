// Lines vertex shader
// Converted from rendertype_lines.vsh (simplified - without line width calculation)

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) normal: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) spherical_vertex_distance: f32,
    @location(1) cylindrical_vertex_distance: f32,
    @location(2) vertex_color: vec4<f32>,
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

const VIEW_SHRINK: f32 = 1.0 - (1.0 / 256.0);

@vertex
fn main_vs(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    // Apply view scale to prevent z-fighting
    let view_scale = mat4x4<f32>(
        vec4<f32>(VIEW_SHRINK, 0.0, 0.0, 0.0),
        vec4<f32>(0.0, VIEW_SHRINK, 0.0, 0.0),
        vec4<f32>(0.0, 0.0, VIEW_SHRINK, 0.0),
        vec4<f32>(0.0, 0.0, 0.0, 1.0)
    );
    out.position = projection.ProjMat * view_scale * dynamic_transforms.ModelViewMat * vec4<f32>(in.position, 1.0);
    out.spherical_vertex_distance = fog_spherical_distance(in.position);
    out.cylindrical_vertex_distance = fog_cylindrical_distance(in.position);
    out.vertex_color = in.color;
    return out;
}
