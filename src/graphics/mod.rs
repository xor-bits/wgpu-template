use std::{borrow::Cow, mem::size_of, sync::Arc, thread, time::Instant};

use anyhow::{anyhow, Result};
use glam::{Mat2, Mat4, Vec2, Vec4};
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    *,
};
use winit::window::Window;

use crate::{settings::GlobalSettings, RuntimeSettings};

use self::surface::{Surface, SurfaceBuilder};

use bytemuck::{Pod, Zeroable};

//

pub mod surface;

//

pub struct Graphics {
    device: Arc<Device>,
    queue: Queue,
    surface: Surface,

    boot: Instant,
    value: f32,

    #[allow(unused)]
    limits: Limits,

    vbo: Buffer,
    pipeline: RenderPipeline,
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct PushConstant {
    mvp: Mat4,
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct Vertex {
    col: Vec4,
    pos: Vec2,
    _pad: Vec2,
}

//

impl Graphics {
    pub async fn init(settings: &GlobalSettings, window: Arc<Window>) -> Result<Self> {
        let s = &settings.graphics;

        let instance = Arc::new(wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: s.allowed_backends.to_backends(),
            ..<_>::default()
        }));

        #[cfg(not(target_family = "wasm"))]
        {
            let inst = instance.clone();
            thread::spawn(move || {
                inst.poll_all(true);
            });
        }

        let surface_builder = SurfaceBuilder::new(instance.clone(), window)?;

        let gpu = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: s.gpu_preference.to_power_preference(),
                force_fallback_adapter: s.force_software_rendering,
                compatible_surface: Some(&surface_builder.surface),
            })
            .await
            .ok_or_else(|| anyhow!("Could not find a suitable GPU"))?;

        /* let features = Features::POLYGON_MODE_LINE | Features::PUSH_CONSTANTS;
        let limits = Limits {
            max_texture_dimension_2d: 128,
            max_push_constant_size: core::mem::size_of::<Push>() as u32,
            ..Limits::downlevel_defaults()
        }; */
        let features = gpu.features();
        let limits = gpu.limits();

        let (device, queue) = gpu
            .request_device(
                &DeviceDescriptor {
                    label: None,
                    features,
                    limits: limits.clone(),
                },
                None,
            )
            .await?;
        let device = Arc::new(device);

        let surface = surface_builder.build(s, &gpu, device.clone());

        let module = device.create_shader_module(ShaderModuleDescriptor {
            label: None,
            source: ShaderSource::Wgsl(Cow::Borrowed(include_str!("./shader.wgsl"))),
        });

        let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[],
            push_constant_ranges: &[PushConstantRange {
                stages: ShaderStages::VERTEX,
                range: 0..size_of::<PushConstant>() as u32,
            }],
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: None,
            layout: Some(&layout),
            vertex: VertexState {
                module: &module,
                entry_point: "vs_main",
                buffers: &[VertexBufferLayout {
                    array_stride: size_of::<Vertex>() as _,
                    step_mode: VertexStepMode::Vertex,
                    attributes: &[
                        VertexAttribute {
                            format: VertexFormat::Float32x4,
                            offset: 0,
                            shader_location: 0,
                        },
                        VertexAttribute {
                            format: VertexFormat::Float32x2,
                            offset: size_of::<Vec4>() as _,
                            shader_location: 1,
                        },
                    ],
                }],
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: <_>::default(),
            fragment: Some(FragmentState {
                module: &module,
                entry_point: "fs_main",
                targets: &[Some(ColorTargetState {
                    format: surface.format(),
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            multiview: None,
        });

        const SCALE: f32 = 0.8;
        let rot_mat = Mat2::from_angle(2.0 * std::f32::consts::FRAC_PI_3);
        let vbo = device.create_buffer_init(&BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[
                Vertex {
                    col: Vec4::new(1.0, 0.0, 0.0, 1.0),
                    pos: Vec2::new(0.0, -SCALE),
                    _pad: Vec2::ZERO,
                },
                Vertex {
                    col: Vec4::new(0.0, 1.0, 0.0, 1.0),
                    pos: rot_mat * Vec2::new(0.0, -SCALE),
                    _pad: Vec2::ZERO,
                },
                Vertex {
                    col: Vec4::new(0.0, 0.0, 1.0, 1.0),
                    pos: rot_mat * rot_mat * Vec2::new(0.0, -SCALE),
                    _pad: Vec2::ZERO,
                },
            ]),
            usage: BufferUsages::VERTEX,
        });

        Ok(Self {
            device,
            queue,
            surface,

            boot: Instant::now(),
            value: 0.0,

            limits,

            vbo,
            pipeline,
        })
    }

    pub fn scrolled(&mut self, delta: (f32, f32)) {
        self.value += delta.0 + delta.1;
        tracing::debug!("value: {}", self.value);
    }

    pub fn resized(&mut self, size: (u32, u32)) {
        self.surface.configure(Some(size));
    }

    pub fn frame(&mut self, _settings: &RuntimeSettings) {
        let texture = self
            .surface
            .acquire()
            .expect("Failed to acquire the next frame");

        let texture_view = texture
            .texture
            .create_view(&TextureViewDescriptor { ..<_>::default() });

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor { ..<_>::default() });

        // let a = 1.0 / (1.0 + (-0.5 + self.value as f64).exp());
        self.value = self.value.max(0.0).min(10.0);
        let a = self.value as f64 / 10.0;
        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &texture_view,
                resolve_target: None,
                /* ops: Operations {
                    load: LoadOp::Load, // no clear
                    store: true,
                }, */
                ops: Operations {
                    load: LoadOp::Clear(Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a,
                    }),
                    store: true,
                },
            })],
            ..<_>::default()
        });

        pass.set_pipeline(&self.pipeline);

        let size = self.surface.window.inner_size().cast::<f32>();
        let aspect = size.width / size.height;
        let push = PushConstant {
            mvp: Mat4::orthographic_rh(-aspect, aspect, 1.0, -1.0, -1.0, 1.0)
                * Mat4::from_rotation_z(self.boot.elapsed().as_secs_f32()),
        };

        pass.set_push_constants(ShaderStages::VERTEX, 0, bytemuck::cast_slice(&[push]));
        pass.set_vertex_buffer(0, self.vbo.slice(..));

        pass.draw(0..3, 0..1);

        drop(pass);

        self.queue.submit([encoder.finish()]);

        texture.present();
        self.surface.window.set_visible(true);
    }
}
