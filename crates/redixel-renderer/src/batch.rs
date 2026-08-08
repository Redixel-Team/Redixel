use wgpu::{Buffer, BufferDescriptor, BufferUsages, Device, IndexFormat, Queue, RenderPass};

use redixel_core::TextureId;
use redixel_math::{Color, Vec2, Vec3};

use crate::{pipeline::Vertex, texture::TextureRegistry};

/// Relative to the rectangle's own first vertex. Clockwise in world space,
/// which the camera's y-flipping projection turns into the `FrontFace::Ccw`
/// the shape pipeline declares.
const RECT_INDICES: [u32; 6] = [0, 2, 1, 1, 2, 3];

const TRIANGLE_INDICES: [u32; 3] = [0, 1, 2];

/// Untextured geometry samples the registry's 1×1 white pixel, so every corner
/// collapses onto the same texel and the sample multiplies the colour by one.
const SOLID_UVS: [[f32; 2]; 4] = [[0.0, 0.0]; 4];

/// Maps a whole texture onto a quad, in the corner order [`quad_vertices`] builds.
const SPRITE_UVS: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];

/// Builds the four corners of an axis-aligned quad in
/// `[top-left, top-right, bottom-left, bottom-right]` order, the order
/// [`RECT_INDICES`] expects.
fn quad_vertices(position: Vec2, size: Vec2, color: Color, uvs: [[f32; 2]; 4]) -> [Vertex; 4] {
    let x0: f32 = position.x;
    let y0: f32 = position.y;
    let x1: f32 = position.x + size.x;
    let y1: f32 = position.y + size.y;
    let c: [f32; 4] = color.to_array();

    [
        Vertex {
            position: [x0, y0, 0.0],
            color: c,
            uv: uvs[0],
        },
        Vertex {
            position: [x1, y0, 0.0],
            color: c,
            uv: uvs[1],
        },
        Vertex {
            position: [x0, y1, 0.0],
            color: c,
            uv: uvs[2],
        },
        Vertex {
            position: [x1, y1, 0.0],
            color: c,
            uv: uvs[3],
        },
    ]
}

/// Builds the three corners of a triangle in the order they were given.
fn triangle_vertices(p1: Vec2, p2: Vec2, p3: Vec2, color: Color) -> [Vertex; 3] {
    let c: [f32; 4] = color.to_array();

    [
        Vertex {
            position: [p1.x, p1.y, 0.0],
            color: c,
            uv: SOLID_UVS[0],
        },
        Vertex {
            position: [p2.x, p2.y, 0.0],
            color: c,
            uv: SOLID_UVS[0],
        },
        Vertex {
            position: [p3.x, p3.y, 0.0],
            color: c,
            uv: SOLID_UVS[0],
        },
    ]
}

/// Builds the three corners of a 3D triangle in the order they were given,
/// pairing each with the matching entry of `uvs`.
///
/// The only vertex-building path whose depth isn't hardcoded to zero.
fn triangle_vertices_3d(points: [Vec3; 3], uvs: [Vec2; 3], color: Color) -> [Vertex; 3] {
    let c: [f32; 4] = color.to_array();

    std::array::from_fn(|i: usize| Vertex {
        position: points[i].to_array(),
        color: c,
        uv: [uvs[i].x, uvs[i].y],
    })
}

/// Builds the three corners of a 3D triangle as [`triangle_vertices_3d`] does,
/// but with an independent colour per corner, which the rasteriser then
/// interpolates across the face.
fn triangle_vertices_3d_shaded(points: [Vec3; 3], colors: [Color; 3]) -> [Vertex; 3] {
    std::array::from_fn(|i: usize| Vertex {
        position: points[i].to_array(),
        color: colors[i].to_array(),
        uv: [0.0, 0.0],
    })
}

/// A contiguous slice of the index buffer sharing one texture, and so
/// submittable as a single draw call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DrawRun {
    texture: Option<TextureId>,
    start: u32,
    end: u32,
}

/// Geometry accumulated on the CPU during a frame.
///
/// Owns no GPU resources, so index rebasing and run coalescing can be exercised
/// without a device.
#[derive(Default)]
struct Geometry {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    runs: Vec<DrawRun>,
}

impl Geometry {
    /// Appends `vertices` and the `indices` referencing them, rebasing each
    /// index onto the geometry already queued and recording its texture.
    ///
    /// Indices arrive relative to the primitive's own first vertex, so a
    /// primitive never has to know how much geometry precedes it; this is the
    /// only place that offset is computed. Consecutive primitives sharing a
    /// texture extend the open run rather than opening a new one.
    fn push(&mut self, vertices: &[Vertex], indices: &[u32], texture: Option<TextureId>) {
        let base: u32 = self.vertices.len() as u32;
        let start: u32 = self.indices.len() as u32;

        self.vertices.extend_from_slice(vertices);
        self.indices.extend(indices.iter().map(|i: &u32| base + i));

        let end: u32 = self.indices.len() as u32;

        match self.runs.last_mut() {
            Some(run) if run.texture == texture => run.end = end,
            _ => self.runs.push(DrawRun { texture, start, end }),
        }
    }

    fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.runs.clear();
    }
}

fn create_vertex_buffer(device: &Device, vertices: usize) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some("REDIXEL_SPRITE_BATCH_VB"),
        size: (vertices * std::mem::size_of::<Vertex>()) as u64,
        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_index_buffer(device: &Device, indices: usize) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some("REDIXEL_SPRITE_BATCH_IB"),
        size: (indices * std::mem::size_of::<u32>()) as u64,
        usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// The GPU-side half of a batch, shared by [`SpriteBatch`] and [`MeshBatch`].
///
/// Imposes no ceiling on how much geometry a frame may queue and reserves
/// nothing up front: buffers start empty and grow to fit whatever is drawn, so
/// nothing is ever silently dropped.
struct GeometryBuffers {
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    vertex_capacity: usize,
    index_capacity: usize,
    geometry: Geometry,
}

impl GeometryBuffers {
    /// The first `flush()` sizes the buffers to the frame actually drawn, so a
    /// game that draws little pays for little.
    fn new(device: &Device) -> Self {
        Self {
            vertex_buffer: create_vertex_buffer(device, 0),
            index_buffer: create_index_buffer(device, 0),
            vertex_capacity: 0,
            index_capacity: 0,
            geometry: Geometry::default(),
        }
    }

    fn push(&mut self, vertices: &[Vertex], indices: &[u32], texture: Option<TextureId>) {
        self.geometry.push(vertices, indices, texture);
    }

    fn unique_vertex_count(&self) -> usize {
        self.geometry.vertices.len()
    }

    fn index_count(&self) -> usize {
        self.geometry.indices.len()
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

        self.reserve(device);

        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&self.geometry.vertices));
        queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&self.geometry.indices));

        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), IndexFormat::Uint32);

        for run in &self.geometry.runs {
            pass.set_bind_group(1, textures.bind_group(run.texture), &[]);
            pass.draw_indexed(run.start..run.end, 0, 0..1);
        }

        self.geometry.clear();
    }

    /// Capacity climbs to the next power of two and never shrinks, so a scene
    /// whose sprite count oscillates settles and stops reallocating.
    fn reserve(&mut self, device: &Device) {
        let vertices: usize = self.geometry.vertices.len();
        if vertices > self.vertex_capacity {
            self.vertex_capacity = vertices.next_power_of_two();
            self.vertex_buffer = create_vertex_buffer(device, self.vertex_capacity);
        }

        let indices: usize = self.geometry.indices.len();
        if indices > self.index_capacity {
            self.index_capacity = indices.next_power_of_two();
            self.index_buffer = create_index_buffer(device, self.index_capacity);
        }
    }
}

/// Accumulates 2D shape and sprite draw calls per frame, submitting them in as
/// few indexed draw calls as the textures allow.
///
/// Solid shapes and sprites share one batch rather than living in two, because
/// splitting them would split their painter order — a sprite drawn after a
/// rectangle has to land on top of it.
///
/// Every vertex here sits at `z = 0` and flushes through the pipeline that
/// neither tests nor writes depth. For depth-tested geometry see [`MeshBatch`].
pub struct SpriteBatch {
    buffers: GeometryBuffers,
}

impl SpriteBatch {
    pub fn new(device: &Device) -> Self {
        Self {
            buffers: GeometryBuffers::new(device),
        }
    }

    /// Queues a filled rectangle for drawing.
    ///
    /// - `position` — top-left corner in world coordinates (y-down)
    /// - `size`     — width × height in world units
    /// - `color`    — RGBA fill colour
    pub fn draw_rect(&mut self, position: Vec2, size: Vec2, color: Color) {
        self.buffers
            .push(&quad_vertices(position, size, color, SOLID_UVS), &RECT_INDICES, None);
    }

    /// Queues a textured rectangle for drawing.
    ///
    /// - `position` — top-left corner in world coordinates (y-down)
    /// - `size`     — width × height in world units
    /// - `texture`  — the image to stretch across the quad
    /// - `tint`     — multiplied into every sampled texel; `Color::WHITE` is the no-op
    pub fn draw_sprite(&mut self, position: Vec2, size: Vec2, texture: TextureId, tint: Color) {
        self.buffers
            .push(&quad_vertices(position, size, tint, SPRITE_UVS), &RECT_INDICES, Some(texture));
    }

    /// Queues a filled triangle for drawing.
    ///
    /// - `p1`, `p2`, `p3` — The three corners of the triangle in world coordinates
    /// - `color`          — RGBA fill colour
    pub fn draw_triangle(&mut self, p1: Vec2, p2: Vec2, p3: Vec2, color: Color) {
        self.buffers
            .push(&triangle_vertices(p1, p2, p3, color), &TRIANGLE_INDICES, None);
    }

    /// Returns the number of unique vertices currently queued — four per
    /// rectangle, three per triangle.
    ///
    /// Since the batch draws indexed, this is *not* the size of the draw call;
    /// see [`SpriteBatch::index_count`].
    pub fn unique_vertex_count(&self) -> usize {
        self.buffers.unique_vertex_count()
    }

    /// Returns the number of indices currently queued, which is the vertex
    /// count the next `flush()` will submit.
    pub fn index_count(&self) -> usize {
        self.buffers.index_count()
    }

    /// Uploads queued geometry and records the indexed draw calls.
    ///
    /// Must be called **inside** an active `RenderPass`, with the 2D pipeline
    /// and orthographic camera already bound. Clears the queue afterwards.
    pub fn flush(&mut self, device: &Device, queue: &Queue, pass: &mut RenderPass<'_>, textures: &TextureRegistry) {
        self.buffers.flush(device, queue, pass, textures);
    }
}

/// Accumulates 3D triangles per frame, submitting them in as few indexed draw
/// calls as the textures allow.
///
/// Separate from [`SpriteBatch`] because it flushes against the perspective
/// camera and the depth-testing pipeline: mixing a `draw_rect` in would put a
/// pixel-space rectangle through a 3D camera.
pub struct MeshBatch {
    buffers: GeometryBuffers,
}

impl MeshBatch {
    pub fn new(device: &Device) -> Self {
        Self {
            buffers: GeometryBuffers::new(device),
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

    /// Queues a triangle in 3D view space with an independent colour per
    /// vertex, interpolated across the face.
    ///
    /// This is the building block for smooth (Gouraud) shading and for soft
    /// gradients — a vertex whose colour carries zero alpha fades the face out
    /// towards that corner.
    pub fn draw_triangle_3d_shaded(&mut self, points: [Vec3; 3], colors: [Color; 3]) {
        self.buffers
            .push(&triangle_vertices_3d_shaded(points, colors), &TRIANGLE_INDICES, None);
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
        self.buffers.unique_vertex_count()
    }

    /// Returns the number of indices currently queued, which is the vertex
    /// count the next `flush()` will submit.
    pub fn index_count(&self) -> usize {
        self.buffers.index_count()
    }

    /// Uploads queued geometry and records the indexed draw calls.
    ///
    /// Must be called **inside** an active `RenderPass`, with the 3D pipeline
    /// and perspective camera already bound. Clears the queue afterwards.
    pub fn flush(&mut self, device: &Device, queue: &Queue, pass: &mut RenderPass<'_>, textures: &TextureRegistry) {
        self.buffers.flush(device, queue, pass, textures);
    }
}

#[cfg(test)]
mod tests {
    use redixel_math::Mat4;

    use super::*;

    fn positions(vertices: &[Vertex]) -> Vec<[f32; 3]> {
        vertices.iter().map(|v: &Vertex| v.position).collect()
    }

    fn uvs(vertices: &[Vertex]) -> Vec<[f32; 2]> {
        vertices.iter().map(|v: &Vertex| v.uv).collect()
    }

    fn project(vertices: &[Vertex]) -> [Vertex; 4] {
        let projection: Mat4 = Mat4::orthographic(0.0, 800.0, 600.0, 0.0, -1.0, 1.0);

        std::array::from_fn(|i: usize| Vertex {
            position: [
                projection.cols[0][0] * vertices[i].position[0] + projection.cols[3][0],
                projection.cols[1][1] * vertices[i].position[1] + projection.cols[3][1],
                0.0,
            ],
            color: vertices[i].color,
            uv: vertices[i].uv,
        })
    }

    fn signed_area(vertices: &[Vertex], indices: &[u32], triangle: usize) -> f32 {
        let i: usize = triangle * 3;
        let a: [f32; 3] = vertices[indices[i] as usize].position;
        let b: [f32; 3] = vertices[indices[i + 1] as usize].position;
        let c: [f32; 3] = vertices[indices[i + 2] as usize].position;

        (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
    }

    fn solid_rect() -> [Vertex; 4] {
        quad_vertices(Vec2::ZERO, Vec2::ONE, Color::WHITE, SOLID_UVS)
    }

    fn sprite_rect() -> [Vertex; 4] {
        quad_vertices(Vec2::ZERO, Vec2::ONE, Color::WHITE, SPRITE_UVS)
    }

    #[test]
    fn push_leaves_indices_alone_on_empty_geometry() {
        let mut geometry: Geometry = Geometry::default();
        geometry.push(&solid_rect(), &RECT_INDICES, None);

        assert_eq!(geometry.indices, RECT_INDICES.to_vec());
    }

    #[test]
    fn push_rebases_indices_onto_queued_vertices() {
        let mut geometry: Geometry = Geometry::default();
        geometry.push(&triangle_vertices(Vec2::ZERO, Vec2::X, Vec2::Y, Color::WHITE), &[0, 1, 2], None);
        geometry.push(&triangle_vertices(Vec2::ONE, Vec2::X, Vec2::Y, Color::RED), &[0, 1, 2], None);
        geometry.push(&triangle_vertices(Vec2::ZERO, Vec2::Y, Vec2::X, Color::BLUE), &[0, 1, 2], None);

        assert_eq!(geometry.vertices.len(), 9);
        assert_eq!(geometry.indices, vec![0, 1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn rect_vertices_are_ordered_tl_tr_bl_br() {
        let vertices: [Vertex; 4] = quad_vertices(Vec2::new(10.0, 20.0), Vec2::new(3.0, 4.0), Color::WHITE, SOLID_UVS);

        assert_eq!(
            positions(&vertices),
            vec![
                [10.0, 20.0, 0.0],
                [13.0, 20.0, 0.0],
                [10.0, 24.0, 0.0],
                [13.0, 24.0, 0.0]
            ]
        );
    }

    #[test]
    fn rect_index_pattern_is_pinned() {
        assert_eq!(RECT_INDICES, [0, 2, 1, 1, 2, 3]);
    }

    #[test]
    fn rect_triangles_wind_counter_clockwise_in_ndc() {
        let vertices: [Vertex; 4] = project(&solid_rect());

        let first: f32 = signed_area(&vertices, &RECT_INDICES, 0);
        let second: f32 = signed_area(&vertices, &RECT_INDICES, 1);

        assert!(first > 0.0, "the shape pipeline declares FrontFace::Ccw");
        assert!(second > 0.0, "the shape pipeline declares FrontFace::Ccw");
    }

    #[test]
    fn consecutive_rects_offset_their_base_vertex() {
        let mut geometry: Geometry = Geometry::default();
        geometry.push(&solid_rect(), &RECT_INDICES, None);
        geometry.push(&solid_rect(), &RECT_INDICES, None);

        assert_eq!(geometry.vertices.len(), 8);
        assert_eq!(geometry.indices, vec![0, 2, 1, 1, 2, 3, 4, 6, 5, 5, 6, 7]);
    }

    #[test]
    fn triangle_pushes_three_sequential_indices() {
        let mut geometry: Geometry = Geometry::default();
        geometry.push(
            &triangle_vertices(Vec2::ZERO, Vec2::X, Vec2::Y, Color::WHITE),
            &TRIANGLE_INDICES,
            None,
        );

        assert_eq!(geometry.vertices.len(), 3);
        assert_eq!(geometry.indices, vec![0, 1, 2]);
    }

    #[test]
    fn mixing_rects_and_triangles_keeps_indices_in_range() {
        let mut geometry: Geometry = Geometry::default();
        geometry.push(
            &triangle_vertices(Vec2::ZERO, Vec2::X, Vec2::Y, Color::WHITE),
            &TRIANGLE_INDICES,
            None,
        );
        geometry.push(&solid_rect(), &RECT_INDICES, None);
        geometry.push(
            &triangle_vertices(Vec2::ONE, Vec2::X, Vec2::Y, Color::BLUE),
            &TRIANGLE_INDICES,
            None,
        );

        assert_eq!(geometry.vertices.len(), 10);
        assert_eq!(geometry.indices.len(), 12);

        let highest: u32 = *geometry.indices.iter().max().unwrap();
        assert!(highest < geometry.vertices.len() as u32);
    }

    #[test]
    fn geometry_has_no_ceiling() {
        const QUADS: usize = 50_000;

        let mut geometry: Geometry = Geometry::default();
        for _ in 0..QUADS {
            geometry.push(&solid_rect(), &RECT_INDICES, None);
        }

        assert_eq!(geometry.vertices.len(), QUADS * 4);
        assert_eq!(geometry.indices.len(), QUADS * 6);

        let highest: u32 = *geometry.indices.iter().max().unwrap();
        assert_eq!(highest, geometry.vertices.len() as u32 - 1);
    }

    #[test]
    fn clear_resets_every_queue() {
        let mut geometry: Geometry = Geometry::default();
        geometry.push(&solid_rect(), &RECT_INDICES, None);
        geometry.clear();

        assert!(geometry.vertices.is_empty());
        assert!(geometry.indices.is_empty());
        assert!(geometry.runs.is_empty());
    }

    #[test]
    fn triangle_vertices_3d_carries_real_depth() {
        let vertices: [Vertex; 3] = triangle_vertices_3d(
            [
                Vec3::new(0.0, 1.0, 2.0),
                Vec3::new(-1.0, -1.0, 2.0),
                Vec3::new(1.0, -1.0, 2.0),
            ],
            [Vec2::ZERO; 3],
            Color::WHITE,
        );

        assert_eq!(positions(&vertices), vec![[0.0, 1.0, 2.0], [-1.0, -1.0, 2.0], [1.0, -1.0, 2.0]]);
    }

    #[test]
    fn triangle_3d_pushes_three_sequential_indices() {
        let mut geometry: Geometry = Geometry::default();
        geometry.push(
            &triangle_vertices_3d([Vec3::ZERO, Vec3::X, Vec3::Y], [Vec2::ZERO; 3], Color::WHITE),
            &TRIANGLE_INDICES,
            None,
        );

        assert_eq!(geometry.vertices.len(), 3);
        assert_eq!(geometry.indices, vec![0, 1, 2]);
    }

    #[test]
    fn sprite_uvs_map_the_whole_texture_onto_the_quad_corners() {
        assert_eq!(
            uvs(&sprite_rect()),
            vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]],
            "UVs must follow the tl/tr/bl/br order RECT_INDICES assumes"
        );
    }

    #[test]
    fn solid_geometry_carries_collapsed_uvs() {
        assert!(
            uvs(&solid_rect()).iter().all(|uv: &[f32; 2]| *uv == [0.0, 0.0]),
            "untextured geometry samples a 1x1 white texture; zero keeps the intent obvious"
        );

        assert!(
            uvs(&triangle_vertices(Vec2::ZERO, Vec2::X, Vec2::Y, Color::WHITE))
                .iter()
                .all(|uv: &[f32; 2]| *uv == [0.0, 0.0])
        );
    }

    #[test]
    fn triangle_vertices_3d_carries_its_uvs() {
        let vertices: [Vertex; 3] = triangle_vertices_3d(
            [Vec3::ZERO, Vec3::X, Vec3::Y],
            [Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(0.5, 1.0)],
            Color::WHITE,
        );

        assert_eq!(uvs(&vertices), vec![[0.0, 0.0], [1.0, 0.0], [0.5, 1.0]]);
    }

    #[test]
    fn untextured_geometry_collapses_into_one_run() {
        let mut geometry: Geometry = Geometry::default();
        for _ in 0..16 {
            geometry.push(&solid_rect(), &RECT_INDICES, None);
        }

        assert_eq!(geometry.runs.len(), 1, "a frame with no textures stays one draw call");
        assert_eq!(geometry.runs[0].start, 0);
        assert_eq!(geometry.runs[0].end, geometry.indices.len() as u32);
    }

    #[test]
    fn consecutive_sprites_sharing_a_texture_collapse_into_one_run() {
        let texture: TextureId = TextureId::new(3);

        let mut geometry: Geometry = Geometry::default();
        for _ in 0..8 {
            geometry.push(&sprite_rect(), &RECT_INDICES, Some(texture));
        }

        assert_eq!(geometry.runs.len(), 1);
        assert_eq!(geometry.runs[0].texture, Some(texture));
    }

    #[test]
    fn changing_texture_opens_a_new_run() {
        let first: TextureId = TextureId::new(0);
        let second: TextureId = TextureId::new(1);

        let mut geometry: Geometry = Geometry::default();
        geometry.push(&sprite_rect(), &RECT_INDICES, Some(first));
        geometry.push(&sprite_rect(), &RECT_INDICES, Some(second));
        geometry.push(&sprite_rect(), &RECT_INDICES, Some(first));

        let textures: Vec<Option<TextureId>> = geometry.runs.iter().map(|r: &DrawRun| r.texture).collect();

        assert_eq!(
            textures,
            vec![Some(first), Some(second), Some(first)],
            "reordering runs to merge them would break painter order"
        );
    }

    #[test]
    fn interleaving_textured_and_untextured_preserves_submission_order() {
        let texture: TextureId = TextureId::new(7);

        let mut geometry: Geometry = Geometry::default();
        geometry.push(&solid_rect(), &RECT_INDICES, None);
        geometry.push(&sprite_rect(), &RECT_INDICES, Some(texture));
        geometry.push(&solid_rect(), &RECT_INDICES, None);

        let textures: Vec<Option<TextureId>> = geometry.runs.iter().map(|r: &DrawRun| r.texture).collect();

        assert_eq!(textures, vec![None, Some(texture), None]);
    }

    #[test]
    fn runs_partition_the_index_buffer() {
        let texture: TextureId = TextureId::new(2);

        let mut geometry: Geometry = Geometry::default();
        geometry.push(&solid_rect(), &RECT_INDICES, None);
        geometry.push(&sprite_rect(), &RECT_INDICES, Some(texture));
        geometry.push(
            &triangle_vertices(Vec2::ZERO, Vec2::X, Vec2::Y, Color::WHITE),
            &TRIANGLE_INDICES,
            None,
        );

        let mut cursor: u32 = 0;
        for run in &geometry.runs {
            assert_eq!(run.start, cursor, "a gap between runs would drop geometry");
            assert!(run.end > run.start, "an empty run is a wasted draw call");
            cursor = run.end;
        }

        assert_eq!(
            cursor,
            geometry.indices.len() as u32,
            "a short last run would silently drop the tail"
        );
    }
}
