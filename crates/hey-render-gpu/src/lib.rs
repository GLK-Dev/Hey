// Copyright (c) Vitaly Golik (mjojo)

//! GPU renderer (wgpu). Draws the same `DisplayList` as the CPU reference (ADR-6) with an
//! analytic-coverage instanced pipeline: one quad per primitive, the fragment shader reproduces
//! the CPU math (box-filter rect coverage, chamfer, centered border, erf shadow, capsule line).
//!
//! Text and images are not drawn yet (glyph atlas is the next step).

use bytemuck::{Pod, Zeroable};
use hey_core::{Color, Rect};
use hey_draw::{DisplayList, DrawCmd, UiRenderer, Viewport};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

const KIND_FILL: f32 = 0.0;
const KIND_BORDER: f32 = 1.0;
const KIND_SHADOW: f32 = 2.0;
const KIND_LINE: f32 = 3.0;

/// One primitive. 64 bytes, 4 x vec4 (no padding, so `Pod` is sound).
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct Instance {
    shape: [f32; 4],
    color: [f32; 4],
    /// x: chamfer, y: border half-width, z: shadow sigma / line width, w: kind.
    p: [f32; 4],
    /// Quad bounds `x0, y0, x1, y1` (whole pixels).
    bounds: [f32; 4],
}

const _: () = assert!(std::mem::size_of::<Instance>() == 64);

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    size: [f32; 2],
    _pad: [f32; 2],
}

#[derive(Debug)]
pub enum GpuError {
    NoAdapter,
    Device(String),
}

impl std::fmt::Display for GpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuError::NoAdapter => write!(f, "no suitable GPU adapter"),
            GpuError::Device(e) => write!(f, "GPU device error: {e}"),
        }
    }
}

impl std::error::Error for GpuError {}

/// A run of instances drawn under one scissor rectangle `(x, y, w, h)`.
struct Batch {
    scissor: (u32, u32, u32, u32),
    start: u32,
    end: u32,
}

/// Offscreen (headless) GPU renderer. The window path will reuse the pipeline with a surface target.
pub struct GpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    globals: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    instances: wgpu::Buffer,
    instances_cap: u64,
    target: wgpu::Texture,
    width: u32,
    height: u32,
    pub clear_color: Color,
}

impl GpuRenderer {
    /// Creates a headless renderer. Fails when no adapter is available (callers fall back to CPU).
    pub fn new_headless(width: u32, height: u32) -> Result<Self, GpuError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .ok_or(GpuError::NoAdapter)?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
            .map_err(|e| GpuError::Device(e.to_string()))?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("hey-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hey-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("hey-layout"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });
        let attrs = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4];
        let over = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("hey-pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &attrs,
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: FORMAT, blend: Some(over), write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hey-globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hey-bind"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() }],
        });
        let instances_cap = 1024 * std::mem::size_of::<Instance>() as u64;
        let instances = Self::make_instances(&device, instances_cap);
        let target = Self::make_target(&device, width.max(1), height.max(1));
        Ok(Self {
            device,
            queue,
            pipeline,
            globals,
            bind_group,
            instances,
            instances_cap,
            target,
            width: width.max(1),
            height: height.max(1),
            clear_color: Color::rgb(0, 0, 0),
        })
    }

    fn make_instances(device: &wgpu::Device, size: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hey-instances"),
            size,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn make_target(device: &wgpu::Device, w: u32, h: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("hey-target"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Reads the last rendered frame back as tightly packed RGBA8, top row first.
    pub fn read_pixels(&self) -> Vec<u8> {
        let (w, h) = (self.width, self.height);
        let row = w * 4;
        let padded = row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hey-readback"),
            size: (padded * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            self.target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(h) },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.queue.submit([enc.finish()]);
        let slice = buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv().expect("map callback").expect("map readback");
        let data = slice.get_mapped_range();
        let mut out = Vec::with_capacity((row * h) as usize);
        for y in 0..h as usize {
            let s = y * padded as usize;
            out.extend_from_slice(&data[s..s + row as usize]);
        }
        drop(data);
        buf.unmap();
        out
    }

    /// Converts a draw command into instances (in painting order).
    fn push_cmd(out: &mut Vec<Instance>, cmd: &DrawCmd) {
        match cmd {
            DrawCmd::Rect { rect, fill, border, chamfer, shadow } => {
                if let Some(s) = shadow {
                    let r = rect.translate(s.offset.x, s.offset.y);
                    let sigma = (s.blur / 2.0).max(0.01);
                    let reach = s.blur * 1.5 + 1.0;
                    out.push(inst(&r, KIND_SHADOW, s.color, [0.0, 0.0, sigma, 0.0], &inflate(&r, reach)));
                }
                let c = chamfer.max(0.0);
                out.push(inst(rect, KIND_FILL, *fill, [c, 0.0, 0.0, 0.0], &inflate(rect, 1.0)));
                if let Some(b) = border {
                    let h = b.width / 2.0;
                    out.push(inst(rect, KIND_BORDER, b.color, [c, h, 0.0, 0.0], &inflate(rect, h + 1.0)));
                }
            }
            DrawCmd::Line { a, b, width, color } => {
                let h = width / 2.0;
                let (x0, y0) = (a.x.min(b.x) - h - 1.0, a.y.min(b.y) - h - 1.0);
                let (x1, y1) = (a.x.max(b.x) + h + 1.0, a.y.max(b.y) + h + 1.0);
                out.push(Instance {
                    shape: [a.x, a.y, b.x, b.y],
                    color: rgba(*color),
                    p: [0.0, 0.0, *width, KIND_LINE],
                    bounds: [x0.floor(), y0.floor(), x1.ceil(), y1.ceil()],
                });
            }
            // Text (glyph atlas) and images come next.
            DrawCmd::Text(_) | DrawCmd::Image { .. } | DrawCmd::ClipPush(_) | DrawCmd::ClipPop => {}
        }
    }
}

fn rgba(c: Color) -> [f32; 4] {
    [c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0, c.a as f32 / 255.0]
}

fn inflate(r: &Rect, d: f32) -> Rect {
    Rect::new(r.x - d, r.y - d, (r.w + 2.0 * d).max(0.0), (r.h + 2.0 * d).max(0.0))
}

fn inst(shape: &Rect, kind: f32, color: Color, p: [f32; 4], bounds: &Rect) -> Instance {
    Instance {
        shape: [shape.x, shape.y, shape.w, shape.h],
        color: rgba(color),
        p: [p[0], p[1], p[2], kind],
        bounds: [bounds.x.floor(), bounds.y.floor(), bounds.right().ceil(), bounds.bottom().ceil()],
    }
}

/// Clip rectangle -> whole-pixel scissor inside the target (same rounding as the CPU path).
fn scissor(clip: Option<Rect>, w: u32, h: u32) -> Option<(u32, u32, u32, u32)> {
    let (mut x0, mut y0, mut x1, mut y1) = (0i64, 0i64, w as i64, h as i64);
    if let Some(c) = clip {
        x0 = x0.max(c.x.round() as i64);
        y0 = y0.max(c.y.round() as i64);
        x1 = x1.min(c.right().round() as i64);
        y1 = y1.min(c.bottom().round() as i64);
    }
    (x1 > x0 && y1 > y0).then(|| (x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32))
}

impl UiRenderer for GpuRenderer {
    fn resize(&mut self, width: u32, height: u32) {
        let (w, h) = (width.max(1), height.max(1));
        if (w, h) != (self.width, self.height) {
            self.target = Self::make_target(&self.device, w, h);
            self.width = w;
            self.height = h;
        }
    }

    fn render(&mut self, list: &DisplayList, _viewport: Viewport) {
        debug_assert!(list.validate().is_ok(), "invalid DisplayList");
        let (w, h) = (self.width, self.height);
        let mut instances: Vec<Instance> = Vec::new();
        let mut batches: Vec<Batch> = Vec::new();
        for (cmd, clip) in list.iter_clipped() {
            let Some(sc) = scissor(clip, w, h) else { continue };
            let start = instances.len() as u32;
            Self::push_cmd(&mut instances, cmd);
            let end = instances.len() as u32;
            if end == start {
                continue;
            }
            match batches.last_mut() {
                Some(b) if b.scissor == sc && b.end == start => b.end = end,
                _ => batches.push(Batch { scissor: sc, start, end }),
            }
        }

        let bytes: &[u8] = bytemuck::cast_slice(&instances);
        if bytes.len() as u64 > self.instances_cap {
            self.instances_cap = (bytes.len() as u64).next_power_of_two();
            self.instances = Self::make_instances(&self.device, self.instances_cap);
        }
        if !bytes.is_empty() {
            self.queue.write_buffer(&self.instances, 0, bytes);
        }
        self.queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&Globals { size: [w as f32, h as f32], _pad: [0.0; 2] }));

        let view = self.target.create_view(&Default::default());
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let cc = rgba(self.clear_color);
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hey-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: cc[0] as f64, g: cc[1] as f64, b: cc[2] as f64, a: cc[3] as f64 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.instances.slice(..));
            for b in &batches {
                let (x, y, sw, sh) = b.scissor;
                pass.set_scissor_rect(x, y, sw, sh);
                pass.draw(0..6, b.start..b.end);
            }
        }
        self.queue.submit([enc.finish()]);
    }
}
