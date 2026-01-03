// Entity sobel post-processing fragment shader
// Converts Minecraft GLSL entity_sobel.fsh to WGSL

struct SamplerInfo {
    OutSize: vec2<f32>,
    InSize: vec2<f32>,
}

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;
@group(0) @binding(2) var<uniform> sampler_info: SamplerInfo;

@fragment
fn main(@location(0) texCoord: vec2<f32>) -> @location(0) vec4<f32> {
    let oneTexel = 1.0 / sampler_info.InSize;

    let center = textureSample(input_texture, input_sampler, texCoord);
    let left = textureSample(input_texture, input_sampler, texCoord - vec2<f32>(oneTexel.x, 0.0));
    let right = textureSample(input_texture, input_sampler, texCoord + vec2<f32>(oneTexel.x, 0.0));
    let up = textureSample(input_texture, input_sampler, texCoord - vec2<f32>(0.0, oneTexel.y));
    let down = textureSample(input_texture, input_sampler, texCoord + vec2<f32>(0.0, oneTexel.y));

    let leftDiff = abs(center.a - left.a);
    let rightDiff = abs(center.a - right.a);
    let upDiff = abs(center.a - up.a);
    let downDiff = abs(center.a - down.a);
    let total = clamp(leftDiff + rightDiff + upDiff + downDiff, 0.0, 1.0);

    let outColor = center.rgb * center.a + left.rgb * left.a + right.rgb * right.a + up.rgb * up.a + down.rgb * down.a;
    return vec4<f32>(outColor * 0.2, total);
}
