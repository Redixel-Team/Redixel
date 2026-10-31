const MIN_FOV_Y: f32 = 1e-4;
const MAX_FOV_Y: f32 = std::f32::consts::PI - 1e-4;
const MIN_NEAR: f32 = 1e-6;
const MIN_DEPTH_RATIO: f32 = 1.0001;

/// A 4×4 column-major matrix used for coordinate transforms.
///
/// Stored as `cols[col][row]` — identical to WGSL's `mat4x4<f32>` memory layout,
/// so `cols` can be uploaded to a uniform buffer as-is.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mat4 {
    pub cols: [[f32; 4]; 4],
}

impl Mat4 {
    pub const IDENTITY: Self = Self {
        cols: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };

    /// Constructs from four column vectors (each a `[f32; 4]`).
    pub fn from_cols(c0: [f32; 4], c1: [f32; 4], c2: [f32; 4], c3: [f32; 4]) -> Self {
        Self { cols: [c0, c1, c2, c3] }
    }

    /// Returns the matrix as a flat `[f32; 16]` array suitable for GPU upload.
    pub fn to_cols_array(self) -> [f32; 16] {
        let c: [[f32; 4]; 4] = self.cols;
        [
            c[0][0], c[0][1], c[0][2], c[0][3], c[1][0], c[1][1], c[1][2], c[1][3], c[2][0], c[2][1], c[2][2], c[2][3],
            c[3][0], c[3][1], c[3][2], c[3][3],
        ]
    }

    /// Constructs an orthographic projection matrix for 2D rendering.
    ///
    /// Maps the rectangle `[left, right] × [bottom, top]` to NDC `[-1, 1]²`.
    /// Depth range `[near, far]` maps to `[0, 1]` (WGPU convention).
    ///
    /// # Usage
    /// For a window of size `(w, h)` with the origin at the top-left corner:
    /// ```ignore
    /// Mat4::orthographic(0.0, w, h, 0.0, -1.0, 1.0)
    /// ```
    pub fn orthographic(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Self {
        let rml: f32 = right - left;
        let tmb: f32 = top - bottom;
        let fmn: f32 = far - near;

        Self::from_cols(
            [2.0 / rml, 0.0, 0.0, 0.0],
            [0.0, 2.0 / tmb, 0.0, 0.0],
            [0.0, 0.0, 1.0 / fmn, 0.0],
            [-(right + left) / rml, -(top + bottom) / tmb, -near / fmn, 1.0],
        )
    }

    /// Constructs a left-handed perspective projection matrix.
    ///
    /// The camera sits at the origin looking down `+Z`, with `+X` right and
    /// `+Y` up. `fov_y_radians` is the full vertical field of view; `near`/
    /// `far` are positive distances along that forward axis. Depth range
    /// `[near, far]` maps to `[0, 1]` (WGPU convention), matching
    /// [`orthographic`](Self::orthographic). Unlike `orthographic`, this does
    /// not flip Y — there is no top-left-origin screen convention to match in
    /// view space.
    ///
    /// # Usage
    /// ```ignore
    /// Mat4::perspective(60.0_f32.to_radians(), w / h, 0.1, 100.0)
    /// ```
    ///
    /// # Degenerate input
    /// The domain is `0 < fov_y_radians < π`, `aspect > 0` and `0 < near < far`.
    /// Anything outside it is clamped: each degeneracy divides by zero, and the
    /// resulting `NaN` reaches a uniform buffer, where it blanks the frame
    /// instead of failing anywhere a caller could see.
    pub fn perspective(fov_y_radians: f32, aspect: f32, near: f32, far: f32) -> Self {
        let fov: f32 = fov_y_radians.clamp(MIN_FOV_Y, MAX_FOV_Y);
        let aspect: f32 = if aspect.is_finite() && aspect > 0.0 {
            aspect
        } else {
            1.0
        };

        let near: f32 = near.max(MIN_NEAR);
        let far: f32 = far.max(near * MIN_DEPTH_RATIO);
        let (sin, cos): (f32, f32) = (0.5 * fov).sin_cos();
        let f: f32 = cos / sin;
        let range: f32 = far / (far - near);

        Self::from_cols(
            [f / aspect, 0.0, 0.0, 0.0],
            [0.0, f, 0.0, 0.0],
            [0.0, 0.0, range, 1.0],
            [0.0, 0.0, -range * near, 0.0],
        )
    }

    /// Returns a translation matrix that moves points by `(tx, ty, tz)`.
    pub fn translate(tx: f32, ty: f32, tz: f32) -> Self {
        let mut m: Mat4 = Self::IDENTITY;
        m.cols[3] = [tx, ty, tz, 1.0];
        m
    }

    /// Returns a scale matrix with an independent factor per axis.
    pub fn scale(sx: f32, sy: f32, sz: f32) -> Self {
        let mut m: Mat4 = Self::IDENTITY;
        m.cols[0][0] = sx;
        m.cols[1][1] = sy;
        m.cols[2][2] = sz;
        m
    }

    /// Returns a 2D rotation matrix (rotation around the Z axis).
    pub fn rotate_z(radians: f32) -> Self {
        let (sin, cos): (f32, f32) = radians.sin_cos();
        let mut m: Mat4 = Self::IDENTITY;
        m.cols[0][0] = cos;
        m.cols[0][1] = sin;
        m.cols[1][0] = -sin;
        m.cols[1][1] = cos;
        m
    }

    /// Matrix × matrix multiplication (self × rhs).
    pub fn mul_mat4(self, rhs: Self) -> Self {
        let a: [[f32; 4]; 4] = self.cols;
        let b: [[f32; 4]; 4] = rhs.cols;
        let mut out: [[f32; 4]; 4] = [[0.0f32; 4]; 4];

        for col in 0..4 {
            for row in 0..4 {
                out[col][row] =
                    a[0][row] * b[col][0] + a[1][row] * b[col][1] + a[2][row] * b[col][2] + a[3][row] * b[col][3];
            }
        }

        Self { cols: out }
    }
}

impl Default for Mat4 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl std::ops::Mul for Mat4 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        self.mul_mat4(rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f32 = 1e-5;

    fn approx_eq(a: [f32; 16], b: [f32; 16]) -> bool {
        a.iter().zip(b.iter()).all(|(x, y): (&f32, &f32)| (*x - *y).abs() < EPS)
    }

    fn ortho_depth(proj: Mat4, z: f32) -> f32 {
        proj.cols[2][2] * z + proj.cols[3][2]
    }

    fn perspective_depth(proj: Mat4, z: f32) -> f32 {
        proj.cols[2][2] + proj.cols[3][2] / z
    }

    #[test]
    fn identity_mul() {
        let m: Mat4 = Mat4::IDENTITY * Mat4::IDENTITY;
        assert!(approx_eq(m.to_cols_array(), Mat4::IDENTITY.to_cols_array()));
    }

    #[test]
    fn orthographic_maps_corners() {
        let proj: Mat4 = Mat4::orthographic(0.0, 800.0, 600.0, 0.0, -1.0, 1.0);
        let c: [[f32; 4]; 4] = proj.cols;

        assert!((c[0][0] - 2.0 / 800.0).abs() < EPS);
        assert!((c[1][1] - 2.0 / (-600.0_f32)).abs() < EPS);
    }

    #[test]
    fn scale_and_translate_compose() {
        let t: Mat4 = Mat4::translate(5.0, 3.0, 0.0);
        let s: Mat4 = Mat4::scale(2.0, 2.0, 1.0);
        let m: Mat4 = t * s;

        // Scale first, then translate: (1, 1) → (2, 2) → (7, 5).
        let c: [[f32; 4]; 4] = m.cols;
        let (x, y): (f32, f32) = (c[0][0] + c[1][0] + c[3][0], c[0][1] + c[1][1] + c[3][1]);
        assert!((x - 7.0).abs() < EPS && (y - 5.0).abs() < EPS, "got ({x}, {y})");
    }

    #[test]
    fn perspective_scales_by_cotangent_of_half_fov_over_aspect() {
        let proj: Mat4 = Mat4::perspective(std::f32::consts::FRAC_PI_2, 2.0, 1.0, 100.0);
        let c: [[f32; 4]; 4] = proj.cols;

        assert!((c[0][0] - 0.5).abs() < EPS);
        assert!((c[1][1] - 1.0).abs() < EPS);
    }

    #[test]
    fn perspective_maps_near_and_far_planes_into_wgpu_depth_range() {
        let (near, far): (f32, f32) = (1.0, 100.0);
        let proj: Mat4 = Mat4::perspective(std::f32::consts::FRAC_PI_2, 1.0, near, far);

        assert!(perspective_depth(proj, near).abs() < EPS);
        assert!((perspective_depth(proj, far) - 1.0).abs() < EPS);
    }

    #[test]
    fn flat_2d_geometry_sits_in_front_of_realistic_3d_depths() {
        let ortho: Mat4 = Mat4::orthographic(0.0, 800.0, 600.0, 0.0, -1.0, 1.0);
        let perspective: Mat4 = Mat4::perspective(60.0_f32.to_radians(), 800.0 / 600.0, 0.1, 100.0);

        let flat: f32 = ortho_depth(ortho, 0.0);
        assert!((flat - 0.5).abs() < EPS);

        for distance in [0.5_f32, 1.0, 3.0, 10.0, 100.0] {
            let depth: f32 = perspective_depth(perspective, distance);
            assert!(
                depth > flat,
                "3D geometry at z={distance} has depth {depth}, which a depth-writing 2D layer at {flat} would occlude"
            );
        }
    }

    #[test]
    fn perspective_clamps_degenerate_input_instead_of_emitting_nan() {
        let degenerate: [(f32, f32, f32, f32); 5] = [
            (0.0, 1.0, 0.1, 100.0),
            (std::f32::consts::PI, 1.0, 0.1, 100.0),
            (std::f32::consts::FRAC_PI_2, 0.0, 0.1, 100.0),
            (std::f32::consts::FRAC_PI_2, 1.0, 0.0, 0.0),
            (std::f32::consts::FRAC_PI_2, 1.0, 100.0, 1.0),
        ];

        for (fov, aspect, near, far) in degenerate {
            let proj: Mat4 = Mat4::perspective(fov, aspect, near, far);
            assert!(
                proj.to_cols_array().iter().all(|v: &f32| v.is_finite()),
                "perspective({fov}, {aspect}, {near}, {far}) produced a non-finite matrix"
            );
        }
    }
}
