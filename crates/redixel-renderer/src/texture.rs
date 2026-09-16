use std::io::Cursor;

use image::{DynamicImage, ImageReader, Limits, RgbaImage};

use wgpu::{
    AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, Device, Extent3d, FilterMode, Origin3d, Queue, Sampler,
    SamplerBindingType, SamplerDescriptor, ShaderStages, TexelCopyBufferLayout, TexelCopyTextureInfo, Texture,
    TextureAspect, TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureUsages, TextureView,
    TextureViewDescriptor, TextureViewDimension,
};

use redixel_core::{RedixelError, TextureFilter, TextureId};

const RGBA_BYTES_PER_TEXEL: u32 = 4;
const CHECKERBOARD_SIZE: u32 = 16;
const CHECKERBOARD_CELL: u32 = 8;

/// Bound whenever geometry carries no texture. Opaque white so the shader's
/// unconditional `sample * color` leaves the vertex colour untouched, which is
/// what lets textured and untextured geometry share one pipeline and one batch.
const WHITE_PIXEL_RGBA: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xFF];

/// Builds the magenta-and-black checkerboard, deliberately the loudest thing
/// the engine can draw: a texture that failed to load is a bug in the game's
/// assets, and rendering nothing would hide it until someone noticed.
fn checkerboard_rgba(size: u32, cell: u32) -> Vec<u8> {
    const MAGENTA: [u8; 4] = [0xFF, 0x00, 0xFF, 0xFF];
    const BLACK: [u8; 4] = [0x00, 0x00, 0x00, 0xFF];

    let mut texels: Vec<u8> = Vec::with_capacity((size * size * RGBA_BYTES_PER_TEXEL) as usize);

    for y in 0..size {
        for x in 0..size {
            let dark: bool = ((x / cell) + (y / cell)).is_multiple_of(2);
            texels.extend_from_slice(if dark { &BLACK } else { &MAGENTA });
        }
    }

    texels
}

/// Every texture the engine has been asked to load, plus the sampler and bind
/// group layout they are all exposed through.
///
/// Slots are indexed by [`TextureId::index`], which the runtime hands out
/// sequentially. A slot stays `None` when its image failed to decode, costing
/// a warning and a checkerboard rather than a dead frame.
pub struct TextureRegistry {
    layout: BindGroupLayout,
    nearest_sampler: Sampler,
    linear_sampler: Sampler,
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
            label: Some("REDIXEL_TEXTURE_BIND_GROUP_LAYOUT"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let nearest_sampler: Sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("REDIXEL_TEXTURE_NEAREST_SAMPLER"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            ..Default::default()
        });
        let linear_sampler: Sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("REDIXEL_TEXTURE_LINEAR_SAMPLER"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });

        let default: BindGroup = Self::create_entry(
            device,
            queue,
            &layout,
            &nearest_sampler,
            "REDIXEL_TEXTURE_DEFAULT",
            1,
            1,
            &WHITE_PIXEL_RGBA,
        );

        let missing: BindGroup = Self::create_entry(
            device,
            queue,
            &layout,
            &nearest_sampler,
            "REDIXEL_TEXTURE_MISSING",
            CHECKERBOARD_SIZE,
            CHECKERBOARD_SIZE,
            &checkerboard_rgba(CHECKERBOARD_SIZE, CHECKERBOARD_CELL),
        );

        Self {
            layout,
            nearest_sampler,
            linear_sampler,
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
        self.upload_filtered(device, queue, id, bytes, TextureFilter::Nearest)
    }

    /// Decodes and uploads `bytes`, choosing how the texture is sampled when
    /// its draw size differs from its source dimensions.
    pub fn upload_filtered(
        &mut self,
        device: &Device,
        queue: &Queue,
        id: TextureId,
        bytes: &[u8],
        filter: TextureFilter,
    ) -> Result<(), RedixelError> {
        let index: usize = id.index() as usize;
        if index >= self.entries.len() {
            self.entries.resize_with(index + 1, || None);
        }

        self.entries[index] = None;

        let rgba: RgbaImage = Self::decode(bytes, device.limits().max_texture_dimension_2d)?;
        let (width, height): (u32, u32) = rgba.dimensions();

        let label: String = format!("REDIXEL_TEXTURE_{index}");
        let sampler: &Sampler = match filter {
            TextureFilter::Nearest => &self.nearest_sampler,
            TextureFilter::Linear => &self.linear_sampler,
        };
        let bind_group: BindGroup =
            Self::create_entry(device, queue, &self.layout, sampler, &label, width, height, &rgba);

        self.entries[index] = Some(bind_group);
        Ok(())
    }

    /// Decodes `bytes` to RGBA8, refusing an image wider or taller than `max`,
    /// the device's `max_texture_dimension_2d`.
    ///
    /// The bound goes to the decoder rather than being checked afterwards:
    /// `create_texture` answers an oversized descriptor with a validation error
    /// that WGPU's default handler turns into a panic, and rejecting from the
    /// header also avoids allocating pixels that were never going to fit.
    fn decode(bytes: &[u8], max: u32) -> Result<RgbaImage, RedixelError> {
        let mut limits: Limits = Limits::default();
        limits.max_image_width = Some(max);
        limits.max_image_height = Some(max);

        let mut reader: ImageReader<Cursor<&[u8]>> = ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|e: std::io::Error| RedixelError::TextureDecode(e.to_string()))?;
        reader.limits(limits);

        let decoded: DynamicImage = reader
            .decode()
            .map_err(|e: image::ImageError| RedixelError::TextureDecode(e.to_string()))?;

        let rgba: RgbaImage = decoded.to_rgba8();
        let (width, height): (u32, u32) = rgba.dimensions();

        if width == 0 || height == 0 {
            return Err(RedixelError::TextureDecode(String::from("image has zero width or height")));
        }

        Ok(rgba)
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
                texture: &texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            texels,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * RGBA_BYTES_PER_TEXEL),
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
                    binding: 0,
                    resource: BindingResource::TextureView(&view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(sampler),
                },
            ],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texel(texels: &[u8], size: u32, x: u32, y: u32) -> [u8; 4] {
        let start: usize = ((y * size + x) * RGBA_BYTES_PER_TEXEL) as usize;
        texels[start..start + 4].try_into().expect("texel is four bytes")
    }

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();

        DynamicImage::ImageRgba8(RgbaImage::new(width, height))
            .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
            .expect("encoding a blank image cannot fail");

        out
    }

    #[test]
    fn checkerboard_fills_every_texel() {
        let texels: Vec<u8> = checkerboard_rgba(CHECKERBOARD_SIZE, CHECKERBOARD_CELL);

        assert_eq!(
            texels.len(),
            (CHECKERBOARD_SIZE * CHECKERBOARD_SIZE * RGBA_BYTES_PER_TEXEL) as usize
        );
    }

    #[test]
    fn checkerboard_alternates_cells() {
        let size: u32 = CHECKERBOARD_SIZE;
        let cell: u32 = CHECKERBOARD_CELL;
        let texels: Vec<u8> = checkerboard_rgba(size, cell);

        assert_eq!(texel(&texels, size, 0, 0), [0x00, 0x00, 0x00, 0xFF]);
        assert_eq!(texel(&texels, size, cell, 0), [0xFF, 0x00, 0xFF, 0xFF]);
        assert_eq!(texel(&texels, size, 0, cell), [0xFF, 0x00, 0xFF, 0xFF]);
        assert_eq!(texel(&texels, size, cell, cell), [0x00, 0x00, 0x00, 0xFF]);
    }

    #[test]
    fn decode_accepts_an_image_inside_the_limit() {
        let rgba: RgbaImage = TextureRegistry::decode(&png(32, 16), 64).expect("32x16 fits under a 64 limit");

        assert_eq!(rgba.dimensions(), (32, 16));
    }

    #[test]
    fn decode_rejects_an_image_past_the_limit() {
        assert!(
            TextureRegistry::decode(&png(64, 8), 32).is_err(),
            "reaching create_texture oversized is a validation error, and the default handler panics"
        );
    }

    #[test]
    fn decode_measures_both_axes_against_the_limit() {
        assert!(TextureRegistry::decode(&png(8, 64), 32).is_err());
    }

    #[test]
    fn decode_rejects_undecodable_input() {
        assert!(TextureRegistry::decode(b"", 4096).is_err());
        assert!(TextureRegistry::decode(b"not a png at all", 4096).is_err());
    }

    #[test]
    fn checkerboard_is_fully_opaque() {
        let texels: Vec<u8> = checkerboard_rgba(CHECKERBOARD_SIZE, CHECKERBOARD_CELL);

        assert!(
            texels.chunks_exact(4).all(|t: &[u8]| t[3] == 0xFF),
            "a translucent marker would blend away the thing it is meant to flag"
        );
    }
}
