use image::RgbaImage;
use redixel_core::{RedixelError, TextureId};
use wgpu::{
    AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, Device, Extent3d, FilterMode, Origin3d, Queue, Sampler,
    SamplerBindingType, SamplerDescriptor, ShaderStages, TexelCopyBufferLayout, TexelCopyTextureInfo, Texture,
    TextureAspect, TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureUsages, TextureView,
    TextureViewDescriptor, TextureViewDimension,
};

use crate::decode::{
    CHECKERBOARD_CELL, CHECKERBOARD_SIZE, RGBA_BYTES_PER_TEXEL, WHITE_PIXEL_RGBA, checkerboard_rgba, decode_rgba,
};

/// Every texture the engine has been asked to load, plus the sampler and bind
/// group layout they are all exposed through.
///
/// Slots are indexed by [`TextureId::index`], which the runtime hands out
/// sequentially. A slot stays `None` when its image failed to decode, costing
/// a warning and a checkerboard rather than a dead frame.
pub struct TextureRegistry {
    layout:  BindGroupLayout,
    sampler: Sampler,
    default: BindGroup,
    missing: BindGroup,
    entries: Vec<Option<BindGroup>>,
}

impl TextureRegistry {
    /// `Nearest` keeps pixel art crisp; linear filtering would blur a sprite
    /// drawn at anything but its native size. `ClampToEdge` stops a UV landing
    /// exactly on 1.0 from wrapping to the opposite edge.
    pub fn new(device: &Device, queue: &Queue) -> Self {
        let layout: BindGroupLayout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label:   Some("REDIXEL_TEXTURE_BIND_GROUP_LAYOUT"),
            entries: &[
                BindGroupLayoutEntry {
                    binding:    0,
                    visibility: ShaderStages::FRAGMENT,
                    ty:         BindingType::Texture {
                        sample_type:    TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled:   false,
                    },
                    count:      None,
                },
                BindGroupLayoutEntry {
                    binding:    1,
                    visibility: ShaderStages::FRAGMENT,
                    ty:         BindingType::Sampler(SamplerBindingType::Filtering),
                    count:      None,
                },
            ],
        });

        let sampler: Sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("REDIXEL_TEXTURE_SAMPLER"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            ..Default::default()
        });

        let default: BindGroup = Self::create_entry(
            device,
            queue,
            &layout,
            &sampler,
            "REDIXEL_TEXTURE_DEFAULT",
            1,
            1,
            &WHITE_PIXEL_RGBA,
        );

        let missing: BindGroup = Self::create_entry(
            device,
            queue,
            &layout,
            &sampler,
            "REDIXEL_TEXTURE_MISSING",
            CHECKERBOARD_SIZE,
            CHECKERBOARD_SIZE,
            &checkerboard_rgba(CHECKERBOARD_SIZE, CHECKERBOARD_CELL),
        );

        Self {
            layout,
            sampler,
            default,
            missing,
            entries: Vec::new(),
        }
    }

    /// The layout every texture bind group is built against, and the one the
    /// shape pipelines declare at group 1.
    pub fn layout(&self) -> &BindGroupLayout {
        &self.layout
    }

    /// Decodes `bytes` to RGBA8 and uploads it into the slot named by `id`.
    ///
    /// The slot is emptied first, so a failed reload of an id that already held
    /// an image falls back to the checkerboard rather than leaving the stale one.
    pub fn upload(&mut self, device: &Device, queue: &Queue, id: TextureId, bytes: &[u8]) -> Result<(), RedixelError> {
        let index: usize = id.index() as usize;
        if index >= self.entries.len() {
            self.entries.resize_with(index + 1, || None);
        }

        self.entries[index] = None;

        let rgba: RgbaImage = decode_rgba(bytes, device.limits().max_texture_dimension_2d)?;
        let (width, height): (u32, u32) = rgba.dimensions();

        let label: String = format!("REDIXEL_TEXTURE_{index}");
        let bind_group: BindGroup =
            Self::create_entry(device, queue, &self.layout, &self.sampler, &label, width, height, &rgba);

        self.entries[index] = Some(bind_group);
        Ok(())
    }

    /// The bind group to use for geometry tagged with `texture`.
    ///
    /// `None` is untextured geometry and gets the white pixel. An id that was
    /// never uploaded, or whose upload failed, gets the checkerboard — drawing
    /// with a broken handle is never fatal.
    pub fn bind_group(&self, texture: Option<TextureId>) -> &BindGroup {
        let Some(id) = texture else {
            return &self.default;
        };

        match self.entries.get(id.index() as usize) {
            Some(Some(entry)) => entry,
            _ => &self.missing,
        }
    }

    /// Creates an `Rgba8UnormSrgb` texture from `texels` and wraps it, with the
    /// shared sampler, in a bind group.
    ///
    /// sRGB on purpose: the surface is normally sRGB and the shader applies no
    /// gamma correction, so sampling has to return linear values for the
    /// write-out encode to land back where it started. Against the rare adapter
    /// offering no sRGB surface format, nothing re-encodes and sprites render
    /// darker than their source.
    ///
    /// The decoder's tightly packed rows go up as-is; only
    /// `copy_buffer_to_texture` needs `bytes_per_row` padded to 256.
    #[allow(clippy::too_many_arguments)]
    fn create_entry(
        device: &Device,
        queue: &Queue,
        layout: &BindGroupLayout,
        sampler: &Sampler,
        label: &str,
        width: u32,
        height: u32,
        texels: &[u8],
    ) -> BindGroup {
        let size: Extent3d = Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture: Texture = device.create_texture(&TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            TexelCopyTextureInfo {
                texture:   &texture,
                mip_level: 0,
                origin:    Origin3d::ZERO,
                aspect:    TextureAspect::All,
            },
            texels,
            TexelCopyBufferLayout {
                offset:         0,
                bytes_per_row:  Some(width * RGBA_BYTES_PER_TEXEL),
                rows_per_image: Some(height),
            },
            size,
        );

        let view: TextureView = texture.create_view(&TextureViewDescriptor::default());

        device.create_bind_group(&BindGroupDescriptor {
            label: Some(label),
            layout,
            entries: &[
                BindGroupEntry {
                    binding:  0,
                    resource: BindingResource::TextureView(&view),
                },
                BindGroupEntry {
                    binding:  1,
                    resource: BindingResource::Sampler(sampler),
                },
            ],
        })
    }
}
