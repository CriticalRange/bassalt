// Lightmap fragment shader
// Procedurally generates lightmap colors based on texture coordinates

struct LightmapInfo {
    SkyFactor: f32,
    BlockFactor: f32,
    NightVisionFactor: f32,
    DarknessScale: f32,
    BossOverlayWorldDarkeningFactor: f32,
    BrightnessFactor: f32,
    BlockLightTint: vec3<f32>,
    SkyLightColor: vec3<f32>,
    AmbientColor: vec3<f32>,
    NightVisionColor: vec3<f32>,
}

@group(0) @binding(0) var<uniform> lightmapInfo: LightmapInfo;

fn get_brightness(level: f32) -> f32 {
    return level / (4.0 - 3.0 * level);
}

fn notGamma(color: vec3<f32>) -> vec3<f32> {
    let maxComponent = max(max(color.x, color.y), color.z);
    let maxInverted = 1.0 - maxComponent;
    let maxScaled = 1.0 - maxInverted * maxInverted * maxInverted * maxInverted;
    return color * (maxScaled / maxComponent);
}

fn parabolicMixFactor(level: f32) -> f32 {
    return (2.0 * level - 1.0) * (2.0 * level - 1.0);
}

@fragment
fn main(@location(0) texCoord: vec2<f32>) -> @location(0) vec4<f32> {
    // Calculate block and sky brightness levels based on texture coordinates
    let block_level = floor(texCoord.x * 16.0) / 15.0;
    let sky_level = floor(texCoord.y * 16.0) / 15.0;

    let block_brightness = get_brightness(block_level) * lightmapInfo.BlockFactor;
    let sky_brightness = get_brightness(sky_level) * lightmapInfo.SkyFactor;

    // Calculate ambient color with or without night vision
    let nightVisionColor = lightmapInfo.NightVisionColor * lightmapInfo.NightVisionFactor;
    var color = max(lightmapInfo.AmbientColor, nightVisionColor);

    // Add sky light
    color = color + lightmapInfo.SkyLightColor * sky_brightness;

    // Add block light
    let BlockLightColor = mix(lightmapInfo.BlockLightTint, vec3<f32>(1.0), 0.9 * parabolicMixFactor(block_level));
    color = color + BlockLightColor * block_brightness;

    // Apply boss overlay darkening effect
    color = mix(color, color * vec3<f32>(0.7, 0.6, 0.6), lightmapInfo.BossOverlayWorldDarkeningFactor);

    // Apply darkness effect scale
    color = color - vec3<f32>(lightmapInfo.DarknessScale);

    // Apply brightness
    color = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
    let notGamma = notGamma(color);
    color = mix(color, notGamma, lightmapInfo.BrightnessFactor);

    return vec4<f32>(color, 1.0);
}
