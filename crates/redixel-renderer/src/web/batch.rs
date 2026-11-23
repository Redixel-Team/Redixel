use redixel_core::TextureId;
use redixel_math::{Color, Vec2, Vec3};
use redixel_profile as profile;
use wgpu::{Buffer, BufferDescriptor, BufferUsages, Device, IndexFormat, Queue, RenderPass};

use super::{pipeline::ShapePipeline, texture::TextureRegistry};
use crate::{
    geometry::{Geometry, TRIANGLE_INDICES, triangle_vertices_3d},
    quad::{Batch2d, RunKind},
    shader::Vertex,
};

/// A GPU buffer rewritten in full every frame, grown on demand.
///
/// Starts unallocated, so a frame that never queues anything pays nothing.
/// Capacity climbs to the next power of two and never shrinks, so a scene whose
/// sprite count oscillates settles and stops reallocating.
struct GrowingBuffer {
    label:    &'static str,
    usage:    BufferUsages,
    buffer:   Option<Buffer>,
    capacity: u64,
}

impl GrowingBuffer {
    fn new(label: &'static str, usage: BufferUsages) -> Self {
        Self {
            label,
            usage: usage | BufferUsages::COPY_DST,
            buffer: None,
            capacity: 0,
        }
    }

    /// Uploads `bytes` to the start of the buffer, growing it first if needed,
    /// and returns it. `None` when `bytes` is empty.
    fn write(&mut self, device: &Device, queue: &Queue, bytes: &[u8]) -> Option<&Buffer> {
        let needed: u64 = bytes.len() as u64;
        if needed == 0 {
            return None;
        }

        if needed > self.capacity || self.buffer.is_none() {
            self.capacity = needed.next_power_of_two();
            self.buffer = Some(device.create_buffer(&BufferDescriptor {
                label:              Some(self.label),
                size:               self.capacity,
                usage:              self.usage,
                mapped_at_creation: false,
            }));
        }

        let buffer: &Buffer = self.buffer.as_ref()?;
        queue.write_buffer(buffer, 0, bytes);
        Some(buffer)
    }
}

/// The GPU-side half of the 3D batch.
///
/// Imposes no ceiling on how much geometry a frame may queue: the buffers grow
/// to fit whatever is drawn, so nothing is ever silently dropped.
struct GeometryBuffers {
    vertex_buffer: GrowingBuffer,
    index_buffer:  GrowingBuffer,
    geometry:      Geometry,
}

impl GeometryBuffers {
    fn new(label_vertices: &'static str, label_indices: &'static str) -> Self {
        Self {
            vertex_buffer: GrowingBuffer::new(label_vertices, BufferUsages::VERTEX),
            index_buffer:  GrowingBuffer::new(label_indices, BufferUsages::INDEX),
            geometry:      Geometry::default(),
        }
    }

    fn push(&mut self, vertices: &[Vertex], indices: &[u32], texture: Option<TextureId>) {
        self.geometry.push(vertices, indices, texture);
    }

    /// Uploads queued geometry and records one indexed draw call per texture
    /// run. Must be called **inside** an active `RenderPass`; clears the queue.
    ///
    /// The whole frame travels in two `write_buffer` calls; only the draw is
    /// split, because a bind group can be swapped between draws but not within
    /// one.
    fn flush(&mut self, device: &Device, queue: &Queue, pass: &mut RenderPass<'_>, textures: &TextureRegistry) {
        if self.geometry.indices.is_empty() {
            return;
        }

        profile::phase("buffer_upload");
        let vertices: Option<&Buffer> =
            self.vertex_buffer
                .write(device, queue, bytemuck::cast_slice(&self.geometry.vertices));
        if let Some(buffer) = vertices {
            pass.set_vertex_buffer(0, buffer.slice(..));
        }
        let indices: Option<&Buffer> =
            self.index_buffer
                .write(device, queue, bytemuck::cast_slice(&self.geometry.indices));
        if let Some(buffer) = indices {
            pass.set_index_buffer(buffer.slice(..), IndexFormat::Uint32);
        }

        profile::phase("draw");
        for run in &self.geometry.runs {
            pass.set_bind_group(1, textures.bind_group(run.texture), &[]);
            pass.draw_indexed(run.start..run.end, 0, 0..1);
        }

        self.geometry.clear();
    }
}

/// Accumulates 3D triangles per frame, submitting them in as few indexed draw
/// calls as the textures allow.
///
/// Separate from the 2D [`Batch2d`] because it flushes against the perspective
/// camera and the depth-testing pipeline: mixing a `draw_rect` in would put a
/// pixel-space rectangle through a 3D camera.
pub struct MeshBatch {
    buffers: GeometryBuffers,
}

impl MeshBatch {
    pub fn new() -> Self {
        Self {
            buffers: GeometryBuffers::new("REDIXEL_MESH_BATCH_VB", "REDIXEL_MESH_BATCH_IB"),
        }
    }

    /// Queues a filled triangle in 3D view space.
    ///
    /// - `p1`, `p2`, `p3` — the three vertices, in the perspective camera's
    ///   view space (see [`redixel_math::Mat4::perspective`]). Winding is not
    ///   significant; nothing is culled.
    /// - `color`          — RGBA fill colour
    pub fn draw_triangle_3d(&mut self, p1: Vec3, p2: Vec3, p3: Vec3, color: Color) {
        self.buffers.push(
            &triangle_vertices_3d([p1, p2, p3], [Vec2::ZERO; 3], color),
            &TRIANGLE_INDICES,
            None,
        );
    }

    /// Queues a textured triangle in 3D view space.
    ///
    /// - `points`  — the three vertices, in the perspective camera's view space
    /// - `uvs`     — the texture coordinate for each vertex, in the same order
    /// - `texture` — the image to sample
    /// - `tint`    — multiplied into every sampled texel; `Color::WHITE` is the no-op
    pub fn draw_triangle_3d_textured(&mut self, points: [Vec3; 3], uvs: [Vec2; 3], texture: TextureId, tint: Color) {
        self.buffers
            .push(&triangle_vertices_3d(points, uvs, tint), &TRIANGLE_INDICES, Some(texture));
    }

    /// Returns the number of unique vertices currently queued — three per
    /// triangle.
    ///
    /// Since the batch draws indexed, this is *not* the size of the draw call;
    /// see [`MeshBatch::index_count`].
    pub fn unique_vertex_count(&self) -> usize {
        self.buffers.geometry.vertices.len()
    }

    /// Returns the number of indices currently queued, which is the vertex
    /// count the next `flush()` will submit.
    pub fn index_count(&self) -> usize {
        self.buffers.geometry.indices.len()
    }

    /// Uploads queued geometry and records the indexed draw calls.
    ///
    /// Must be called **inside** an active `RenderPass`, with the 3D pipeline
    /// and perspective camera already bound. Clears the queue afterwards.
    pub fn flush(&mut self, device: &Device, queue: &Queue, pass: &mut RenderPass<'_>, textures: &TextureRegistry) {
        self.buffers.flush(device, queue, pass, textures);
    }
}

impl Default for MeshBatch {
    fn default() -> Self {
        Self::new()
    }
}

/// The GPU-side half of the 2D [`Batch2d`]: one growing buffer each for its
/// quad instances, triangle vertices and triangle indices.
pub(crate) struct Batch2dBuffers {
    instances: GrowingBuffer,
    vertices:  GrowingBuffer,
    indices:   GrowingBuffer,
}

impl Batch2dBuffers {
    pub(crate) fn new() -> Self {
        Self {
            instances: GrowingBuffer::new("REDIXEL_BATCH_2D_QUADS", BufferUsages::VERTEX),
            vertices:  GrowingBuffer::new("REDIXEL_BATCH_2D_VB", BufferUsages::VERTEX),
            indices:   GrowingBuffer::new("REDIXEL_BATCH_2D_IB", BufferUsages::INDEX),
        }
    }

    /// Uploads `batch` and records its runs in submission order, switching
    /// between the instanced quad pipeline and the indexed triangle pipeline at
    /// run boundaries, then clears it.
    ///
    /// Must be called **inside** an active `RenderPass` with the 2D globals bound
    /// at group 0. Both pipelines share one layout, so the group stays bound
    /// across the switch.
    pub(crate) fn flush(
        &mut self,
        device: &Device,
        queue: &Queue,
        pass: &mut RenderPass<'_>,
        pipelines: &ShapePipeline,
        textures: &TextureRegistry,
        batch: &mut Batch2d,
    ) {
        if batch.is_empty() {
            return;
        }

        batch.finish();

        profile::phase("buffer_upload");
        let instances: Option<&Buffer> = self.instances.write(device, queue, bytemuck::cast_slice(&batch.quads));
        let vertices: Option<&Buffer> = self
            .vertices
            .write(device, queue, bytemuck::cast_slice(&batch.vertices));
        let indices: Option<&Buffer> = self.indices.write(device, queue, bytemuck::cast_slice(&batch.indices));

        profile::phase("draw");
        let mut bound: Option<RunKind> = None;
        for run in &batch.runs {
            if bound != Some(run.kind) {
                match (run.kind, instances, vertices, indices) {
                    (RunKind::Quads, Some(instances), _, _) => {
                        pass.set_pipeline(&pipelines.pipeline_quads);
                        pass.set_vertex_buffer(0, instances.slice(..));
                    }
                    (RunKind::Triangles, _, Some(vertices), Some(indices)) => {
                        pass.set_pipeline(&pipelines.pipeline_2d);
                        pass.set_vertex_buffer(0, vertices.slice(..));
                        pass.set_index_buffer(indices.slice(..), IndexFormat::Uint32);
                    }
                    _ => continue,
                }
                bound = Some(run.kind);
            }

            pass.set_bind_group(1, textures.bind_group(run.texture), &[]);
            match run.kind {
                RunKind::Quads => pass.draw(0..4, run.start..run.end),
                RunKind::Triangles => pass.draw_indexed(run.start..run.end, 0, 0..1),
            }
        }

        batch.clear();
    }
}
