// Invert post-processing fragment shader
// Converts Minecraft GLSL invert.fsh to WGSL

struct SamplerInfo {
    OutSize: vec2<f32>,
    InSize: vec2<f32>,
}

struct InvertConfig {
    InverseAmount: f32,
}

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;
@group(0) @binding(2) var<uniform> sampler_info: SamplerInfo;
@group(0) @binding(3) var<uniform> invert_config: InvertConfig;

@fragment
fn main(@location(0) texCoord: vec2<f32>) -> @location(0) vec4<f32> {
    let diffuseColor = textureSample(input_texture, input_sampler, texCoord);
    let invertColor = vec4<f32>(1.0, 1.0, 1.0, 1.0) - diffuseColor;
    let outColor = mix(diffuseColor, invertColor, invert_config.InverseAmount);
    return vec4<f32>(outColor.rgb, 1.0);
}
