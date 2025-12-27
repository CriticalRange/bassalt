// Entity decal vertex shader
// Converted from rendertype_entity_decal.vsh

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv0: vec2<f32>,
    @location(3) uv1: vec2<f32>,
    @location(4) uv2: vec2<f32>,
    @location(5) normal: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) spherical_vertex_distance: f32,
    @location(1) cylindrical_vertex_distance: f32,
    @location(2) vertex_color: vec4<f32>,
    @location(3) tex_coord0: vec2<f32>,
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

struct Lighting {
    Light0_Direction: vec3<f32>,
    _padding0: f32,
    Light1_Direction: vec3<f32>,
    _padding1: f32,
}

@group(0) @binding(0)
var<uniform> dynamic_transforms: DynamicTransforms;

@group(0) @binding(1)
var<uniform> projection: Projection;

@group(0) @binding(5)
var<uniform> lighting: Lighting;

const MINECRAFT_LIGHT_POWER: f32 = 0.6;
const MINECRAFT_AMBIENT_LIGHT: f32 = 0.4;

fn fog_spherical_distance(pos: vec3<f32>) -> f32 {
    return length(pos);
}

fn fog_cylindrical_distance(pos: vec3<f32>) -> f32 {
    let dist_xz = length(pos.xz);
    let dist_y = abs(pos.y);
    return max(dist_xz, dist_y);
}

fn minecraft_mix_light(light_dir0: vec3<f32>, light_dir1: vec3<f32>, normal: vec3<f32>, color: vec4<f32>) -> vec4<f32> {
    let light0 = max(0.0, dot(light_dir0, normal));
    let light1 = max(0.0, dot(light_dir1, normal));
    let light_accum = min(1.0, (light0 + light1) * MINECRAFT_LIGHT_POWER + MINECRAFT_AMBIENT_LIGHT);
    return vec4<f32>(color.rgb * light_accum, color.a);
}

@vertex
fn main_vs(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = projection.ProjMat * dynamic_transforms.ModelViewMat * vec4<f32>(in.position, 1.0);
    out.spherical_vertex_distance = fog_spherical_distance(in.position);
    out.cylindrical_vertex_distance = fog_cylindrical_distance(in.position);
    out.vertex_color = minecraft_mix_light(lighting.Light0_Direction, lighting.Light1_Direction, in.normal, in.color);
    out.tex_coord0 = in.uv0;
    return out;
}
