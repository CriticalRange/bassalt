// Color convolve post-processing fragment shader
// Converts Minecraft GLSL color_convolve.fsh to WGSL

struct SamplerInfo {
    OutSize: vec2<f32>,
    InSize: vec2<f32>,
}

struct ColorConfig {
    RedMatrix: vec3<f32>,
    GreenMatrix: vec3<f32>,
    BlueMatrix: vec3<f32>,
}

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;
@group(0) @binding(2) var<uniform> sampler_info: SamplerInfo;
@group(0) @binding(3) var<uniform> color_config: ColorConfig;

const GRAY: vec3<f32> = vec3<f32>(0.3, 0.59, 0.11);
const SATURATION: f32 = 1.8;

@fragment
fn main(@location(0) texCoord: vec2<f32>) -> @location(0) vec4<f32> {
    let inTexel = textureSample(input_texture, input_sampler, texCoord);

    // Color Matrix
    let redValue = dot(inTexel.rgb, color_config.RedMatrix);
    let greenValue = dot(inTexel.rgb, color_config.GreenMatrix);
    let blueValue = dot(inTexel.rgb, color_config.BlueMatrix);
    var outColor = vec3<f32>(redValue, greenValue, blueValue);

    // Saturation
    let luma = dot(outColor, GRAY);
    let chroma = outColor - luma;
    outColor = (chroma * SATURATION) + luma;

    return vec4<f32>(outColor, 1.0);
}
