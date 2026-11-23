/// Decodes one sRGB-encoded channel into linear space.
///
/// The piecewise sRGB electro-optical transfer function (IEC 61966-2-1).
#[inline]
fn srgb_channel_to_linear(channel: f32) -> f32 {
    if channel <= 0.040_45 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

/// Encodes one linear channel into sRGB space, the inverse of
/// [`srgb_channel_to_linear`].
#[inline]
fn linear_channel_to_srgb(channel: f32) -> f32 {
    if channel <= 0.003_130_8 {
        channel * 12.92
    } else {
        1.055 * channel.powf(1.0 / 2.4) - 0.055
    }
}

/// An RGBA colour with `f32` components, stored in **linear** space.
///
/// Used for clear colours, tints, and draw calls.
///
/// The renderer presents to an sRGB surface format, so the GPU performs the
/// linear-to-sRGB encoding when it writes the framebuffer. Shaders must pass
/// these values through untouched — correcting gamma in a shader would encode
/// twice and wash the image out.
///
/// 8-bit and hex values are sRGB-encoded by universal convention (that is what
/// colour pickers and CSS emit), so [`Color::from_rgba8`], [`Color::from_hex`]
/// and [`Color::srgb`] decode them on the way in. [`Color::rgb`] and
/// [`Color::rgba`] take components that are already linear.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const WHITE: Self = Self::rgba(1.0, 1.0, 1.0, 1.0);
    pub const BLACK: Self = Self::rgba(0.0, 0.0, 0.0, 1.0);
    pub const RED: Self = Self::rgba(1.0, 0.0, 0.0, 1.0);
    pub const GREEN: Self = Self::rgba(0.0, 1.0, 0.0, 1.0);
    pub const BLUE: Self = Self::rgba(0.0, 0.0, 1.0, 1.0);
    pub const YELLOW: Self = Self::rgba(1.0, 1.0, 0.0, 1.0);
    pub const CYAN: Self = Self::rgba(0.0, 1.0, 1.0, 1.0);
    pub const MAGENTA: Self = Self::rgba(1.0, 0.0, 1.0, 1.0);
    pub const TRANSPARENT: Self = Self::rgba(0.0, 0.0, 0.0, 0.0);

    /// Creates a colour from linear components in the range `[0.0, 1.0]`.
    #[inline]
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Creates an opaque colour from linear components in the range `[0.0, 1.0]`.
    #[inline]
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self::rgba(r, g, b, 1.0)
    }

    /// Creates a colour from sRGB components in the range `[0.0, 1.0]`,
    /// decoding the colour channels into linear space.
    ///
    /// The float counterpart to [`Color::from_rgba8`]: `Color::srgba(0.5, 0.5,
    /// 0.5, 1.0)` and `Color::from_rgba8(128, 128, 128, 255)` describe the same
    /// colour, whereas [`Color::rgba`] takes components that are already
    /// linear. Alpha is never gamma-encoded.
    #[inline]
    pub fn srgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self::rgba(
            srgb_channel_to_linear(r),
            srgb_channel_to_linear(g),
            srgb_channel_to_linear(b),
            a,
        )
    }

    /// Creates an opaque colour from sRGB components in the range `[0.0, 1.0]`,
    /// decoding them into linear space.
    #[inline]
    pub fn srgb(r: f32, g: f32, b: f32) -> Self {
        Self::srgba(r, g, b, 1.0)
    }

    /// Creates a colour from 8-bit sRGB components (0–255).
    ///
    /// The colour channels are decoded into linear space. Alpha is never
    /// gamma-encoded, so it is only normalised.
    #[inline]
    pub fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::rgba(
            srgb_channel_to_linear(r as f32 / 255.0),
            srgb_channel_to_linear(g as f32 / 255.0),
            srgb_channel_to_linear(b as f32 / 255.0),
            a as f32 / 255.0,
        )
    }

    /// Creates a colour from an sRGB hex code (`0xRRGGBBAA`), decoding the
    /// colour channels into linear space.
    #[inline]
    pub fn from_hex(hex: u32) -> Self {
        Self::from_rgba8(
            ((hex >> 24) & 0xFF) as u8,
            ((hex >> 16) & 0xFF) as u8,
            ((hex >> 8) & 0xFF) as u8,
            (hex & 0xFF) as u8,
        )
    }

    /// Returns the colour as 8-bit sRGB components, the inverse of
    /// [`Color::from_rgba8`]. Components outside `[0.0, 1.0]` are clamped.
    #[inline]
    pub fn to_rgba8(self) -> [u8; 4] {
        let encode = |channel: f32| -> u8 { (linear_channel_to_srgb(channel).clamp(0.0, 1.0) * 255.0).round() as u8 };

        [
            encode(self.r),
            encode(self.g),
            encode(self.b),
            (self.a.clamp(0.0, 1.0) * 255.0).round() as u8,
        ]
    }

    /// Returns `[r, g, b, a]` in linear space — the layout the shape pipeline
    /// uploads per vertex.
    #[inline]
    pub fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// Blends linearly toward `other` by factor `t ∈ [0, 1]`.
    ///
    /// Interpolation happens in linear space, which is how light actually
    /// mixes. Blending the corresponding sRGB values instead would give a
    /// different, darker midpoint.
    #[inline]
    pub fn lerp(self, other: Self, t: f32) -> Self {
        let lerp = |a: f32, b: f32| -> f32 { a + (b - a) * t };
        Self::rgba(
            lerp(self.r, other.r),
            lerp(self.g, other.g),
            lerp(self.b, other.b),
            lerp(self.a, other.a),
        )
    }

    /// Returns the colour with a different alpha.
    #[inline]
    pub fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::WHITE
    }
}

/// Converts to the `wgpu-types` colour (re-exported as `wgpu::Color`) for use
/// in render pass clear values.
///
/// Gated behind the `wgpu` feature so this crate does not force a graphics-API
/// dependency on consumers that only want the maths; the feature pulls in only
/// `wgpu-types`, not `wgpu` itself.
#[cfg(feature = "wgpu")]
impl From<Color> for wgpu_types::Color {
    fn from(c: Color) -> Self {
        wgpu_types::Color {
            r: c.r as f64,
            g: c.g as f64,
            b: c.b as f64,
            a: c.a as f64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_hex_decodes_srgb_to_linear() {
        let c: Color = Color::from_hex(0xFF8000FF);

        assert!((c.r - 1.0).abs() < 1e-3);
        assert!((c.g - 0.216).abs() < 1e-3, "expected linear 0.216, got {}", c.g);
        assert!((c.b - 0.0).abs() < 1e-3);
        assert!((c.a - 1.0).abs() < 1e-3);
    }

    #[test]
    fn srgb_extremes_are_identical_in_both_spaces() {
        let black: Color = Color::from_rgba8(0, 0, 0, 255);
        let white: Color = Color::from_rgba8(255, 255, 255, 255);

        assert!((black.r - 0.0).abs() < 1e-6);
        assert!((white.r - 1.0).abs() < 1e-6);
    }

    #[test]
    fn named_constants_match_their_srgb_spelling() {
        assert_eq!(Color::RED, Color::from_rgba8(255, 0, 0, 255));
        assert_eq!(Color::BLACK, Color::from_rgba8(0, 0, 0, 255));
        assert_eq!(Color::WHITE, Color::from_rgba8(255, 255, 255, 255));
    }

    #[test]
    fn rgba8_round_trips_through_linear() {
        for value in [0u8, 1, 17, 70, 128, 200, 254, 255] {
            let round_tripped: [u8; 4] = Color::from_rgba8(value, value, value, value).to_rgba8();
            assert_eq!(
                round_tripped,
                [value, value, value, value],
                "round trip lost precision at {value}"
            );
        }
    }

    #[test]
    fn alpha_is_not_gamma_encoded() {
        let c: Color = Color::from_rgba8(0, 0, 0, 128);
        assert!((c.a - 128.0 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn srgb_float_and_8bit_constructors_agree() {
        let float: Color = Color::srgb(128.0 / 255.0, 70.0 / 255.0, 200.0 / 255.0);
        let bytes: Color = Color::from_rgba8(128, 70, 200, 255);

        assert!((float.r - bytes.r).abs() < 1e-6);
        assert!((float.g - bytes.g).abs() < 1e-6);
        assert!((float.b - bytes.b).abs() < 1e-6);
    }

    #[test]
    fn srgba_leaves_alpha_untouched() {
        let c: Color = Color::srgba(0.5, 0.5, 0.5, 0.25);

        assert!((c.a - 0.25).abs() < 1e-6);
        assert!(c.r < 0.25, "colour channel should have been decoded, got {}", c.r);
    }

    #[test]
    fn lerp_halfway() {
        let c: Color = Color::BLACK.lerp(Color::WHITE, 0.5);
        assert!((c.r - 0.5).abs() < 1e-6);
    }
}
