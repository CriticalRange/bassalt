// Animate sprite interpolate vertex shader

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) animation_progress: f32,
    @location(1) tex_coord0: vec2<f32>,
}

struct AnimationSprite {
    ProjectionMatrix: mat4x4<f32>,
    SpriteMatrix: mat4x4<f32>,
    UPadding: f32,
    VPadding: f32,
    MipMapLevel: f32,
}

@group(0) @binding(0)
var<uniform> animation_sprite: AnimationSprite;

@vertex
fn main_vs(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0)
    );
    
    let index = vertex_index & 7u;
    let frame_progress = f32(vertex_index >> 3u) / 1000.0;
    let padding = vec2<f32>(animation_sprite.UPadding, animation_sprite.VPadding);
    
    out.position = animation_sprite.ProjectionMatrix * animation_sprite.SpriteMatrix * vec4<f32>(positions[index], 0.0, 1.0);
    let uv = positions[index];
    let direction = uv * 2.0 - 1.0;
    out.tex_coord0 = uv + (padding * direction);
    out.animation_progress = frame_progress;
    
    return out;
}
