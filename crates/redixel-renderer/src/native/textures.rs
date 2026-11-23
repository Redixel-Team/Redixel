//! Sampled textures: the white pixel, the checkerboard, and every image the
//! game loaded, each with the bind group that exposes it at group 1.

use std::{borrow::Cow, iter};

use hal::{CommandEncoder as _, Device as _, Queue as _};
use redixel_core::{RedixelError, TextureId};
use wgpu_hal as hal;
use wgpu_types as wgt;

use super::api::{create_buffer, gpu_err, write_mapped};
use crate::decode::RGBA_BYTES_PER_TEXEL;

/// sRGB on purpose: the surface is sRGB and the shader applies no gamma
/// correction, so sampling has to return linear values.
const FORMAT: wgt::TextureFormat = wgt::TextureFormat::Rgba8UnormSrgb;

pub(super) struct GpuTexture<A: hal::Api> {
    texture: A::Texture,
    view:    A::TextureView,
    group:   A::BindGroup,
}

impl<A: hal::Api> GpuTexture<A> {
    /// # Safety
    ///
    /// No pending submission samples the texture.
    pub(super) unsafe fn destroy(self, device: &A::Device) {
        unsafe {
            device.destroy_bind_group(self.group);
            device.destroy_texture_view(self.view);
            device.destroy_texture(self.texture);
        }
    }
}

/// What a synchronous texture upload borrows from the renderer.
pub(super) struct Upload<'a, A: hal::Api> {
    pub(super) device:         &'a A::Device,
    pub(super) queue:          &'a A::Queue,
    pub(super) fence:          &'a A::Fence,
    pub(super) last_submitted: &'a mut hal::FenceValue,
    pub(super) encoder:        &'a mut A::CommandEncoder,
    pub(super) layout:         &'a A::BindGroupLayout,
    pub(super) sampler:        &'a A::Sampler,
}

impl<A: hal::Api> Upload<'_, A> {
    /// Creates a `width`×`height` texture from tightly packed RGBA8 `texels`
    /// and waits until the GPU has copied them in.
    ///
    /// # Safety
    ///
    /// No other thread is using the queue.
    pub(super) unsafe fn texture(
        &mut self,
        label: &str,
        width: u32,
        height: u32,
        texels: &[u8],
    ) -> Result<GpuTexture<A>, RedixelError> {
        let tight: u32 = width * RGBA_BYTES_PER_TEXEL;
        let padded: u32 = tight.next_multiple_of(wgt::COPY_BYTES_PER_ROW_ALIGNMENT);
        let rows: Cow<'_, [u8]> = if padded == tight {
            Cow::Borrowed(texels)
        } else {
            let mut rows: Vec<u8> = vec![0; (padded * height) as usize];
            for (dst, src) in rows
                .chunks_exact_mut(padded as usize)
                .zip(texels.chunks_exact(tight as usize))
            {
                dst[..tight as usize].copy_from_slice(src);
            }
            Cow::Owned(rows)
        };

        let staging: A::Buffer = create_buffer::<A>(
            self.device,
            "REDIXEL_TEXTURE_STAGING",
            rows.len() as u64,
            wgt::BufferUses::COPY_SRC | wgt::BufferUses::MAP_WRITE,
        )?;

        // SAFETY: the staging buffer is exactly `rows` long, the texture is
        // created here and transitioned out of its undefined state before the
        // copy, and both outlive the submission because it is waited for
        // before either can be destroyed.
        let result: Result<GpuTexture<A>, RedixelError> = unsafe {
            write_mapped::<A>(self.device, &staging, &rows)
                .and_then(|()| self.copy(label, width, height, padded, &staging))
        };
        unsafe { self.device.destroy_buffer(staging) };

        result
    }

    /// # Safety
    ///
    /// As for [`texture`](Self::texture), with `staging` already filled with
    /// `height` rows `bytes_per_row` apart.
    unsafe fn copy(
        &mut self,
        label: &str,
        width: u32,
        height: u32,
        bytes_per_row: u32,
        staging: &A::Buffer,
    ) -> Result<GpuTexture<A>, RedixelError> {
        let size: wgt::Extent3d = wgt::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        unsafe {
            let texture: A::Texture = self
                .device
                .create_texture(&hal::TextureDescriptor {
                    label: Some(label),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgt::TextureDimension::D2,
                    format: FORMAT,
                    usage: wgt::TextureUses::COPY_DST | wgt::TextureUses::RESOURCE,
                    memory_flags: hal::MemoryFlags::empty(),
                    view_formats: Vec::new(),
                })
                .map_err(gpu_err)?;

            if let Err(e) = self.record_and_wait(&texture, size, bytes_per_row, staging) {
                self.device.destroy_texture(texture);
                return Err(e);
            }

            let view: A::TextureView = match self.device.create_texture_view(
                &texture,
                &hal::TextureViewDescriptor {
                    label:     Some(label),
                    format:    FORMAT,
                    dimension: wgt::TextureViewDimension::D2,
                    usage:     wgt::TextureUses::RESOURCE,
                    range:     wgt::ImageSubresourceRange::default(),
                },
            ) {
                Ok(view) => view,
                Err(e) => {
                    self.device.destroy_texture(texture);
                    return Err(gpu_err(e));
                }
            };

            let group: A::BindGroup = match self.device.create_bind_group(&hal::BindGroupDescriptor {
                label:                   Some(label),
                layout:                  self.layout,
                buffers:                 &[],
                samplers:                &[self.sampler],
                textures:                &[hal::TextureBinding {
                    view:  &view,
                    usage: wgt::TextureUses::RESOURCE,
                }],
                acceleration_structures: &[],
                external_textures:       &[],
                entries:                 &[
                    hal::BindGroupEntry {
                        binding:        0,
                        resource_index: 0,
                        count:          1,
                    },
                    hal::BindGroupEntry {
                        binding:        1,
                        resource_index: 0,
                        count:          1,
                    },
                ],
            }) {
                Ok(group) => group,
                Err(e) => {
                    self.device.destroy_texture_view(view);
                    self.device.destroy_texture(texture);
                    return Err(gpu_err(e));
                }
            };

            Ok(GpuTexture { texture, view, group })
        }
    }

    /// # Safety
    ///
    /// As for [`copy`](Self::copy), with `texture` freshly created.
    unsafe fn record_and_wait(
        &mut self,
        texture: &A::Texture,
        size: wgt::Extent3d,
        bytes_per_row: u32,
        staging: &A::Buffer,
    ) -> Result<(), RedixelError> {
        unsafe {
            self.encoder
                .begin_encoding(Some("REDIXEL_TEXTURE_UPLOAD"))
                .map_err(gpu_err)?;
            self.encoder.transition_buffers(iter::once(hal::BufferBarrier {
                buffer: staging,
                usage:  hal::StateTransition {
                    from: wgt::BufferUses::MAP_WRITE,
                    to:   wgt::BufferUses::COPY_SRC,
                },
            }));
            self.encoder.transition_textures(iter::once(hal::TextureBarrier {
                texture,
                range: wgt::ImageSubresourceRange::default(),
                usage: hal::StateTransition {
                    from: wgt::TextureUses::UNINITIALIZED,
                    to:   wgt::TextureUses::COPY_DST,
                },
            }));
            self.encoder.copy_buffer_to_texture(
                staging,
                texture,
                iter::once(hal::BufferTextureCopy {
                    buffer_layout: wgt::TexelCopyBufferLayout {
                        offset:         0,
                        bytes_per_row:  Some(bytes_per_row),
                        rows_per_image: Some(size.height),
                    },
                    texture_base:  hal::TextureCopyBase {
                        origin:      wgt::Origin3d::ZERO,
                        mip_level:   0,
                        array_layer: 0,
                        aspect:      hal::FormatAspects::COLOR,
                    },
                    size:          hal::CopyExtent {
                        width:  size.width,
                        height: size.height,
                        depth:  1,
                    },
                }),
            );
            self.encoder.transition_textures(iter::once(hal::TextureBarrier {
                texture,
                range: wgt::ImageSubresourceRange::default(),
                usage: hal::StateTransition {
                    from: wgt::TextureUses::COPY_DST,
                    to:   wgt::TextureUses::RESOURCE,
                },
            }));

            let commands: A::CommandBuffer = self.encoder.end_encoding().map_err(gpu_err)?;
            *self.last_submitted += 1;
            let submitted: Result<(), RedixelError> = self
                .queue
                .submit(&[&commands], &[], (self.fence, *self.last_submitted))
                .map_err(gpu_err)
                .and_then(|()| {
                    self.device
                        .wait(self.fence, *self.last_submitted, None)
                        .map(drop)
                        .map_err(gpu_err)
                });
            self.encoder.reset_all(iter::once(commands));

            submitted
        }
    }
}

/// Every texture the game has loaded, indexed by [`TextureId::index`], plus
/// the two the engine makes itself.
pub(super) struct Textures<A: hal::Api> {
    white:   GpuTexture<A>,
    missing: GpuTexture<A>,
    entries: Vec<Option<GpuTexture<A>>>,
}

impl<A: hal::Api> Textures<A> {
    pub(super) fn new(white: GpuTexture<A>, missing: GpuTexture<A>) -> Self {
        Self {
            white,
            missing,
            entries: Vec::new(),
        }
    }

    /// Stores `texture` in `id`'s slot, or empties the slot for `None`, and
    /// returns what the slot held.
    pub(super) fn replace(&mut self, id: TextureId, texture: Option<GpuTexture<A>>) -> Option<GpuTexture<A>> {
        let index: usize = id.index() as usize;
        if index >= self.entries.len() {
            self.entries.resize_with(index + 1, || None);
        }
        std::mem::replace(&mut self.entries[index], texture)
    }

    /// The group for geometry tagged with `texture`: the white pixel for
    /// `None`, the checkerboard for an id that never loaded.
    pub(super) fn group(&self, texture: Option<TextureId>) -> &A::BindGroup {
        let Some(id) = texture else {
            return &self.white.group;
        };

        match self.entries.get(id.index() as usize) {
            Some(Some(entry)) => &entry.group,
            _ => &self.missing.group,
        }
    }

    /// # Safety
    ///
    /// No pending submission samples any of these textures.
    pub(super) unsafe fn destroy(self, device: &A::Device) {
        for texture in [self.white, self.missing]
            .into_iter()
            .chain(self.entries.into_iter().flatten())
        {
            unsafe { texture.destroy(device) };
        }
    }
}
