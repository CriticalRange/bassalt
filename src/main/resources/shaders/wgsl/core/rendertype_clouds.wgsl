// Clouds shader (combined vertex + fragment)
// Uses @builtin(vertex_index) - no vertex input attributes
// Based on original GLSL which uses gl_VertexID and texture buffer lookup

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) vertex_distance: f32,
    @location(1) vertex_color: vec4<f32>,
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

struct Fog {
    FogColor: vec4<f32>,
    FogEnvironmentalStart: f32,
    FogEnvironmentalEnd: f32,
    FogRenderDistanceStart: f32,
    FogRenderDistanceEnd: f32,
    FogSkyEnd: f32,
    FogCloudsEnd: f32,
}

@group(0) @binding(0)
var<uniform> dynamic_transforms: DynamicTransforms;

@group(0) @binding(1)
var<uniform> projection: Projection;

@group(0) @binding(4)
var<uniform> fog: Fog;

// Cube vertices for 6 faces × 4 vertices each
const vertices = array<vec3<f32>, 24>(
    // Bottom face
    vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(1.0, 0.0, 1.0), vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 0.0, 0.0),
    // Top face
    vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, 1.0, 1.0), vec3<f32>(1.0, 1.0, 1.0), vec3<f32>(1.0, 1.0, 0.0),
    // North face
    vec3<f32>(0.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0),
    // South face
    vec3<f32>(1.0, 0.0, 1.0), vec3<f32>(1.0, 1.0, 1.0), vec3<f32>(0.0, 1.0, 1.0), vec3<f32>(0.0, 0.0, 1.0),
    // West face
    vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 1.0, 1.0), vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, 0.0, 0.0),
    // East face
    vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(1.0, 1.0, 0.0), vec3<f32>(1.0, 1.0, 1.0), vec3<f32>(1.0, 0.0, 1.0)
);

const faceColors = array<vec4<f32>, 6>(
    vec4<f32>(0.7, 0.7, 0.7, 1.0),  // Bottom
    vec4<f32>(1.0, 1.0, 1.0, 1.0),  // Top
    vec4<f32>(0.8, 0.8, 0.8, 1.0),  // North
    vec4<f32>(0.8, 0.8, 0.8, 1.0),  // South
    vec4<f32>(0.9, 0.9, 0.9, 1.0),  // West
    vec4<f32>(0.9, 0.9, 0.9, 1.0)   // East
);

fn fog_spherical_distance(pos: vec3<f32>) -> f32 {
    return length(pos);
}

fn linear_fog_value(vertex_distance: f32, fog_start: f32, fog_end: f32) -> f32 {
    if (vertex_distance <= fog_start) {
        return 0.0;
    } else if (vertex_distance >= fog_end) {
        return 1.0;
    }
    return (vertex_distance - fog_start) / (fog_end - fog_start);
}

@vertex
fn main_vs(@builtin(vertex_index) vertex_id: u32) -> VertexOutput {
    var out: VertexOutput;
    
    let quad_vertex = vertex_id % 4u;
    let face_index = (vertex_id / 4u) % 6u;
    let cell_index = vertex_id / 24u;
    
    let vertex_index = face_index * 4u + quad_vertex;
    var pos = vertices[vertex_index];
    
    // Simplified positioning (full impl needs CloudFaces texture buffer)
    let cell_x = f32(cell_index % 16u);
    let cell_z = f32((cell_index / 16u) % 16u);
    pos = pos * 16.0 + vec3<f32>(cell_x * 16.0, 128.0, cell_z * 16.0);
    
    out.position = projection.ProjMat * dynamic_transforms.ModelViewMat * vec4<f32>(pos, 1.0);
    out.vertex_distance = fog_spherical_distance(pos);
    out.vertex_color = faceColors[face_index] * dynamic_transforms.ColorModulator;
    
    return out;
}

struct FragmentInput {
    @location(0) vertex_distance: f32,
    @location(1) vertex_color: vec4<f32>,
}

@fragment
fn main_fs(in: FragmentInput) -> @location(0) vec4<f32> {
    var color = in.vertex_color;
    color.a = color.a * (1.0 - linear_fog_value(in.vertex_distance, 0.0, fog.FogCloudsEnd));
    return color;
}
