// Position-Texture-Color fragment shader
// Used for GUI textures, Mojang logo, title screen, etc.
//
// All bindings in group 0 to match Bassalt's single bind group approach

struct DynamicTransforms_t {
    ModelViewMat: mat4x4<f32>,
    ColorModulator: vec4<f32>,
    ModelOffset: vec3<f32>,
    _pad0: f32,
    TextureMat: mat4x4<f32>,
}

// Group 0 bindings
@group(0) @binding(0) var Sampler0: texture_2d<f32>;
@group(0) @binding(1) var Sampler0Sampler: sampler;
@group(0) @binding(4) var<uniform> DynamicTransforms: DynamicTransforms_t;

struct FragmentInput {
    @location(0) tex_coord: vec2<f32>,
    @location(1) vertex_color: vec4<f32>,
}

@fragment
fn main(in: FragmentInput) -> @location(0) vec4<f32> {
    // Sample texture
    let tex_color = textureSample(Sampler0, Sampler0Sampler, in.tex_coord);
    
    // Combine texture with vertex color
    var color = tex_color * in.vertex_color;
    
    // Discard fully transparent pixels
    if (color.a == 0.0) {
        discard;
    }
    
    // Apply color modulator (tint from Minecraft)
    return color * DynamicTransforms.ColorModulator;
}
