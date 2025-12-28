struct DynamicTransforms {
    ModelViewMat: mat4x4<f32>,
    ColorModulator: vec4<f32>,
    ModelOffset: vec3<f32>,
    TextureMat: mat4x4<f32>,
}

struct Projection {
    ProjMat: mat4x4<f32>,
}

struct VertexOutput {
    @location(0) texCoord0_: vec3<f32>,
    @builtin(position) gl_Position: vec4<f32>,
}

@group(0) @binding(0) 
var<uniform> global: DynamicTransforms;
@group(0) @binding(1) 
var<uniform> global_1: Projection;
var<private> Position_1: vec3<f32>;
var<private> texCoord0_: vec3<f32>;
var<private> gl_Position: vec4<f32>;

fn projection_from_position(position: vec4<f32>) -> vec4<f32> {
    var position_1: vec4<f32>;
    var projection: vec4<f32>;

    position_1 = position;
    let _e12 = position_1;
    projection = (_e12 * 0.5f);
    let _e16 = projection;
    let _e18 = projection;
    let _e20 = projection;
    let _e23 = projection;
    let _e25 = projection;
    let _e28 = vec2<f32>((_e18.x + _e20.w), (_e23.y + _e25.w));
    projection.x = _e28.x;
    projection.y = _e28.y;
    let _e33 = projection;
    let _e35 = position_1;
    let _e36 = _e35.zw;
    projection.z = _e36.x;
    projection.w = _e36.y;
    let _e41 = projection;
    return _e41;
}

fn main_1() {
    let _e13 = global_1.ProjMat;
    let _e14 = global.ModelViewMat;
    let _e16 = Position_1;
    gl_Position = ((_e13 * _e14) * vec4<f32>(_e16.x, _e16.y, _e16.z, 1f));
    let _e23 = Position_1;
    texCoord0_ = _e23;
    return;
}

@vertex 
fn main(@location(0) Position: vec3<f32>) -> VertexOutput {
    Position_1 = Position;
    main_1();
    let _e17 = texCoord0_;
    let _e19 = gl_Position;
    return VertexOutput(_e17, _e19);
}
