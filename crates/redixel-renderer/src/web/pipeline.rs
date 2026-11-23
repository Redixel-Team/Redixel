use std::num::NonZeroU64;

use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry,
    BindingResource, BindingType, BlendState, Buffer, BufferBindingType, BufferDescriptor, BufferUsages,
    ColorTargetState, ColorWrites, DepthStencilState, Device, FragmentState, FrontFace, MultisampleState,
    PipelineLayout, PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology, Queue, RenderPipeline,
    RenderPipelineDescriptor, ShaderModule, ShaderModuleDescriptor, ShaderSource, ShaderStages, TextureFormat,
    VertexBufferLayout, VertexState, VertexStepMode,
};

use crate::{
    quad::QuadInstance,
    shader::{GlobalUniforms, QUAD_SHADER_SRC, SHAPE_SHADER_SRC, Vertex, depth_2d, depth_3d},
};

impl Vertex {
    /// The layout of the vertex buffer the shape pipelines read.
    pub fn layout() -> VertexBufferLayout<'static> {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode:    VertexStepMode::Vertex,
            attributes:   &Self::ATTRIBUTES,
        }
    }
}

/// The vertex half of one shape pipeline: which shader entry point runs, the
/// single vertex buffer it reads, and how its vertices assemble into triangles.
struct VertexStage<'a> {
    shader:   &'a ShaderModule,
    entry:    &'a str,
    buffer:   VertexBufferLayout<'a>,
    topology: PrimitiveTopology,
}

/// A uniform buffer paired with the bind group that exposes it.
///
/// The 2D and 3D paths differ only in the matrices they carry, so both are
/// built from this type against the same layout, and each owns a full copy of
/// the frame globals: one buffer is bound per pass, so the values a pass reads
/// have to live in the block it binds.
pub struct UniformBlock {
    pub buffer:     Buffer,
    pub bind_group: BindGroup,
}

impl UniformBlock {
    fn new(device: &Device, layout: &BindGroupLayout, buffer_label: &str, group_label: &str) -> Self {
        let buffer: Buffer = device.create_buffer(&BufferDescriptor {
            label:              Some(buffer_label),
            size:               std::mem::size_of::<GlobalUniforms>() as u64,
            usage:              BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group: BindGroup = device.create_bind_group(&BindGroupDescriptor {
            label: Some(group_label),
            layout,
            entries: &[BindGroupEntry {
                binding:  0,
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
    pub pipeline_2d:       RenderPipeline,
    pub pipeline_3d:       RenderPipeline,
    pub pipeline_quads:    RenderPipeline,
    pub bind_group_layout: BindGroupLayout,
    pub globals_2d:        UniformBlock,
    pub globals_3d:        UniformBlock,
}

impl ShapePipeline {
    /// Group 0 is visible only to the vertex stage, the only one that reads
    /// `globals` — it transforms by `projection * view`; the fragment stage
    /// samples the sprite texture and nothing else.
    ///
    /// The layout pins `min_binding_size` to the size of [`GlobalUniforms`],
    /// which fails pipeline creation if the WGSL struct outgrows the Rust one.
    /// It is a floor, not an equality check: wgpu only rejects a binding
    /// smaller than the shader needs, so a Rust struct that grows or reorders
    /// its fields still binds cleanly and the shader reads the wrong offsets.
    /// Any edit here belongs in `shape.wgsl` in the same commit.
    pub fn new(device: &Device, surface_format: TextureFormat, texture_layout: &BindGroupLayout) -> Self {
        let shader: ShaderModule = device.create_shader_module(ShaderModuleDescriptor {
            label:  Some("REDIXEL_SHAPE_SHADER"),
            source: ShaderSource::Wgsl(SHAPE_SHADER_SRC.into()),
        });

        let quad_shader: ShaderModule = device.create_shader_module(ShaderModuleDescriptor {
            label:  Some("REDIXEL_QUAD_SHADER"),
            source: ShaderSource::Wgsl(QUAD_SHADER_SRC.into()),
        });

        let bind_group_layout: BindGroupLayout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label:   Some("REDIXEL_GLOBALS_BIND_GROUP_LAYOUT"),
            entries: &[BindGroupLayoutEntry {
                binding:    0,
                visibility: ShaderStages::VERTEX,
                ty:         BindingType::Buffer {
                    ty:                 BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size:   NonZeroU64::new(std::mem::size_of::<GlobalUniforms>() as u64),
                },
                count:      None,
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
            &pipeline_layout,
            surface_format,
            "REDIXEL_SHAPE_PIPELINE_2D",
            depth_2d(),
            VertexStage {
                shader:   &shader,
                entry:    "vs_main",
                buffer:   Vertex::layout(),
                topology: PrimitiveTopology::TriangleList,
            },
        );

        let pipeline_3d: RenderPipeline = Self::create_pipeline(
            device,
            &pipeline_layout,
            surface_format,
            "REDIXEL_SHAPE_PIPELINE_3D",
            depth_3d(),
            VertexStage {
                shader:   &shader,
                entry:    "vs_main",
                buffer:   Vertex::layout(),
                topology: PrimitiveTopology::TriangleList,
            },
        );

        let pipeline_quads: RenderPipeline = Self::create_pipeline(
            device,
            &pipeline_layout,
            surface_format,
            "REDIXEL_QUAD_PIPELINE",
            depth_2d(),
            VertexStage {
                shader:   &quad_shader,
                entry:    "vs_quad",
                buffer:   VertexBufferLayout {
                    array_stride: std::mem::size_of::<QuadInstance>() as u64,
                    step_mode:    VertexStepMode::Instance,
                    attributes:   &QuadInstance::ATTRIBUTES,
                },
                topology: PrimitiveTopology::TriangleStrip,
            },
        );

        Self {
            pipeline_2d,
            pipeline_3d,
            pipeline_quads,
            bind_group_layout,
            globals_2d,
            globals_3d,
        }
    }

    /// Builds one shape pipeline. All of them share the layout, the fragment
    /// stage and the blend state; they differ in `depth` and in the vertex
    /// stage `vertex` describes.
    ///
    /// None culls: `draw_triangle_3d` takes its corners in whatever order the
    /// caller supplies, so culling would silently drop triangles wound the
    /// "wrong" way.
    fn create_pipeline(
        device: &Device,
        layout: &PipelineLayout,
        surface_format: TextureFormat,
        label: &str,
        depth: DepthStencilState,
        vertex: VertexStage<'_>,
    ) -> RenderPipeline {
        let VertexStage {
            shader,
            entry,
            buffer,
            topology,
        }: VertexStage<'_> = vertex;

        device.create_render_pipeline(&RenderPipelineDescriptor {
            label:          Some(label),
            layout:         Some(layout),
            vertex:         VertexState {
                module:              shader,
                entry_point:         Some(entry),
                buffers:             &[Some(buffer)],
                compilation_options: Default::default(),
            },
            fragment:       Some(FragmentState {
                module:              shader,
                entry_point:         Some("fs_main"),
                targets:             &[Some(ColorTargetState {
                    format:     surface_format,
                    blend:      Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive:      PrimitiveState {
                topology,
                front_face: FrontFace::Ccw,
                polygon_mode: PolygonMode::Fill,
                ..Default::default()
            },
            depth_stencil:  Some(depth),
            multisample:    MultisampleState::default(),
            multiview_mask: None,
            cache:          None,
        })
    }
}
