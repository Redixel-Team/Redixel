//! Triangle geometry built on the CPU: the vertex helpers, and the indexed
//! [`Geometry`] queue that coalesces consecutive triangles sharing a texture
//! into one draw run. Owns no GPU resources.

use redixel_core::TextureId;
use redixel_math::{Color, Vec2, Vec3};

use crate::shader::Vertex;

pub(crate) const TRIANGLE_INDICES: [u32; 3] = [0, 1, 2];

/// Untextured geometry samples the registry's 1×1 white pixel, so every corner
/// collapses onto the same texel and the sample multiplies the colour by one.
const SOLID_UV: [f32; 2] = [0.0, 0.0];

/// Builds the three corners of a 2D triangle in the order they were given.
pub(crate) fn triangle_vertices(p1: Vec2, p2: Vec2, p3: Vec2, color: Color) -> [Vertex; 3] {
    let c: [f32; 4] = color.to_array();

    [p1, p2, p3].map(|p: Vec2| Vertex {
        position: [p.x, p.y, 0.0],
        color:    c,
        uv:       SOLID_UV,
    })
}

/// Builds the three corners of a 3D triangle in the order they were given,
/// pairing each with the matching entry of `uvs`.
///
/// The only vertex-building path whose depth isn't hardcoded to zero.
pub(crate) fn triangle_vertices_3d(points: [Vec3; 3], uvs: [Vec2; 3], color: Color) -> [Vertex; 3] {
    let c: [f32; 4] = color.to_array();

    std::array::from_fn(|i: usize| Vertex {
        position: points[i].to_array(),
        color:    c,
        uv:       [uvs[i].x, uvs[i].y],
    })
}

/// A contiguous slice of the index buffer sharing one texture, and so
/// submittable as a single draw call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DrawRun {
    pub(crate) texture: Option<TextureId>,
    pub(crate) start:   u32,
    pub(crate) end:     u32,
}

/// Geometry accumulated on the CPU during a frame.
///
/// Owns no GPU resources, so index rebasing and run coalescing can be exercised
/// without a device.
#[derive(Default)]
pub(crate) struct Geometry {
    pub(crate) vertices: Vec<Vertex>,
    pub(crate) indices:  Vec<u32>,
    pub(crate) runs:     Vec<DrawRun>,
}

impl Geometry {
    /// Appends `vertices` and the `indices` referencing them, rebasing each
    /// index onto the geometry already queued and recording its texture.
    ///
    /// Indices arrive relative to the primitive's own first vertex, so a
    /// primitive never has to know how much geometry precedes it; this is the
    /// only place that offset is computed. Consecutive primitives sharing a
    /// texture extend the open run rather than opening a new one.
    pub(crate) fn push(&mut self, vertices: &[Vertex], indices: &[u32], texture: Option<TextureId>) {
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

    pub(crate) fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.runs.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn positions(vertices: &[Vertex]) -> Vec<[f32; 3]> {
        vertices.iter().map(|v: &Vertex| v.position).collect()
    }

    fn uvs(vertices: &[Vertex]) -> Vec<[f32; 2]> {
        vertices.iter().map(|v: &Vertex| v.uv).collect()
    }

    fn triangle() -> [Vertex; 3] {
        triangle_vertices_3d([Vec3::ZERO, Vec3::X, Vec3::Y], [Vec2::ZERO; 3], Color::WHITE)
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
    fn geometry_has_no_ceiling() {
        const TRIANGLES: usize = 50_000;

        let mut geometry: Geometry = Geometry::default();
        for _ in 0..TRIANGLES {
            geometry.push(&triangle(), &TRIANGLE_INDICES, None);
        }

        assert_eq!(geometry.vertices.len(), TRIANGLES * 3);
        assert_eq!(geometry.indices.len(), TRIANGLES * 3);

        let highest: u32 = *geometry.indices.iter().max().unwrap();
        assert_eq!(highest, geometry.vertices.len() as u32 - 1);
    }

    #[test]
    fn clear_resets_every_queue() {
        let mut geometry: Geometry = Geometry::default();
        geometry.push(&triangle(), &TRIANGLE_INDICES, None);
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
    fn triangle_vertices_carry_collapsed_uvs() {
        assert!(
            uvs(&triangle_vertices(Vec2::ZERO, Vec2::X, Vec2::Y, Color::WHITE))
                .iter()
                .all(|uv: &[f32; 2]| *uv == [0.0, 0.0]),
            "untextured geometry samples a 1x1 white texture; zero keeps the intent obvious"
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
            geometry.push(&triangle(), &TRIANGLE_INDICES, None);
        }

        assert_eq!(geometry.runs.len(), 1, "a frame with no textures stays one draw call");
        assert_eq!(geometry.runs[0].start, 0);
        assert_eq!(geometry.runs[0].end, geometry.indices.len() as u32);
    }

    #[test]
    fn changing_texture_opens_a_new_run() {
        let first: TextureId = TextureId::new(0);
        let second: TextureId = TextureId::new(1);

        let mut geometry: Geometry = Geometry::default();
        geometry.push(&triangle(), &TRIANGLE_INDICES, Some(first));
        geometry.push(&triangle(), &TRIANGLE_INDICES, Some(second));
        geometry.push(&triangle(), &TRIANGLE_INDICES, Some(first));

        let textures: Vec<Option<TextureId>> = geometry.runs.iter().map(|r: &DrawRun| r.texture).collect();

        assert_eq!(
            textures,
            vec![Some(first), Some(second), Some(first)],
            "reordering runs to merge them would break draw order"
        );
    }

    #[test]
    fn runs_partition_the_index_buffer() {
        let texture: TextureId = TextureId::new(2);

        let mut geometry: Geometry = Geometry::default();
        geometry.push(&triangle(), &TRIANGLE_INDICES, None);
        geometry.push(&triangle(), &TRIANGLE_INDICES, Some(texture));
        geometry.push(&triangle(), &TRIANGLE_INDICES, None);

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
