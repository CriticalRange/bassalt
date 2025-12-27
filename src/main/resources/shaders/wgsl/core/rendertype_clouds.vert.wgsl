// Clouds vertex shader (no vertex input - uses vertex_index)
// Original GLSL uses gl_VertexID and texture buffer lookup for cloud cells
// WebGPU version uses @builtin(vertex_index) instead

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

struct CloudInfo {
    CloudColor: vec4<f32>,
    CloudOffset: vec3<f32>,
    _pad1: f32,
    CellSize: vec3<f32>,
    _pad2: f32,
}

@group(0) @binding(0)
var<uniform> dynamic_transforms: DynamicTransforms;

@group(0) @binding(1)
var<uniform> projection: Projection;

// Fallback: just render white clouds at fixed positions
// Full implementation would need CloudInfo buffer and CloudFaces texture buffer
// which requires additional binding support

// Cube vertices for 6 faces (bottom, top, north, south, west, east) × 4 vertices each
const vertices = array<vec3<f32>, 24>(
    // Bottom face
    vec3<f32>(1.0, 0.0, 0.0),
    vec3<f32>(1.0, 0.0, 1.0),
    vec3<f32>(0.0, 0.0, 1.0),
    vec3<f32>(0.0, 0.0, 0.0),
    // Top face
    vec3<f32>(0.0, 1.0, 0.0),
    vec3<f32>(0.0, 1.0, 1.0),
    vec3<f32>(1.0, 1.0, 1.0),
    vec3<f32>(1.0, 1.0, 0.0),
    // North face
    vec3<f32>(0.0, 0.0, 0.0),
    vec3<f32>(0.0, 1.0, 0.0),
    vec3<f32>(1.0, 1.0, 0.0),
    vec3<f32>(1.0, 0.0, 0.0),
    // South face
    vec3<f32>(1.0, 0.0, 1.0),
    vec3<f32>(1.0, 1.0, 1.0),
    vec3<f32>(0.0, 1.0, 1.0),
    vec3<f32>(0.0, 0.0, 1.0),
    // West face
    vec3<f32>(0.0, 0.0, 1.0),
    vec3<f32>(0.0, 1.0, 1.0),
    vec3<f32>(0.0, 1.0, 0.0),
    vec3<f32>(0.0, 0.0, 0.0),
    // East face
    vec3<f32>(1.0, 0.0, 0.0),
    vec3<f32>(1.0, 1.0, 0.0),
    vec3<f32>(1.0, 1.0, 1.0),
    vec3<f32>(1.0, 0.0, 1.0)
);

const faceColors = array<vec4<f32>, 6>(
    // Bottom face
    vec4<f32>(0.7, 0.7, 0.7, 1.0),
    // Top face
    vec4<f32>(1.0, 1.0, 1.0, 1.0),
    // North face
    vec4<f32>(0.8, 0.8, 0.8, 1.0),
    // South face
    vec4<f32>(0.8, 0.8, 0.8, 1.0),
    // West face
    vec4<f32>(0.9, 0.9, 0.9, 1.0),
    // East face
    vec4<f32>(0.9, 0.9, 0.9, 1.0)
);

fn fog_spherical_distance(pos: vec3<f32>) -> f32 {
    return length(pos);
}

@vertex
fn main_vs(@builtin(vertex_index) vertex_id: u32) -> VertexOutput {
    var out: VertexOutput;
    
    // Simplified cloud rendering - just use the vertex index to select a position
    // In the full implementation, this would fetch cloud cell data from a texture buffer
    let quad_vertex = vertex_id % 4u;
    let face_index = (vertex_id / 4u) % 6u;
    let cell_index = vertex_id / 24u;
    
    // Get the vertex position from the cube vertices
    let vertex_index = face_index * 4u + quad_vertex;
    var pos = vertices[vertex_index];
    
    // Apply a simple offset based on cell index (simplified, no CloudFaces lookup)
    let cell_x = f32(cell_index % 16u);
    let cell_z = f32((cell_index / 16u) % 16u);
    pos = pos * 16.0 + vec3<f32>(cell_x * 16.0, 128.0, cell_z * 16.0);
    
    out.position = projection.ProjMat * dynamic_transforms.ModelViewMat * vec4<f32>(pos, 1.0);
    out.vertex_distance = fog_spherical_distance(pos);
    out.vertex_color = faceColors[face_index] * dynamic_transforms.ColorModulator;
    
    return out;
}
