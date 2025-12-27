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

struct FragmentOutput {
    @location(0) fragColor: vec4<f32>,
}

@group(0) @binding(0) 
var<uniform> lightmapInfo: LightmapInfo;
var<private> texCoord_1: vec2<f32>;
var<private> fragColor: vec4<f32>;

fn get_brightness(level: f32) -> f32 {
    var level_1: f32;

    level_1 = level;
    let _e25 = level_1;
    let _e28 = level_1;
    return (_e25 / (4f - (3f * _e28)));
}

fn notGamma(color: vec3<f32>) -> vec3<f32> {
    var color_1: vec3<f32>;
    var maxComponent: f32;
    var maxInverted: f32;
    var maxScaled: f32;

    color_1 = color;
    let _e25 = color_1;
    let _e27 = color_1;
    let _e30 = color_1;
    maxComponent = max(max(_e25.x, _e27.y), _e30.z);
    let _e35 = maxComponent;
    maxInverted = (1f - _e35);
    let _e39 = maxInverted;
    let _e40 = maxInverted;
    let _e42 = maxInverted;
    let _e44 = maxInverted;
    maxScaled = (1f - (((_e39 * _e40) * _e42) * _e44));
    let _e48 = color_1;
    let _e49 = maxScaled;
    let _e50 = maxComponent;
    return (_e48 * (_e49 / _e50));
}

fn parabolicMixFactor(level_2: f32) -> f32 {
    var level_3: f32;

    level_3 = level_2;
    let _e26 = level_3;
    let _e31 = level_3;
    return (((2f * _e26) - 1f) * ((2f * _e31) - 1f));
}

fn main_1() {
    var block_level: f32;
    var sky_level: f32;
    var block_brightness: f32;
    var sky_brightness: f32;
    var nightVisionColor: vec3<f32>;
    var color_2: vec3<f32>;
    var BlockLightColor: vec3<f32>;
    var notGamma_1: vec3<f32>;

    let _e23 = texCoord_1;
    block_level = (floor((_e23.x * 16f)) / 15f);
    let _e33 = texCoord_1;
    sky_level = (floor((_e33.y * 16f)) / 15f);
    let _e43 = block_level;
    let _e44 = get_brightness(_e43);
    let _e45 = lightmapInfo;
    block_brightness = (_e44 * _e45.BlockFactor);
    let _e49 = sky_level;
    let _e50 = get_brightness(_e49);
    let _e51 = lightmapInfo;
    sky_brightness = (_e50 * _e51.SkyFactor);
    let _e55 = lightmapInfo;
    let _e57 = lightmapInfo;
    nightVisionColor = (_e55.NightVisionColor * _e57.NightVisionFactor);
    let _e61 = lightmapInfo;
    let _e63 = nightVisionColor;
    color_2 = max(_e61.AmbientColor, _e63);
    let _e66 = color_2;
    let _e67 = lightmapInfo;
    let _e69 = sky_brightness;
    color_2 = (_e66 + (_e67.SkyLightColor * _e69));
    let _e72 = lightmapInfo;
    let _e77 = block_level;
    let _e78 = parabolicMixFactor(_e77);
    BlockLightColor = mix(_e72.BlockLightTint, vec3(1f), vec3((0.9f * _e78)));
    let _e83 = color_2;
    let _e84 = BlockLightColor;
    let _e85 = block_brightness;
    color_2 = (_e83 + (_e84 * _e85));
    let _e88 = color_2;
    let _e89 = color_2;
    let _e95 = lightmapInfo;
    color_2 = mix(_e88, (_e89 * vec3<f32>(0.7f, 0.6f, 0.6f)), vec3(_e95.BossOverlayWorldDarkeningFactor));
    let _e99 = color_2;
    let _e100 = lightmapInfo;
    color_2 = (_e99 - vec3(_e100.DarknessScale));
    let _e104 = color_2;
    color_2 = clamp(_e104, vec3(0f), vec3(1f));
    let _e110 = color_2;
    let _e111 = notGamma(_e110);
    notGamma_1 = _e111;
    let _e113 = color_2;
    let _e114 = notGamma_1;
    let _e115 = lightmapInfo;
    color_2 = mix(_e113, _e114, vec3(_e115.BrightnessFactor));
    let _e119 = color_2;
    fragColor = vec4<f32>(_e119.x, _e119.y, _e119.z, 1f);
    return;
}

@fragment 
fn main(@location(0) texCoord: vec2<f32>) -> FragmentOutput {
    texCoord_1 = texCoord;
    main_1();
    let _e28 = fragColor;
    return FragmentOutput(_e28);
}
