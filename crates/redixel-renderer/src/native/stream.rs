//! The buffers a frame slot rewrites every frame: triangle vertices and
//! indices, and quad instances.

use std::ptr;

use hal::Device as _;
use redixel_core::RedixelError;
use wgpu_hal as hal;
use wgpu_types::BufferUses;

use super::api::{ApiExt, Mapped, write_mapped};
use crate::{quad::QuadInstance, shader::Vertex};

/// One growing buffer, written from the CPU and read by the GPU in the same
/// frame slot.
pub(super) struct StreamBuffer<A: ApiExt> {
    buffer:   A::Buffer,
    mapped:   Option<Mapped>,
    capacity: usize,
}

impl<A: ApiExt> StreamBuffer<A> {
    /// The buffer to bind for drawing.
    pub(super) fn buffer(&self) -> &A::Buffer {
        &self.buffer
    }

    /// Makes `slot` hold at least `bytes` bytes, replacing a smaller buffer.
    /// Capacity climbs to the next power of two and never shrinks.
    ///
    /// # Safety
    ///
    /// The owning frame slot's fence has passed, so a buffer replaced here is
    /// no longer read by the GPU.
    unsafe fn reserve(
        device: &A::Device,
        slot: &mut Option<Self>,
        bytes: usize,
        label: &str,
        usage: BufferUses,
    ) -> Result<(), RedixelError> {
        if slot.as_ref().is_some_and(|b: &Self| b.capacity >= bytes) {
            return Ok(());
        }

        let capacity: usize = bytes.next_power_of_two();
        let (buffer, mapped): (A::Buffer, Option<Mapped>) =
            A::create_stream_buffer(device, label, capacity as u64, usage)?;
        if let Some(old) = slot.replace(Self {
            buffer,
            mapped,
            capacity,
        }) {
            unsafe { old.destroy(device) };
        }

        Ok(())
    }

    /// # Safety
    ///
    /// The owning frame slot's fence has passed, and `bytes` fits the capacity.
    unsafe fn write(&self, device: &A::Device, bytes: &[u8]) -> Result<(), RedixelError> {
        match self.mapped {
            Some(Mapped(pointer)) => {
                unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), pointer.as_ptr(), bytes.len()) };
                Ok(())
            }
            None => unsafe { write_mapped::<A>(device, &self.buffer, bytes) },
        }
    }

    /// # Safety
    ///
    /// No pending submission references the buffer.
    unsafe fn destroy(self, device: &A::Device) {
        unsafe { device.destroy_buffer(self.buffer) };
    }
}

/// One buffer [`Stream::upload`] fills: its slot, the bytes, a label and
/// the usage it is created with.
type Part<'a, A> = (&'a mut Option<StreamBuffer<A>>, &'a [u8], &'static str, BufferUses);

/// A frame slot's streaming buffers. Each is allocated on first use.
pub(super) struct Stream<A: ApiExt> {
    pub(super) vertex:   Option<StreamBuffer<A>>,
    pub(super) index:    Option<StreamBuffer<A>>,
    pub(super) instance: Option<StreamBuffer<A>>,
}

impl<A: ApiExt> Default for Stream<A> {
    fn default() -> Self {
        Self {
            vertex:   None,
            index:    None,
            instance: None,
        }
    }
}

impl<A: ApiExt> Stream<A> {
    /// Writes this frame's vertices, indices and quad instances. Empty parts
    /// are skipped and keep whatever buffer they had.
    ///
    /// # Safety
    ///
    /// The owning frame slot's fence has passed.
    pub(super) unsafe fn upload(
        &mut self,
        device: &A::Device,
        vertices: &[Vertex],
        indices: &[u32],
        quads: &[QuadInstance],
    ) -> Result<(), RedixelError> {
        let parts: [Part<'_, A>; 3] = [
            (
                &mut self.vertex,
                bytemuck::cast_slice(vertices),
                "REDIXEL_STREAM_VB",
                BufferUses::VERTEX,
            ),
            (
                &mut self.index,
                bytemuck::cast_slice(indices),
                "REDIXEL_STREAM_IB",
                BufferUses::INDEX,
            ),
            (
                &mut self.instance,
                bytemuck::cast_slice(quads),
                "REDIXEL_STREAM_QUADS",
                BufferUses::VERTEX,
            ),
        ];

        for (slot, bytes, label, usage) in parts {
            if bytes.is_empty() {
                continue;
            }
            unsafe {
                StreamBuffer::reserve(device, slot, bytes.len(), label, usage)?;
                if let Some(buffer) = slot.as_ref() {
                    buffer.write(device, bytes)?;
                }
            }
        }

        Ok(())
    }

    /// # Safety
    ///
    /// No pending submission references these buffers.
    pub(super) unsafe fn destroy(self, device: &A::Device) {
        for buffer in [self.vertex, self.index, self.instance].into_iter().flatten() {
            unsafe { buffer.destroy(device) };
        }
    }
}
