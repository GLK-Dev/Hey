// Analytic-coverage shader. Reproduces the CPU reference (`hey-render-cpu/src/raster.rs`) per pixel.

struct Globals {
    size: vec2<f32>,
    _pad: vec2<f32>,
};
@group(0) @binding(0) var<uniform> g: Globals;

struct Instance {
    @location(0) shape: vec4<f32>,  // rect x, y, w, h (line: ax, ay, bx, by)
    @location(1) color: vec4<f32>,  // straight alpha, 0..1
    @location(2) p: vec4<f32>,      // x: chamfer, y: border half-width, z: shadow sigma / line width, w: kind
    @location(3) bounds: vec4<f32>, // quad bounds x0, y0, x1, y1 in pixels
};

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) shape: vec4<f32>,
    @location(1) color: vec4<f32>,
    @location(2) p: vec4<f32>,
};

const SQRT2: f32 = 1.41421356;

@vertex
fn vs(@builtin(vertex_index) vi: u32, inst: Instance) -> VOut {
    // Two triangles: (0,0) (1,0) (0,1) (0,1) (1,0) (1,1).
    let cx = f32((vi == 1u) || (vi == 4u) || (vi == 5u));
    let cy = f32((vi == 2u) || (vi == 3u) || (vi == 5u));
    let px = mix(inst.bounds.x, inst.bounds.z, cx);
    let py = mix(inst.bounds.y, inst.bounds.w, cy);
    var o: VOut;
    o.pos = vec4<f32>(px / g.size.x * 2.0 - 1.0, 1.0 - py / g.size.y * 2.0, 0.0, 1.0);
    o.shape = inst.shape;
    o.color = inst.color;
    o.p = inst.p;
    return o;
}

fn overlap(p: f32, a: f32, b: f32) -> f32 {
    return clamp(min(p + 1.0, b) - max(p, a), 0.0, 1.0);
}

// Rectangle with a TL/BR chamfer; `px, py` is the pixel's top-left corner.
fn shape_cov(r: vec4<f32>, c: f32, px: f32, py: f32) -> f32 {
    var cov = overlap(px, r.x, r.x + r.z) * overlap(py, r.y, r.y + r.w);
    if (c > 0.0) {
        let s = px + 0.5 + py + 0.5;
        let tl = (s - (r.x + r.y + c)) / SQRT2;
        let br = ((r.x + r.z + r.y + r.w - c) - s) / SQRT2;
        cov = cov * clamp(0.5 + tl, 0.0, 1.0) * clamp(0.5 + br, 0.0, 1.0);
    }
    return cov;
}

fn inflate(r: vec4<f32>, d: f32) -> vec4<f32> {
    return vec4<f32>(r.x - d, r.y - d, max(r.z + 2.0 * d, 0.0), max(r.w + 2.0 * d, 0.0));
}

// Abramowitz-Stegun 7.1.26, same as the CPU.
fn erf(x: f32) -> f32 {
    let s = select(-1.0, 1.0, x >= 0.0);
    let a = abs(x);
    let t = 1.0 / (1.0 + 0.3275911 * a);
    let y = 1.0 - (((((1.0614054 * t - 1.4531521) * t) + 1.4214138) * t - 0.28449672) * t + 0.2548296) * t * exp(-a * a);
    return s * y;
}

fn axis(p: f32, a: f32, b: f32, k: f32) -> f32 {
    return 0.5 * (erf((b - p) / k) - erf((a - p) / k));
}

fn seg_dist(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let d = b - a;
    let len2 = dot(d, d);
    var t = 0.0;
    if (len2 != 0.0) {
        t = clamp(dot(p - a, d) / len2, 0.0, 1.0);
    }
    return length(p - (a + t * d));
}

@fragment
fn fs(i: VOut) -> @location(0) vec4<f32> {
    let px = floor(i.pos.x);
    let py = floor(i.pos.y);
    let kind = u32(i.p.w + 0.5);
    var cov = 0.0;
    if (kind == 0u) {
        cov = shape_cov(i.shape, i.p.x, px, py);
    } else if (kind == 1u) {
        let h = i.p.y;
        let k = h * (2.0 - SQRT2);
        let outer = shape_cov(inflate(i.shape, h), i.p.x + k, px, py);
        let inner = shape_cov(inflate(i.shape, -h), max(i.p.x - k, 0.0), px, py);
        cov = max(outer - inner, 0.0);
    } else if (kind == 2u) {
        let k = i.p.z * SQRT2;
        let r = i.shape;
        cov = axis(px + 0.5, r.x, r.x + r.z, k) * axis(py + 0.5, r.y, r.y + r.w, k);
    } else {
        let d = seg_dist(vec2<f32>(px + 0.5, py + 0.5), i.shape.xy, i.shape.zw);
        cov = clamp(0.5 + i.p.z * 0.5 - d, 0.0, 1.0);
    }
    return vec4<f32>(i.color.rgb, i.color.a * clamp(cov, 0.0, 1.0));
}
