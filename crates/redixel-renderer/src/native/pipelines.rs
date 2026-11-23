//! The long-lived pipeline state: shaders, bind group layouts, the three
//! pipelines and the sampler every texture is read through.

use std::borrow::Cow;

use hal::Device as _;
use redixel_core::RedixelError;
use wgpu_hal as hal;
use wgpu_types as wgt;

use super::api::gpu_err;
use crate::{
    quad::QuadInstance,
    shader::{GlobalUniforms, QUAD_SHADER_SRC, SHAPE_SHADER_SRC, Vertex, depth_2d, depth_3d},
};

pub(super) struct Pipelines<A: hal::Api> {
    shape_shader:              A::ShaderModule,
    quad_shader:               A::ShaderModule,
    pub(super) globals_layout: A::BindGroupLayout,
    pub(super) texture_layout: A::BindGroupLayout,
    pub(super) layout:         A::PipelineLayout,
    pub(super) triangles_2d:   A::RenderPipeline,
    pub(super) triangles_3d:   A::RenderPipeline,
    pub(super) quads:          A::RenderPipeline,
    pub(super) sampler:        A::Sampler,
}

impl<A: hal::Api> Pipelines<A> {
    /// Builds everything against a colour target of `surface_format`.
    ///
    /// The sampler is `Nearest` so pixel art stays crisp at any scale, and
    /// `ClampToEdge` so a UV of exactly 1.0 does not wrap to the opposite edge.
    pub(super) fn new(device: &A::Device, surface_format: wgt::TextureFormat) -> Result<Self, RedixelError> {
        let shape_shader: A::ShaderModule = create_shader::<A>(device, SHAPE_SHADER_SRC, "REDIXEL_SHAPE_SHADER")?;
        let quad_shader: A::ShaderModule = create_shader::<A>(device, QUAD_SHADER_SRC, "REDIXEL_QUAD_SHADER")?;

        // SAFETY: every descriptor is fully initialised, and each object is
        // built only from objects created on `device` just before it.
        unsafe {
            let globals_layout: A::BindGroupLayout = device
                .create_bind_group_layout(&hal::BindGroupLayoutDescriptor {
                    label:   Some("REDIXEL_GLOBALS_LAYOUT"),
                    flags:   hal::BindGroupLayoutFlags::empty(),
                    entries: &[wgt::BindGroupLayoutEntry {
                        binding:    0,
                        visibility: wgt::ShaderStages::VERTEX,
                        ty:         wgt::BindingType::Buffer {
                            ty:                 wgt::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size:   wgt::BufferSize::new(size_of::<GlobalUniforms>() as u64),
                        },
                        count:      None,
                    }],
                })
                .map_err(gpu_err)?;

            let texture_layout: A::BindGroupLayout = device
                .create_bind_group_layout(&hal::BindGroupLayoutDescriptor {
                    label:   Some("REDIXEL_TEXTURE_LAYOUT"),
                    flags:   hal::BindGroupLayoutFlags::empty(),
                    entries: &[
                        wgt::BindGroupLayoutEntry {
                            binding:    0,
                            visibility: wgt::ShaderStages::FRAGMENT,
                            ty:         wgt::BindingType::Texture {
                                sample_type:    wgt::TextureSampleType::Float { filterable: true },
                                view_dimension: wgt::TextureViewDimension::D2,
                                multisampled:   false,
                            },
                            count:      None,
                        },
                        wgt::BindGroupLayoutEntry {
                            binding:    1,
                            visibility: wgt::ShaderStages::FRAGMENT,
                            ty:         wgt::BindingType::Sampler(wgt::SamplerBindingType::Filtering),
                            count:      None,
                        },
                    ],
                })
                .map_err(gpu_err)?;

            let layout: A::PipelineLayout = device
                .create_pipeline_layout(&hal::PipelineLayoutDescriptor {
                    label:              Some("REDIXEL_PIPELINE_LAYOUT"),
                    flags:              hal::PipelineLayoutFlags::empty(),
                    bind_group_layouts: &[Some(&globals_layout), Some(&texture_layout)],
                    immediate_size:     0,
                })
                .map_err(gpu_err)?;

            let vertices: [Option<hal::VertexBufferLayout<'_>>; 1] = [Some(hal::VertexBufferLayout {
                array_stride: size_of::<Vertex>() as u64,
                step_mode:    wgt::VertexStepMode::Vertex,
                attributes:   &Vertex::ATTRIBUTES,
            })];
            let instances: [Option<hal::VertexBufferLayout<'_>>; 1] = [Some(hal::VertexBufferLayout {
                array_stride: size_of::<QuadInstance>() as u64,
                step_mode:    wgt::VertexStepMode::Instance,
                attributes:   &QuadInstance::ATTRIBUTES,
            })];

            let triangles_2d: A::RenderPipeline = create_pipeline::<A>(
                device,
                (&shape_shader, "vs_main"),
                &layout,
                &vertices,
                wgt::PrimitiveTopology::TriangleList,
                surface_format,
                depth_2d(),
            )?;
            let triangles_3d: A::RenderPipeline = create_pipeline::<A>(
                device,
                (&shape_shader, "vs_main"),
                &layout,
                &vertices,
                wgt::PrimitiveTopology::TriangleList,
                surface_format,
                depth_3d(),
            )?;
            let quads: A::RenderPipeline = create_pipeline::<A>(
                device,
                (&quad_shader, "vs_quad"),
                &layout,
                &instances,
                wgt::PrimitiveTopology::TriangleStrip,
                surface_format,
                depth_2d(),
            )?;

            let sampler: A::Sampler = device
                .create_sampler(&hal::SamplerDescriptor {
                    label:            Some("REDIXEL_SAMPLER"),
                    address_modes:    [wgt::AddressMode::ClampToEdge; 3],
                    mag_filter:       wgt::FilterMode::Nearest,
                    min_filter:       wgt::FilterMode::Nearest,
                    mipmap_filter:    wgt::MipmapFilterMode::Nearest,
                    lod_clamp:        0.0..32.0,
                    compare:          None,
                    anisotropy_clamp: 1,
                    border_color:     None,
                })
                .map_err(gpu_err)?;

            Ok(Self {
                shape_shader,
                quad_shader,
                globals_layout,
                texture_layout,
                layout,
                triangles_2d,
                triangles_3d,
                quads,
                sampler,
            })
        }
    }

    /// # Safety
    ///
    /// No pending submission uses these pipelines, and every bind group built
    /// against these layouts is already destroyed.
    pub(super) unsafe fn destroy(self, device: &A::Device) {
        unsafe {
            device.destroy_sampler(self.sampler);
            device.destroy_render_pipeline(self.triangles_2d);
            device.destroy_render_pipeline(self.triangles_3d);
            device.destroy_render_pipeline(self.quads);
            device.destroy_pipeline_layout(self.layout);
            device.destroy_bind_group_layout(self.texture_layout);
            device.destroy_bind_group_layout(self.globals_layout);
            device.destroy_shader_module(self.shape_shader);
            device.destroy_shader_module(self.quad_shader);
        }
    }
}

/// Parses and validates `source` with naga and builds a shader module from it.
fn create_shader<A: hal::Api>(device: &A::Device, source: &str, label: &str) -> Result<A::ShaderModule, RedixelError> {
    let module: naga::Module = naga::front::wgsl::parse_str(source)
        .map_err(|e: naga::front::wgsl::ParseError| gpu_err(e.emit_to_string(source)))?;
    let info: naga::valid::ModuleInfo =
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty())
            .validate(&module)
            .map_err(|e: naga::WithSpan<naga::valid::ValidationError>| gpu_err(e.emit_to_string(source)))?;

    // SAFETY: the module was validated by naga just above.
    unsafe {
        device.create_shader_module(
            &hal::ShaderModuleDescriptor {
                label:          Some(label),
                runtime_checks: wgt::ShaderRuntimeChecks::checked(),
            },
            hal::ShaderInput::Naga(hal::NagaShader {
                module: Cow::Owned(module),
                info,
                debug_source: None,
            }),
        )
    }
    .map_err(|e: hal::ShaderError| gpu_err(format!("{label}: {e}")))
}

/// Builds one pipeline: `vertex` names the shader module and entry point the
/// vertex stage runs, and the fragment stage is `fs_main` in the same module.
/// Colour is alpha-blended onto the target.
///
/// # Safety
///
/// The shader and `layout` were created on `device`.
unsafe fn create_pipeline<A: hal::Api>(
    device: &A::Device,
    vertex: (&A::ShaderModule, &str),
    layout: &A::PipelineLayout,
    vertex_buffers: &[Option<hal::VertexBufferLayout<'_>>],
    topology: wgt::PrimitiveTopology,
    surface_format: wgt::TextureFormat,
    depth: wgt::DepthStencilState,
) -> Result<A::RenderPipeline, RedixelError> {
    let (shader, vertex_entry): (&A::ShaderModule, &str) = vertex;
    let constants: naga::back::PipelineConstants = naga::back::PipelineConstants::default();

    unsafe {
        device.create_render_pipeline(&hal::RenderPipelineDescriptor {
            label: Some("REDIXEL_PIPELINE"),
            layout,
            vertex_processor: hal::VertexProcessor::Standard {
                vertex_buffers,
                vertex_stage: hal::ProgrammableStage {
                    module: shader,
                    entry_point: vertex_entry,
                    constants: &constants,
                    zero_initialize_workgroup_memory: true,
                },
            },
            primitive: wgt::PrimitiveState {
                topology,
                front_face: wgt::FrontFace::Ccw,
                polygon_mode: wgt::PolygonMode::Fill,
                ..Default::default()
            },
            depth_stencil: Some(depth),
            multisample: wgt::MultisampleState::default(),
            fragment_stage: Some(hal::ProgrammableStage {
                module: shader,
                entry_point: "fs_main",
                constants: &constants,
                zero_initialize_workgroup_memory: true,
            }),
            color_targets: &[Some(wgt::ColorTargetState {
                format:     surface_format,
                blend:      Some(wgt::BlendState::ALPHA_BLENDING),
                write_mask: wgt::ColorWrites::ALL,
            })],
            multiview_mask: None,
            cache: None,
        })
    }
    .map_err(gpu_err)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn validates(source: &str) {
        let module: naga::Module = naga::front::wgsl::parse_str(source).expect("WGSL parses");
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty())
            .validate(&module)
            .expect("WGSL validates");
    }

    #[test]
    fn shaders_validate() {
        validates(SHAPE_SHADER_SRC);
        validates(QUAD_SHADER_SRC);
    }
}
