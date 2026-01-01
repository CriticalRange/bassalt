// Block fragment shader
// 
// Uniform layout (consistent with vertex shader):
// Group 0: Sampler0 (block texture) + Sampler2 (lightmap)
// Group 1: DynamicTransforms
// Group 2: Projection (declared for layout consistency)
// Group 3: Fog

struct DynamicTransforms {
    ModelViewMat: mat4x4<f32>,
    ColorModulator: vec4<f32>,
    ModelOffset: vec3<f32>,
    _pad0: f32,
    TextureMat: mat4x4<f32>,
}

struct Projection {
    ProjMat: mat4x4<f32>,
}

// Group 0: Textures (block texture at 0,1 - lightmap at 2,3)
@group(0) @binding(0) var Sampler0: texture_2d<f32>;
@group(0) @binding(1) var Sampler0Sampler: sampler;
@group(0) @binding(2) var Sampler2: texture_2d<f32>;
@group(0) @binding(3) var Sampler2Sampler: sampler;

// Group 1: DynamicTransforms (for ColorModulator)
@group(1) @binding(0) var<uniform> transforms: DynamicTransforms;

// Group 2: Projection
@group(2) @binding(0) var<uniform> projection: Projection;

struct FragmentInput {
    @location(0) tex_coord: vec2<f32>,
    @location(1) vertex_color: vec4<f32>,
    @location(2) spherical_dist: f32,
    @location(3) cylindrical_dist: f32,
}

@fragment
fn main(in: FragmentInput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(Sampler0, Sampler0Sampler, in.tex_coord);
    var color = tex_color * in.vertex_color * transforms.ColorModulator;
    
    // Alpha cutout (ALPHA_CUTOUT = 0.5)
    if (color.a < 0.5) {
        discard;
    }
    
    return color;
}
