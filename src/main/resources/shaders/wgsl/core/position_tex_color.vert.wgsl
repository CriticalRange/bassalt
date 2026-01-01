// Position-Texture-Color vertex shader
// Used for GUI textures, Mojang logo, title screen, etc.
// 
// Uniform layout (std140):
// Group 0: Textures (binding 0: texture, binding 1: sampler)
// Group 1: DynamicTransforms (binding 0: uniform buffer)
// Group 2: Projection (binding 0: uniform buffer)

// DynamicTransforms uniform block - matches Minecraft's layout
// std140 layout: mat4(64) + vec4(16) + vec3(16 padded) + mat4(64) = 160 bytes
struct DynamicTransforms {
    ModelViewMat: mat4x4<f32>,     // offset 0,   size 64
    ColorModulator: vec4<f32>,      // offset 64,  size 16
    ModelOffset: vec3<f32>,         // offset 80,  size 12
    _pad0: f32,                     // offset 92,  size 4 (padding)
    TextureMat: mat4x4<f32>,        // offset 96,  size 64
}

// Projection uniform block
struct Projection {
    ProjMat: mat4x4<f32>,
}

// Group 1: DynamicTransforms
@group(1) @binding(0) var<uniform> transforms: DynamicTransforms;

// Group 2: Projection  
@group(2) @binding(0) var<uniform> projection: Projection;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) vertex_color: vec4<f32>,
}

@vertex
fn main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    // Apply projection and model-view matrices
    out.position = projection.ProjMat * transforms.ModelViewMat * vec4<f32>(in.position, 1.0);
    
    // Pass through texture coordinates
    out.tex_coord = in.tex_coord;
    
    // Pass through vertex color
    out.vertex_color = in.color;
    
    return out;
}
