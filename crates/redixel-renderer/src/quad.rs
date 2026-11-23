//! The instanced 2D queue: rectangles and sprites as 36-byte quad instances,
//! triangles as indexed vertices, interleaved in submission order.
//!
//! Pure CPU data, so the runtime can fill it directly while the game renders
//! and hand it to the renderer without copying.

use redixel_core::TextureId;
use redixel_math::{Color, Vec2};
use wgpu::wgt;

use crate::{
    batch::{TRIANGLE_INDICES, triangle_vertices},
    pipeline::Vertex,
};

/// One rectangle or sprite for the instanced quad pipeline: 36 bytes, against
/// the 168 (four vertices and six indices) the indexed path spends on a quad.
///
/// `quad.wgsl` reads the fields in this order, with `color` as `Unorm8x4`.
/// Solid rectangles collapse `uv_min` and `uv_max` onto the white pixel's
/// texel, as the indexed path does.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct QuadInstance {
    pub position: [f32; 2],
    pub size:     [f32; 2],
    pub color:    [u8; 4],
    pub uv_min:   [f32; 2],
    pub uv_max:   [f32; 2],
}

impl QuadInstance {
    pub const ATTRIBUTES: [wgt::VertexAttribute; 5] = wgpu::vertex_attr_array![
        0 => Float32x2,
        1 => Float32x2,
        2 => Unorm8x4,
        3 => Float32x2,
        4 => Float32x2,
    ];

    pub fn new(position: Vec2, size: Vec2, color: Color, uv_min: [f32; 2], uv_max: [f32; 2]) -> Self {
        Self {
            position: [position.x, position.y],
            size: [size.x, size.y],
            color: color.to_array().map(|channel: f32| (channel * 255.0 + 0.5) as u8),
            uv_min,
            uv_max,
        }
    }
}

/// Whether a 2D run draws instanced quads or indexed triangles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunKind {
    Quads,
    Triangles,
}

/// A contiguous stretch of one kind sharing one texture: instance indices for
/// [`RunKind::Quads`], index-buffer indices for [`RunKind::Triangles`].
#[derive(Debug, Clone, Copy)]
pub struct Run2d {
    pub kind:    RunKind,
    pub texture: Option<TextureId>,
    pub start:   u32,
    pub end:     u32,
}

/// The 2D queue: rectangles and sprites as quad instances, triangles as
/// indexed vertices, and the runs that interleave them in submission order so
/// painter order survives the split.
#[derive(Default)]
pub struct Batch2d {
    pub(crate) vertices: Vec<Vertex>,
    pub(crate) indices:  Vec<u32>,
    pub(crate) quads:    Vec<QuadInstance>,
    pub(crate) runs:     Vec<Run2d>,
}

impl Batch2d {
    /// Queues a filled rectangle; see `SpriteBatch::draw_rect`.
    pub fn push_rect(&mut self, position: Vec2, size: Vec2, color: Color) {
        self.push_quad(QuadInstance::new(position, size, color, [0.0, 0.0], [0.0, 0.0]), None);
    }

    /// Queues a textured rectangle; see `SpriteBatch::draw_sprite`.
    pub fn push_sprite(&mut self, position: Vec2, size: Vec2, texture: TextureId, tint: Color) {
        self.push_quad(QuadInstance::new(position, size, tint, [0.0, 0.0], [1.0, 1.0]), Some(texture));
    }

    /// Queues a filled triangle; see `SpriteBatch::draw_triangle`.
    pub fn push_triangle(&mut self, p1: Vec2, p2: Vec2, p3: Vec2, color: Color) {
        self.push_triangle_vertices(triangle_vertices(p1, p2, p3, color), None);
    }

    fn push_quad(&mut self, quad: QuadInstance, texture: Option<TextureId>) {
        self.open_run(RunKind::Quads, texture);
        self.quads.push(quad);
    }

    fn push_triangle_vertices(&mut self, vertices: [Vertex; 3], texture: Option<TextureId>) {
        self.open_run(RunKind::Triangles, texture);
        let base: u32 = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&vertices);
        self.indices.extend(TRIANGLE_INDICES.iter().map(|i: &u32| base + i));
    }

    /// How far `kind`'s queue reaches: instances for quads, indices for triangles.
    fn reach(&self, kind: RunKind) -> u32 {
        match kind {
            RunKind::Quads => self.quads.len() as u32,
            RunKind::Triangles => self.indices.len() as u32,
        }
    }

    /// Makes the last run one of `kind` and `texture`, opening a new one when
    /// it is not.
    ///
    /// The open run's `end` is left stale and only written when the run is
    /// closed, here or in [`finish`](Self::finish), so extending a run reads
    /// memory but stores nothing beyond the queued data itself.
    fn open_run(&mut self, kind: RunKind, texture: Option<TextureId>) {
        if self
            .runs
            .last()
            .is_some_and(|run: &Run2d| run.kind == kind && run.texture == texture)
        {
            return;
        }

        self.finish();
        let start: u32 = self.reach(kind);
        self.runs.push(Run2d {
            kind,
            texture,
            start,
            end: start,
        });
    }

    /// Closes the open run, making every run's `end` exact. Must run before
    /// the runs are read; idempotent.
    pub fn finish(&mut self) {
        let Some(kind) = self.runs.last().map(|run: &Run2d| run.kind) else {
            return;
        };
        let end: u32 = self.reach(kind);
        if let Some(run) = self.runs.last_mut() {
            run.end = end;
        }
    }

    /// Rectangles and sprites queued so far.
    pub fn quad_count(&self) -> usize {
        self.quads.len()
    }

    /// The queued rectangles and sprites, in submission order.
    pub fn quads(&self) -> &[QuadInstance] {
        &self.quads
    }

    /// Whether nothing at all is queued.
    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }

    /// Empties every queue, keeping their capacity.
    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.quads.clear();
        self.runs.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(batch: &mut Batch2d, texture: Option<TextureId>) {
        match texture {
            Some(texture) => batch.push_sprite(Vec2::ZERO, Vec2::new(1.0, 1.0), texture, Color::WHITE),
            None => batch.push_rect(Vec2::ZERO, Vec2::new(1.0, 1.0), Color::WHITE),
        }
    }

    fn triangle(batch: &mut Batch2d) {
        batch.push_triangle(Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0), Color::WHITE);
    }

    fn spans(batch: &mut Batch2d) -> Vec<(RunKind, u32, u32)> {
        batch.finish();
        batch.runs.iter().map(|r: &Run2d| (r.kind, r.start, r.end)).collect()
    }

    #[test]
    fn consecutive_quads_share_one_run() {
        let mut batch: Batch2d = Batch2d::default();
        rect(&mut batch, None);
        rect(&mut batch, None);
        rect(&mut batch, None);
        assert_eq!(spans(&mut batch), vec![(RunKind::Quads, 0, 3)]);
    }

    #[test]
    fn interleaving_preserves_submission_order() {
        let mut batch: Batch2d = Batch2d::default();
        rect(&mut batch, None);
        triangle(&mut batch);
        triangle(&mut batch);
        rect(&mut batch, None);
        assert_eq!(
            spans(&mut batch),
            vec![
                (RunKind::Quads, 0, 1),
                (RunKind::Triangles, 0, 6),
                (RunKind::Quads, 1, 2)
            ]
        );
        assert_eq!(batch.indices, vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn texture_change_splits_quad_run() {
        let mut batch: Batch2d = Batch2d::default();
        rect(&mut batch, None);
        rect(&mut batch, Some(TextureId::new(0)));
        assert_eq!(spans(&mut batch), vec![(RunKind::Quads, 0, 1), (RunKind::Quads, 1, 2)]);
    }

    #[test]
    fn rect_collapses_its_uvs_onto_the_white_texel() {
        let mut batch: Batch2d = Batch2d::default();
        batch.push_rect(Vec2::new(10.0, 20.0), Vec2::new(3.0, 4.0), Color::WHITE);

        let quad: QuadInstance = batch.quads[0];
        assert_eq!(quad.position, [10.0, 20.0]);
        assert_eq!(quad.size, [3.0, 4.0]);
        assert_eq!((quad.uv_min, quad.uv_max), ([0.0, 0.0], [0.0, 0.0]));
    }

    #[test]
    fn sprite_spans_the_whole_texture() {
        let mut batch: Batch2d = Batch2d::default();
        batch.push_sprite(Vec2::ZERO, Vec2::ONE, TextureId::new(1), Color::WHITE);

        let quad: QuadInstance = batch.quads[0];
        assert_eq!((quad.uv_min, quad.uv_max), ([0.0, 0.0], [1.0, 1.0]));
    }

    #[test]
    fn color_rounds_to_unorm8() {
        let mut batch: Batch2d = Batch2d::default();
        batch.push_rect(Vec2::ZERO, Vec2::ONE, Color::RED.with_alpha(0.5));

        assert_eq!(batch.quads[0].color, [255, 0, 0, 128]);
    }

    #[test]
    fn quad_instance_is_36_bytes() {
        assert_eq!(std::mem::size_of::<QuadInstance>(), 36);
    }
}
