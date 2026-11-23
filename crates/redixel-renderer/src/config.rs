//! Renderer settings and the per-frame camera state derived from them,
//! independent of any GPU API.

use redixel_math::Mat4;
use wgpu_types::{Backends, PresentMode};

use crate::shader::GlobalUniforms;

const CAMERA_FOV_Y_DEGREES: f32 = 60.0;
const CAMERA_NEAR: f32 = 0.1;
const CAMERA_FAR: f32 = 100.0;

/// All renderer settings resolved from `config.json` by `redixel-runtime`
/// and injected at construction time. The renderer never touches the config system.
///
/// - `present_spin`: whether the frame thread busy-waits, for at most a couple
///   of milliseconds, for the previous frame's present instead of sleeping.
///   Spinning saves the wake-up latency at the cost of a busy core while
///   waiting, so it is on by default on desktop and off on mobile. Only the
///   `hal` renderer presents on its own thread, so only it reads this.
#[derive(Debug, Clone)]
pub struct RendererConfig {
    pub backends:     Backends,
    pub present_mode: PresentMode,
    pub present_spin: bool,
}

/// [`RendererConfig::present_spin`] unless configured.
pub const DEFAULT_PRESENT_SPIN: bool = !cfg!(any(target_os = "android", target_os = "ios"));

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            backends:     Backends::all(),
            present_mode: PresentMode::AutoVsync,
            present_spin: DEFAULT_PRESENT_SPIN,
        }
    }
}

/// Builds the 2D (orthographic, pixel space) and 3D (perspective) uniform
/// blocks for a `w`×`h` surface at `time`, in that order.
///
/// The matrices are rebuilt from the surface size every frame, which is what
/// keeps geometry proportional across a resize without the resize path having
/// to re-upload anything. `resolution` carries those same dimensions to the
/// shader, where the projection is no help.
///
/// `view` is the identity in both blocks: the engine has no camera transform
/// yet, and 3D draw calls arrive already in view space.
///
/// `time` is seconds since startup, narrowed to `f32` — WGSL uniforms have no
/// `f64`. Its resolution decays to about a millisecond after a couple of hours
/// of uptime; an effect needing better than that wants the time reduced modulo
/// a multiple of `TAU` by whoever renders, not a wider type.
pub(crate) fn frame_uniforms(w: u32, h: u32, time: f32) -> (GlobalUniforms, GlobalUniforms) {
    let resolution: [f32; 2] = [w as f32, h as f32];

    let ortho: Mat4 = Mat4::orthographic(0.0, w as f32, h as f32, 0.0, -1.0, 1.0);
    let uniforms_2d: GlobalUniforms = GlobalUniforms {
        view: Mat4::IDENTITY.cols,
        projection: ortho.cols,
        resolution,
        time,
        _padding: 0.0,
    };

    let aspect: f32 = w as f32 / h as f32;
    let perspective: Mat4 = Mat4::perspective(CAMERA_FOV_Y_DEGREES.to_radians(), aspect, CAMERA_NEAR, CAMERA_FAR);
    let uniforms_3d: GlobalUniforms = GlobalUniforms {
        view: Mat4::IDENTITY.cols,
        projection: perspective.cols,
        resolution,
        time,
        _padding: 0.0,
    };

    (uniforms_2d, uniforms_3d)
}
