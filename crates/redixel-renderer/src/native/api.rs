//! What differs between `wgpu-hal` APIs, behind [`ApiExt`], and the buffer
//! helpers every API shares.

use std::{iter, ptr, ptr::NonNull};

use hal::Device as _;
use redixel_core::RedixelError;
use wgpu_hal as hal;
use wgpu_types::BufferUses;

pub(super) fn gpu_err(e: impl std::fmt::Display) -> RedixelError {
    RedixelError::Gpu(e.to_string())
}

/// Creates a buffer `size` bytes long, preferring host-coherent memory when
/// `usage` maps it.
pub(super) fn create_buffer<A: hal::Api>(
    device: &A::Device,
    label: &str,
    size: u64,
    usage: BufferUses,
) -> Result<A::Buffer, RedixelError> {
    // SAFETY: plain allocation; the descriptor is fully initialised.
    unsafe {
        device.create_buffer(&hal::BufferDescriptor {
            label: Some(label),
            size,
            usage,
            memory_flags: hal::MemoryFlags::PREFER_COHERENT,
        })
    }
    .map_err(gpu_err)
}

/// Copies `bytes` to the start of a buffer created with `MAP_WRITE`.
///
/// # Safety
///
/// `buffer` is at least `bytes.len()` long and no pending submission reads it.
pub(super) unsafe fn write_mapped<A: hal::Api>(
    device: &A::Device,
    buffer: &A::Buffer,
    bytes: &[u8],
) -> Result<(), RedixelError> {
    let len: u64 = bytes.len() as u64;

    unsafe {
        let mapping: hal::BufferMapping = device.map_buffer(buffer, 0..len).map_err(gpu_err)?;
        ptr::copy_nonoverlapping(bytes.as_ptr(), mapping.ptr.as_ptr(), bytes.len());
        if !mapping.is_coherent {
            device.flush_mapped_ranges(buffer, iter::once(0..len));
        }
        device.unmap_buffer(buffer);
    }

    Ok(())
}

/// The CPU address a streaming buffer stays mapped at for its whole life.
///
/// Only the frame thread writes through it, and only while it holds the
/// renderer mutably, so moving it across threads is sound.
#[derive(Debug, Clone, Copy)]
pub(super) struct Mapped(pub(super) NonNull<u8>);

// SAFETY: see the type's documentation.
unsafe impl Send for Mapped {}
// SAFETY: see the type's documentation.
unsafe impl Sync for Mapped {}

/// The per-API hooks [`HalRenderer`](super::renderer::HalRenderer) needs beyond
/// the `wgpu-hal` traits. Each has a default that is correct for every API; an
/// API overrides one only where it can do better.
pub(super) trait ApiExt: hal::Api {
    /// Allocates a buffer the CPU rewrites every frame and the GPU reads once.
    /// Returns, alongside it, the address it stays mapped at, when it does.
    ///
    /// The default asks `wgpu-hal` for `MAP_WRITE` memory and maps it for each
    /// write.
    fn create_stream_buffer(
        device: &Self::Device,
        label: &str,
        size: u64,
        usage: BufferUses,
    ) -> Result<(Self::Buffer, Option<Mapped>), RedixelError> {
        Ok((create_buffer::<Self>(device, label, size, usage | BufferUses::MAP_WRITE)?, None))
    }

    /// A value that stays the same for as long as a swapchain image does, so a
    /// view of it can be reused across frames. `None` disables that reuse.
    fn surface_image_key(_texture: &Self::Texture) -> Option<u64> {
        None
    }
}

/// DX12's `MAP_WRITE` buffers live in an upload heap and Metal's in shared
/// memory, both system RAM the GPU reads directly, so the defaults are already
/// the fast path there.
#[cfg(any(target_os = "linux", target_os = "android", target_os = "windows"))]
impl ApiExt for hal::api::Gles {}

#[cfg(target_os = "windows")]
impl ApiExt for hal::api::Dx12 {}

#[cfg(any(target_os = "macos", target_os = "ios"))]
impl ApiExt for hal::api::Metal {}

#[cfg(any(target_os = "linux", target_os = "android", target_os = "windows"))]
mod vulkan {
    use ash::vk::Handle as _;
    use wgpu_hal::vulkan as vk;

    use super::*;

    impl ApiExt for hal::api::Vulkan {
        /// Places the buffer in write-combined system RAM, allocated through
        /// `ash` (see [`create_wc_buffer`]). `gpu-allocator`, which `wgpu-hal`
        /// allocates through, offers no way to ask for that memory and prefers
        /// device-local BAR memory for `MAP_WRITE`, where CPU writes are several
        /// times slower.
        fn create_stream_buffer(
            device: &vk::Device,
            _label: &str,
            size: u64,
            usage: BufferUses,
        ) -> Result<(vk::Buffer, Option<Mapped>), RedixelError> {
            let raw_usage: ash::vk::BufferUsageFlags = if usage.contains(BufferUses::INDEX) {
                ash::vk::BufferUsageFlags::INDEX_BUFFER
            } else if usage.contains(BufferUses::UNIFORM) {
                ash::vk::BufferUsageFlags::UNIFORM_BUFFER
            } else {
                ash::vk::BufferUsageFlags::VERTEX_BUFFER
            };
            let (buffer, mapped): (vk::Buffer, Mapped) = create_wc_buffer(device, size, raw_usage)?;
            Ok((buffer, Some(mapped)))
        }

        fn surface_image_key(texture: &vk::Texture) -> Option<u64> {
            // SAFETY: the handle is only read, never destroyed through this copy.
            Some(unsafe { texture.raw_handle() }.as_raw())
        }
    }

    /// Picks a host-visible, host-coherent memory type, preferring one that is
    /// neither device-local nor host-cached: write-combined system RAM, which
    /// the CPU fills at full speed with sequential writes and the GPU reads over
    /// PCIe without snooping the CPU caches. On unified memory (integrated GPUs,
    /// phones, software rasterisers) every type is device-local, so any
    /// host-visible coherent type is taken.
    fn wc_memory_type(device: &vk::Device, type_bits: u32) -> Option<u32> {
        // SAFETY: the physical device belongs to the instance it is queried through.
        let properties: ash::vk::PhysicalDeviceMemoryProperties = unsafe {
            device
                .shared_instance()
                .raw_instance()
                .get_physical_device_memory_properties(device.raw_physical_device())
        };

        let required: ash::vk::MemoryPropertyFlags =
            ash::vk::MemoryPropertyFlags::HOST_VISIBLE | ash::vk::MemoryPropertyFlags::HOST_COHERENT;
        let preferences: [ash::vk::MemoryPropertyFlags; 4] = [
            ash::vk::MemoryPropertyFlags::DEVICE_LOCAL | ash::vk::MemoryPropertyFlags::HOST_CACHED,
            ash::vk::MemoryPropertyFlags::DEVICE_LOCAL,
            ash::vk::MemoryPropertyFlags::HOST_CACHED,
            ash::vk::MemoryPropertyFlags::empty(),
        ];

        preferences.iter().find_map(|excluded: &ash::vk::MemoryPropertyFlags| {
            (0..properties.memory_type_count).find(|&index: &u32| {
                let flags: ash::vk::MemoryPropertyFlags = properties.memory_types[index as usize].property_flags;
                type_bits & (1 << index) != 0 && flags.contains(required) && !flags.intersects(*excluded)
            })
        })
    }

    /// Allocates a `size`-byte buffer in write-combined system RAM through
    /// `ash`, maps it for good, and hands it to `wgpu-hal`, which from then on
    /// owns both the buffer and its memory and frees them in `destroy_buffer`.
    ///
    /// `wgpu-hal` cannot map a buffer it did not allocate, so the mapping made
    /// here is the only one; freeing the memory unmaps it implicitly.
    fn create_wc_buffer(
        device: &vk::Device,
        size: u64,
        usage: ash::vk::BufferUsageFlags,
    ) -> Result<(vk::Buffer, Mapped), RedixelError> {
        let raw: &ash::Device = device.raw_device();

        // SAFETY: every handle created here is either handed to `wgpu-hal` or
        // destroyed on the error path before returning.
        unsafe {
            let buffer: ash::vk::Buffer = raw
                .create_buffer(
                    &ash::vk::BufferCreateInfo::default()
                        .size(size)
                        .usage(usage)
                        .sharing_mode(ash::vk::SharingMode::EXCLUSIVE),
                    None,
                )
                .map_err(gpu_err)?;

            let requirements: ash::vk::MemoryRequirements = raw.get_buffer_memory_requirements(buffer);
            let Some(memory_type) = wc_memory_type(device, requirements.memory_type_bits) else {
                raw.destroy_buffer(buffer, None);
                return Err(gpu_err("no host-visible coherent memory type for streaming buffers"));
            };

            let memory: ash::vk::DeviceMemory = match raw.allocate_memory(
                &ash::vk::MemoryAllocateInfo::default()
                    .allocation_size(requirements.size)
                    .memory_type_index(memory_type),
                None,
            ) {
                Ok(memory) => memory,
                Err(e) => {
                    raw.destroy_buffer(buffer, None);
                    return Err(gpu_err(e));
                }
            };

            let mapped: *mut std::ffi::c_void = match raw
                .bind_buffer_memory(buffer, memory, 0)
                .and_then(|()| raw.map_memory(memory, 0, ash::vk::WHOLE_SIZE, ash::vk::MemoryMapFlags::empty()))
            {
                Ok(mapped) => mapped,
                Err(e) => {
                    raw.destroy_buffer(buffer, None);
                    raw.free_memory(memory, None);
                    return Err(gpu_err(e));
                }
            };

            let Some(pointer) = NonNull::new(mapped.cast::<u8>()) else {
                raw.destroy_buffer(buffer, None);
                raw.free_memory(memory, None);
                return Err(gpu_err("vkMapMemory returned a null pointer"));
            };

            Ok((
                vk::Buffer::from_raw_managed(buffer, memory, 0, requirements.size),
                Mapped(pointer),
            ))
        }
    }
}
