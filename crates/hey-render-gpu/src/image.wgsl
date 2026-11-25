// Image quads: nearest-neighbour sampling with integer math, identical to the CPU reference.

struct Globals {
    size: vec2<f32>,
    _pad: vec2<f32>,
};
@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var img: texture_2d<f32>;

struct Quad {
    @location(0) dst: vec4<f32>,   // x, y, w, h in pixels (whole numbers)
    @location(1) src: vec4<f32>,   // image width, height (z, w unused)
    @location(2) color: vec4<f32>, // unused
};

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) dst: vec4<f32>,
    @location(1) src: vec4<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32, i: Quad) -> VOut {
    let cx = f32((vi == 1u) || (vi == 4u) || (vi == 5u));
    let cy = f32((vi == 2u) || (vi == 3u) || (vi == 5u));
    let px = i.dst.x + i.dst.z * cx;
    let py = i.dst.y + i.dst.w * cy;
    var o: VOut;
    o.pos = vec4<f32>(px / g.size.x * 2.0 - 1.0, 1.0 - py / g.size.y * 2.0, 0.0, 1.0);
    o.dst = i.dst;
    o.src = i.src;
    return o;
}

@fragment
fn fs(i: VOut) -> @location(0) vec4<f32> {
    let local = vec2<i32>(floor(i.pos.xy) - i.dst.xy);
    let dst_size = vec2<i32>(i.dst.zw);
    let src_size = vec2<i32>(i.src.xy);
    let t = (local * src_size) / dst_size;
    return textureLoad(img, t, 0);
}
