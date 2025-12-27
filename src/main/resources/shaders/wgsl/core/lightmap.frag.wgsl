// Lightmap fragment shader
// Converted from lightmap.fsh

struct FragmentInput {
    @location(0) tex_coord: vec2<f32>,
}

struct LightmapInfo {
    SkyFactor: f32,
    BlockFactor: f32,
    NightVisionFactor: f32,
    DarknessScale: f32,
    BossOverlayWorldDarkeningFactor: f32,
    BrightnessFactor: f32,
    _padding0: f32,
    _padding1: f32,
    BlockLightTint: vec3<f32>,
    _padding2: f32,
    SkyLightColor: vec3<f32>,
    _padding3: f32,
    AmbientColor: vec3<f32>,
    _padding4: f32,
    NightVisionColor: vec3<f32>,
    _padding5: f32,
}

@group(0) @binding(0)
var<uniform> lightmap_info: LightmapInfo;

fn get_brightness(level: f32) -> f32 {
    return level / (4.0 - 3.0 * level);
}

fn not_gamma(color: vec3<f32>) -> vec3<f32> {
    let max_component = max(max(color.x, color.y), color.z);
    if (max_component <= 0.0) {
        return color;
    }
    let max_inverted = 1.0 - max_component;
    let max_scaled = 1.0 - max_inverted * max_inverted * max_inverted * max_inverted;
    return color * (max_scaled / max_component);
}

fn parabolic_mix_factor(level: f32) -> f32 {
    return (2.0 * level - 1.0) * (2.0 * level - 1.0);
}

@fragment
fn main_fs(in: FragmentInput) -> @location(0) vec4<f32> {
    // Calculate block and sky brightness levels based on texture coordinates
    let block_level = floor(in.tex_coord.x * 16.0) / 15.0;
    let sky_level = floor(in.tex_coord.y * 16.0) / 15.0;

    let block_brightness = get_brightness(block_level) * lightmap_info.BlockFactor;
    let sky_brightness = get_brightness(sky_level) * lightmap_info.SkyFactor;

    // Calculate ambient color with or without night vision
    let night_vision_color = lightmap_info.NightVisionColor * lightmap_info.NightVisionFactor;
    var color = max(lightmap_info.AmbientColor, night_vision_color);

    // Add sky light
    color = color + lightmap_info.SkyLightColor * sky_brightness;

    // Add block light
    let block_light_color = mix(lightmap_info.BlockLightTint, vec3<f32>(1.0), 0.9 * parabolic_mix_factor(block_level));
    color = color + block_light_color * block_brightness;

    // Apply boss overlay darkening effect
    color = mix(color, color * vec3<f32>(0.7, 0.6, 0.6), lightmap_info.BossOverlayWorldDarkeningFactor);

    // Apply darkness effect scale
    color = color - vec3<f32>(lightmap_info.DarknessScale);

    // Apply brightness
    color = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
    let not_gamma_color = not_gamma(color);
    color = mix(color, not_gamma_color, lightmap_info.BrightnessFactor);

    return vec4<f32>(color, 1.0);
}
