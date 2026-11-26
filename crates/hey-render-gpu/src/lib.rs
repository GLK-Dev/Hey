// Copyright (c) Vitaly Golik (mjojo)

//! GPU renderer (wgpu). Draws the same `DisplayList` as the CPU reference (ADR-6) with an
#![warn(missing_docs)]
//! analytic-coverage instanced pipeline: one quad per primitive, the fragment shader reproduces
//! the CPU math (box-filter rect coverage, chamfer, centered border, erf shadow, capsule line).
//!
//! Text is drawn from an R8 glyph atlas (`atlas.rs`) with whole-pixel placement like the CPU path.
//! Images are one RGBA8 texture each, drawn nearest-neighbour (`image.wgsl`). Output goes to an offscreen texture (tests) or a window surface.

mod atlas;

use std::collections::HashMap;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use hey_core::{Color, Rect};
use hey_draw::{DisplayList, DrawCmd, ImageId, ImageStore, UiRenderer, Viewport};
use hey_text::Fonts;

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

/// One glyph quad. 48 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct GlyphInst {
    /// Destination `x, y, w, h` in whole pixels.
    dst: [f32; 4],
    /// Atlas texel origin `x, y` (z, w unused).
    src: [f32; 4],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    size: [f32; 2],
    _pad: [f32; 2],
}

#[derive(Debug)]
/// Errors raised while creating the GPU renderer.
#[non_exhaustive]
pub enum GpuError {
    /// No compatible GPU adapter was found.
    NoAdapter,
    /// The device could not be created.
    Device(String),
    /// The window surface could not be created or configured.
    Surface(String),
}

impl std::fmt::Display for GpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuError::NoAdapter => write!(f, "no suitable GPU adapter"),
            GpuError::Device(e) => write!(f, "GPU device error: {e}"),
            GpuError::Surface(e) => write!(f, "GPU surface error: {e}"),
        }
    }
}

impl std::error::Error for GpuError {}

/// What a batch draws; decides the pipeline and bind group.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Shape,
    Glyph,
    Image(u32),
}

/// A run of instances (shapes, glyphs or one image's quads) drawn under one scissor rectangle `(x, y, w, h)`.
struct Batch {
    kind: Kind,
    scissor: (u32, u32, u32, u32),
    start: u32,
    end: u32,
}

/// A window surface and its current configuration.
struct Presenter {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    modes: Vec<wgpu::PresentMode>,
}

/// GPU renderer: draws either into an offscreen texture (headless, for tests) or into a window surface.
pub struct GpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    globals: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    instances: wgpu::Buffer,
    instances_cap: u64,
    text_pipeline: wgpu::RenderPipeline,
    text_bind: wgpu::BindGroup,
    image_pipeline: wgpu::RenderPipeline,
    image_bgl: wgpu::BindGroupLayout,
    /// Uploaded images (each bind group keeps its texture alive); cleared by `set_images`.
    image_cache: HashMap<u32, wgpu::BindGroup>,
    images: Option<Arc<ImageStore>>,
    glyphs: wgpu::Buffer,
    glyphs_cap: u64,
    atlas_tex: wgpu::Texture,
    atlas: atlas::Atlas,
    /// Font store used to rasterize glyphs (text is skipped when `None`).
    pub fonts: Option<Arc<Fonts>>,
    /// Offscreen target (headless mode only).
    target: Option<wgpu::Texture>,
    /// Window surface (window mode only).
    present: Option<Presenter>,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    /// Colour the frame is cleared to.
    pub clear_color: Color,
}

impl GpuRenderer {
    /// Creates a headless renderer. Fails when no adapter is available (callers fall back to CPU).
    pub fn new_headless(width: u32, height: u32) -> Result<Self, GpuError> {
        Self::create(wgpu::Instance::new(&wgpu::InstanceDescriptor::default()), None, width, height)
    }

    /// Creates a renderer that presents into `window` (`Arc<winit::window::Window>` works).
    /// Fails when there is no adapter or surface (callers fall back to the CPU renderer).
    pub fn new_for_window(window: impl Into<wgpu::SurfaceTarget<'static>>, width: u32, height: u32) -> Result<Self, GpuError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window).map_err(|e| GpuError::Surface(e.to_string()))?;
        Self::create(instance, Some(surface), width, height)
    }

    fn create(instance: wgpu::Instance, surface: Option<wgpu::Surface<'static>>, width: u32, height: u32) -> Result<Self, GpuError> {
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: surface.as_ref(),
        }))
        .ok_or(GpuError::NoAdapter)?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
            .map_err(|e| GpuError::Device(e.to_string()))?;
        let (width, height) = (width.max(1), height.max(1));
        // A non-sRGB surface format keeps the blending math identical to the CPU reference (gamma space).
        let (format, present) = match surface {
            None => (FORMAT, None),
            Some(surface) => {
                let caps = surface.get_capabilities(&adapter);
                let format = caps
                    .formats
                    .iter()
                    .copied()
                    .find(|f| matches!(f, wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm))
                    .ok_or_else(|| GpuError::Surface("no non-sRGB 8-bit surface format".into()))?;
                let config = wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format,
                    width,
                    height,
                    present_mode: wgpu::PresentMode::Fifo,
                    desired_maximum_frame_latency: 2,
                    alpha_mode: caps.alpha_modes.first().copied().unwrap_or(wgpu::CompositeAlphaMode::Auto),
                    view_formats: vec![],
                };
                surface.configure(&device, &config);
                (format, Some(Presenter { surface, config, modes: caps.present_modes.clone() }))
            }
        };

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
                targets: &[Some(wgpu::ColorTargetState { format, blend: Some(over), write_mask: wgpu::ColorWrites::ALL })],
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

        // Text pipeline: glyph quads sampling the R8 atlas with `textureLoad`.
        let text_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("hey-text-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("text.wgsl").into()),
        });
        let text_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hey-text-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let text_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("hey-text-layout"),
            bind_group_layouts: &[&text_bgl],
            push_constant_ranges: &[],
        });
        let gattrs = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4];
        let text_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("hey-text-pipeline"),
            layout: Some(&text_layout),
            vertex: wgpu::VertexState {
                module: &text_shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GlyphInst>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &gattrs,
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &text_shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: Some(over), write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        // Image pipeline: same quad layout as glyphs, its own shader and a per-image bind group.
        let image_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("hey-image-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("image.wgsl").into()),
        });
        let image_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("hey-image-pipeline"),
            layout: Some(&text_layout),
            vertex: wgpu::VertexState {
                module: &image_shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GlyphInst>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &gattrs,
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &image_shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: Some(over), write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let atlas_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("hey-atlas"),
            size: wgpu::Extent3d { width: atlas::PAGE, height: atlas::PAGE, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = atlas_tex.create_view(&Default::default());
        let text_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hey-text-bind"),
            layout: &text_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&atlas_view) },
            ],
        });
        let glyphs_cap = 1024 * std::mem::size_of::<GlyphInst>() as u64;
        let glyphs = Self::make_instances(&device, glyphs_cap);
        let target = present.is_none().then(|| Self::make_target(&device, format, width, height));
        Ok(Self {
            device,
            queue,
            pipeline,
            globals,
            bind_group,
            instances,
            instances_cap,
            text_pipeline,
            text_bind,
            image_pipeline,
            image_bgl: text_bgl,
            image_cache: HashMap::new(),
            images: None,
            glyphs,
            glyphs_cap,
            atlas_tex,
            atlas: atlas::Atlas::default(),
            fonts: None,
            target,
            present,
            format,
            width,
            height,
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

    fn make_target(device: &wgpu::Device, format: wgpu::TextureFormat, w: u32, h: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("hey-target"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }

    /// Next surface texture; reconfigures once after `Outdated`/`Lost` (resize, display change).
    fn acquire(device: &wgpu::Device, p: &Presenter) -> Option<wgpu::SurfaceTexture> {
        for _ in 0..2 {
            match p.surface.get_current_texture() {
                Ok(f) => return Some(f),
                Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => p.surface.configure(device, &p.config),
                Err(_) => return None,
            }
        }
        None
    }

    /// `true`: vsync (`Fifo`, always available). `false`: lowest-latency uncapped mode the surface supports
    /// (`Mailbox`, then `Immediate`), falling back to `Fifo`. Returns the mode actually applied; no-op when headless.
    pub fn set_vsync(&mut self, vsync: bool) -> Option<wgpu::PresentMode> {
        let p = self.present.as_mut()?;
        let want = if vsync {
            wgpu::PresentMode::Fifo
        } else {
            [wgpu::PresentMode::Mailbox, wgpu::PresentMode::Immediate]
                .into_iter()
                .find(|m| p.modes.contains(m))
                .unwrap_or(wgpu::PresentMode::Fifo)
        };
        if p.config.present_mode != want {
            p.config.present_mode = want;
            p.surface.configure(&self.device, &p.config);
        }
        Some(want)
    }

    /// Current surface size in pixels as `(width, height)`.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Reads the last rendered frame back as tightly packed RGBA8, top row first (headless mode only).
    pub fn read_pixels(&self) -> Vec<u8> {
        let target = self.target.as_ref().expect("read_pixels needs a headless renderer");
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
            target.as_image_copy(),
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

    /// Sets the image registry (drops previously uploaded textures).
    pub fn set_images(&mut self, images: Option<Arc<ImageStore>>) {
        self.images = images;
        self.image_cache.clear();
    }

    /// Uploads the image on first use and emits its quad; mirrors the CPU path (whole-pixel rect, nearest sampling).
    fn push_image(&mut self, rect: &Rect, id: ImageId, out: &mut Vec<GlyphInst>) -> Option<u32> {
        let img = self.images.as_ref()?.get(id)?;
        let (x0, y0) = (rect.x.round(), rect.y.round());
        let (w, h) = (rect.right().round() - x0, rect.bottom().round() - y0);
        if w <= 0.0 || h <= 0.0 {
            return None;
        }
        let (iw, ih) = (img.width, img.height);
        if !self.image_cache.contains_key(&id.0) {
            let size = wgpu::Extent3d { width: iw, height: ih, depth_or_array_layers: 1 };
            let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("hey-image"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            self.queue.write_texture(
                tex.as_image_copy(),
                &img.rgba,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(iw * 4), rows_per_image: Some(ih) },
                size,
            );
            let view = tex.create_view(&Default::default());
            let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("hey-image-bind"),
                layout: &self.image_bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.globals.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                ],
            });
            self.image_cache.insert(id.0, bind);
        }
        out.push(GlyphInst { dst: [x0, y0, w, h], src: [iw as f32, ih as f32, 0.0, 0.0], color: [1.0; 4] });
        Some(id.0)
    }

    /// Rasterizes (once) and places the glyphs of a run; mirrors the CPU text path.
    fn push_text(&mut self, run: &hey_draw::GlyphRun, out: &mut Vec<GlyphInst>) {
        let Some(fonts) = self.fonts.clone() else { return };
        let color = rgba(run.color);
        for g in &run.glyphs {
            let id = g.id as u16;
            let key = (run.font, id, run.size_px.to_bits());
            let bmp = fonts.bitmap(run.font, id, run.size_px);
            if bmp.w == 0 || bmp.h == 0 {
                continue;
            }
            let slot = match self.atlas.get(key) {
                Some(s) => s,
                None => {
                    let s = self.atlas.alloc(key, bmp.w, bmp.h);
                    if let Some(s) = s {
                        self.queue.write_texture(
                            wgpu::TexelCopyTextureInfo {
                                texture: &self.atlas_tex,
                                mip_level: 0,
                                origin: wgpu::Origin3d { x: s.x, y: s.y, z: 0 },
                                aspect: wgpu::TextureAspect::All,
                            },
                            &bmp.data,
                            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bmp.w), rows_per_image: Some(bmp.h) },
                            wgpu::Extent3d { width: bmp.w, height: bmp.h, depth_or_array_layers: 1 },
                        );
                    }
                    s
                }
            };
            let Some(slot) = slot else { continue };
            let x0 = g.x.round() as i32 + bmp.xmin;
            let y0 = g.y.round() as i32 - bmp.ymin - bmp.h as i32;
            out.push(GlyphInst {
                dst: [x0 as f32, y0 as f32, bmp.w as f32, bmp.h as f32],
                src: [slot.x as f32, slot.y as f32, 0.0, 0.0],
                color,
            });
        }
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
            // Text and images are handled in `render` (they need renderer state).
            DrawCmd::Text(_) | DrawCmd::Image { .. } | DrawCmd::ClipPush(_) | DrawCmd::ClipPop => {}
            _ => {}
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
            self.width = w;
            self.height = h;
            if let Some(p) = &mut self.present {
                p.config.width = w;
                p.config.height = h;
                p.surface.configure(&self.device, &p.config);
            } else {
                self.target = Some(Self::make_target(&self.device, self.format, w, h));
            }
        }
    }

    fn render(&mut self, list: &DisplayList, _viewport: Viewport) {
        debug_assert!(list.validate().is_ok(), "invalid DisplayList");
        let (w, h) = (self.width, self.height);
        // A page that overflowed last frame is flushed now; its glyphs are re-added on demand.
        if self.atlas.overflowed {
            self.atlas.clear();
        }
        let mut instances: Vec<Instance> = Vec::new();
        let mut glyphs: Vec<GlyphInst> = Vec::new();
        let mut batches: Vec<Batch> = Vec::new();
        for (cmd, clip) in list.iter_clipped() {
            let Some(sc) = scissor(clip, w, h) else { continue };
            let mut kind = Kind::Shape;
            let (start, end) = if let DrawCmd::Text(run) = cmd {
                kind = Kind::Glyph;
                let start = glyphs.len() as u32;
                self.push_text(run, &mut glyphs);
                (start, glyphs.len() as u32)
            } else if let DrawCmd::Image { rect, image } = cmd {
                let start = glyphs.len() as u32;
                if let Some(id) = self.push_image(rect, *image, &mut glyphs) {
                    kind = Kind::Image(id);
                }
                (start, glyphs.len() as u32)
            } else {
                let start = instances.len() as u32;
                Self::push_cmd(&mut instances, cmd);
                (start, instances.len() as u32)
            };
            if end == start {
                continue;
            }
            match batches.last_mut() {
                Some(b) if b.kind == kind && b.scissor == sc && b.end == start => b.end = end,
                _ => batches.push(Batch { kind, scissor: sc, start, end }),
            }
        }
        let gbytes: &[u8] = bytemuck::cast_slice(&glyphs);
        if gbytes.len() as u64 > self.glyphs_cap {
            self.glyphs_cap = (gbytes.len() as u64).next_power_of_two();
            self.glyphs = Self::make_instances(&self.device, self.glyphs_cap);
        }
        if !gbytes.is_empty() {
            self.queue.write_buffer(&self.glyphs, 0, gbytes);
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

        let frame = match &self.present {
            Some(p) => match Self::acquire(&self.device, p) {
                Some(f) => Some(f),
                None => return, // frame skipped (occluded, timed out, ...); the next redraw retries
            },
            None => None,
        };
        let view = match (&frame, &self.target) {
            (Some(f), _) => f.texture.create_view(&Default::default()),
            (None, Some(t)) => t.create_view(&Default::default()),
            (None, None) => return,
        };
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
            // `None` = nothing bound yet; rebinding happens only when the batch kind changes.
            let mut bound: Option<Kind> = None;
            for b in &batches {
                if bound != Some(b.kind) {
                    match b.kind {
                        Kind::Glyph => {
                            pass.set_pipeline(&self.text_pipeline);
                            pass.set_bind_group(0, &self.text_bind, &[]);
                            pass.set_vertex_buffer(0, self.glyphs.slice(..));
                        }
                        Kind::Image(id) => {
                            pass.set_pipeline(&self.image_pipeline);
                            pass.set_bind_group(0, &self.image_cache[&id], &[]);
                            pass.set_vertex_buffer(0, self.glyphs.slice(..));
                        }
                        Kind::Shape => {
                            pass.set_pipeline(&self.pipeline);
                            pass.set_bind_group(0, &self.bind_group, &[]);
                            pass.set_vertex_buffer(0, self.instances.slice(..));
                        }
                    }
                    bound = Some(b.kind);
                }
                let (x, y, sw, sh) = b.scissor;
                pass.set_scissor_rect(x, y, sw, sh);
                pass.draw(0..6, b.start..b.end);
            }
        }
        self.queue.submit([enc.finish()]);
        if let Some(f) = frame {
            f.present();
        }
    }
}
