use std::num::NonZeroU64;

use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry,
    BindingResource, BindingType, BlendState, Buffer, BufferBindingType, BufferDescriptor, BufferUsages,
    ColorTargetState, ColorWrites, CompareFunction, DepthBiasState, DepthStencilState, Device, FragmentState,
    FrontFace, MultisampleState, PipelineLayout, PipelineLayoutDescriptor, PolygonMode, PrimitiveState,
    PrimitiveTopology, Queue, RenderPipeline, RenderPipelineDescriptor, ShaderModule, ShaderModuleDescriptor,
    ShaderSource, ShaderStages, StencilState, TextureFormat, VertexAttribute, VertexBufferLayout, VertexState,
    VertexStepMode,
};

use crate::device::DEPTH_FORMAT;

const SHADER_SRC: &str = include_str!("../shaders/shape.wgsl");

/// A single vertex in the shape batch: 3D position + RGBA colour + texture
/// coordinate.
///
/// 2D draw calls carry `z = 0.0` through; only `draw_triangle_3d` supplies a
/// real z. Untextured geometry carries `uv = [0.0, 0.0]` and samples the
/// registry's 1×1 white pixel, so every vertex goes through the same
/// sample-and-multiply, textured or not.
///
/// `repr(C)` + packed fields → safe to cast to `&[u8]` via `bytemuck`.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
    pub uv: [f32; 2],
}

impl Vertex {
    const ATTRIBUTES: [VertexAttribute; 3] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x4,
        2 => Float32x2,
    ];

    pub fn layout() -> VertexBufferLayout<'static> {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

/// Everything the shader reads from `group(0) binding(0)`: the two column-major
/// 4×4 camera matrices plus the per-frame globals every stage may sample.
///
/// The field order is the memory layout WGSL expects, so it is not free to
/// change: matrices align to 16 bytes and sit first, `resolution` is a
/// `vec2<f32>` (8-byte aligned, unlike a `vec3` which would align to 16 and need
/// padding of its own), and the two trailing scalars close the struct at 144
/// bytes — a multiple of the 16-byte struct alignment, leaving no implicit
/// padding for `bytemuck::Pod` to reject.
///
/// `glow` is the strength of the shader's time-driven brightness modulation,
/// `0.0` disabling it outright, which is what every frame that never asks for
/// it uploads.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlobalUniforms {
    pub view: [[f32; 4]; 4],
    pub projection: [[f32; 4]; 4],
    pub resolution: [f32; 2],
    pub time: f32,
    pub glow: f32,
}

/// A uniform buffer paired with the bind group that exposes it.
///
/// The 2D and 3D paths differ only in the matrices they carry, so both are
/// built from this type against the same layout, and each owns a full copy of
/// the frame globals: one buffer is bound per pass, so the values a pass reads
/// have to live in the block it binds.
pub struct UniformBlock {
    pub buffer: Buffer,
    pub bind_group: BindGroup,
}

impl UniformBlock {
    fn new(device: &Device, layout: &BindGroupLayout, buffer_label: &str, group_label: &str) -> Self {
        let buffer: Buffer = device.create_buffer(&BufferDescriptor {
            label: Some(buffer_label),
            size: std::mem::size_of::<GlobalUniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group: BindGroup = device.create_bind_group(&BindGroupDescriptor {
            label: Some(group_label),
            layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(buffer.as_entire_buffer_binding()),
            }],
        });

        Self { buffer, bind_group }
    }

    /// Uploads `uniforms` to this block's buffer.
    pub fn update(&self, queue: &Queue, uniforms: &GlobalUniforms) {
        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(uniforms));
    }
}

/// Owns the two render pipelines that draw shapes, and the global uniforms each
/// one reads.
///
/// Both run the same shader over the same vertex layout, differing only in how
/// they treat the depth attachment. Group 1 (texture + sampler) belongs to the
/// [`TextureRegistry`], which owns its layout because it also owns every bind
/// group built against it, and both pipelines declare it — so the 3D path binds
/// the white pixel and its output is unchanged.
///
/// [`TextureRegistry`]: crate::texture::TextureRegistry
pub struct ShapePipeline {
    pub pipeline_2d: RenderPipeline,
    pub pipeline_3d: RenderPipeline,
    pub bind_group_layout: BindGroupLayout,
    pub globals_2d: UniformBlock,
    pub globals_3d: UniformBlock,
}

impl ShapePipeline {
    /// Group 0 is visible to both stages: the vertex stage needs the matrices,
    /// and the fragment stage reads the frame globals, which a `VERTEX`-only
    /// layout would reject at pipeline creation.
    ///
    /// That layout also pins `min_binding_size` to the size of
    /// [`GlobalUniforms`], turning a drift between the Rust struct and its WGSL
    /// counterpart into a validation error here rather than garbage read by the
    /// shader.
    pub fn new(device: &Device, surface_format: TextureFormat, texture_layout: &BindGroupLayout) -> Self {
        let shader: ShaderModule = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("REDIXEL_SHAPE_SHADER"),
            source: ShaderSource::Wgsl(SHADER_SRC.into()),
        });

        let bind_group_layout: BindGroupLayout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("REDIXEL_GLOBALS_BIND_GROUP_LAYOUT"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX_FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(std::mem::size_of::<GlobalUniforms>() as u64),
                },
                count: None,
            }],
        });

        let globals_2d: UniformBlock = UniformBlock::new(
            device,
            &bind_group_layout,
            "REDIXEL_GLOBALS_BUFFER_2D",
            "REDIXEL_GLOBALS_BIND_GROUP_2D",
        );

        let globals_3d: UniformBlock = UniformBlock::new(
            device,
            &bind_group_layout,
            "REDIXEL_GLOBALS_BUFFER_3D",
            "REDIXEL_GLOBALS_BIND_GROUP_3D",
        );

        let pipeline_layout: PipelineLayout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("REDIXEL_SHAPE_PIPELINE_LAYOUT"),
            bind_group_layouts: &[Some(&bind_group_layout), Some(texture_layout)],
            ..Default::default()
        });

        let pipeline_2d: RenderPipeline = Self::create_pipeline(
            device,
            &shader,
            &pipeline_layout,
            surface_format,
            "REDIXEL_SHAPE_PIPELINE_2D",
            Self::depth_2d(),
        );

        let pipeline_3d: RenderPipeline = Self::create_pipeline(
            device,
            &shader,
            &pipeline_layout,
            surface_format,
            "REDIXEL_SHAPE_PIPELINE_3D",
            Self::depth_3d(),
        );

        Self {
            pipeline_2d,
            pipeline_3d,
            bind_group_layout,
            globals_2d,
            globals_3d,
        }
    }

    /// Depth state for 3D geometry: a real depth test, written to the buffer.
    ///
    /// `LessEqual` rather than `Less` so coplanar geometry resolves in
    /// submission order instead of the second surface failing its own test.
    fn depth_3d() -> DepthStencilState {
        DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(CompareFunction::LessEqual),
            stencil: StencilState::default(),
            bias: DepthBiasState::default(),
        }
    }

    /// Depth state for 2D geometry: no test, no write.
    ///
    /// 2D content is painter-ordered, so later draw calls belong on top
    /// regardless of the buffer, and writing to it would let a flat overlay
    /// occlude the 3D scene behind it.
    fn depth_2d() -> DepthStencilState {
        DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(false),
            depth_compare: Some(CompareFunction::Always),
            stencil: StencilState::default(),
            bias: DepthBiasState::default(),
        }
    }

    /// Builds one of the two shape pipelines. They differ only in `depth`.
    ///
    /// Neither culls: `draw_triangle_3d` takes its corners in whatever order
    /// the caller supplies, so culling would silently drop triangles wound the
    /// "wrong" way.
    fn create_pipeline(
        device: &Device,
        shader: &ShaderModule,
        layout: &PipelineLayout,
        surface_format: TextureFormat,
        label: &str,
        depth: DepthStencilState,
    ) -> RenderPipeline {
        device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(layout),
            vertex: VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(Vertex::layout())],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: shader,
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format: surface_format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                front_face: FrontFace::Ccw,
                polygon_mode: PolygonMode::Fill,
                ..Default::default()
            },
            depth_stencil: Some(depth),
            multisample: MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        })
    }
}
