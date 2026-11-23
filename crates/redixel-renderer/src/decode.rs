//! Turning image bytes into RGBA8 texels, plus the two images the engine makes
//! itself: the white pixel untextured geometry samples, and the checkerboard a
//! broken texture handle draws. Independent of any GPU API.

use std::io::Cursor;

use image::{DynamicImage, ImageReader, Limits, RgbaImage};
use redixel_core::RedixelError;

pub(crate) const RGBA_BYTES_PER_TEXEL: u32 = 4;
pub(crate) const CHECKERBOARD_SIZE: u32 = 16;
pub(crate) const CHECKERBOARD_CELL: u32 = 8;

/// Bound whenever geometry carries no texture. Opaque white so the shader's
/// unconditional `sample * color` leaves the vertex colour untouched, which is
/// what lets textured and untextured geometry share one pipeline and one batch.
pub(crate) const WHITE_PIXEL_RGBA: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xFF];

/// Builds the magenta-and-black checkerboard, deliberately the loudest thing
/// the engine can draw: a texture that failed to load is a bug in the game's
/// assets, and rendering nothing would hide it until someone noticed.
pub(crate) fn checkerboard_rgba(size: u32, cell: u32) -> Vec<u8> {
    const MAGENTA: [u8; 4] = [0xFF, 0x00, 0xFF, 0xFF];
    const BLACK: [u8; 4] = [0x00, 0x00, 0x00, 0xFF];

    let mut texels: Vec<u8> = Vec::with_capacity((size * size * RGBA_BYTES_PER_TEXEL) as usize);

    for y in 0..size {
        for x in 0..size {
            let dark: bool = ((x / cell) + (y / cell)).is_multiple_of(2);
            texels.extend_from_slice(if dark { &BLACK } else { &MAGENTA });
        }
    }

    texels
}

/// Decodes `bytes` to RGBA8, refusing an image wider or taller than `max`,
/// the device's `max_texture_dimension_2d`.
///
/// The bound goes to the decoder rather than being checked afterwards:
/// `create_texture` answers an oversized descriptor with a validation error
/// that WGPU's default handler turns into a panic, and rejecting from the
/// header also avoids allocating pixels that were never going to fit.
pub(crate) fn decode_rgba(bytes: &[u8], max: u32) -> Result<RgbaImage, RedixelError> {
    let mut limits: Limits = Limits::default();
    limits.max_image_width = Some(max);
    limits.max_image_height = Some(max);

    let mut reader: ImageReader<Cursor<&[u8]>> = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e: std::io::Error| RedixelError::TextureDecode(e.to_string()))?;
    reader.limits(limits);

    let decoded: DynamicImage = reader
        .decode()
        .map_err(|e: image::ImageError| RedixelError::TextureDecode(e.to_string()))?;

    let rgba: RgbaImage = decoded.to_rgba8();
    let (width, height): (u32, u32) = rgba.dimensions();

    if width == 0 || height == 0 {
        return Err(RedixelError::TextureDecode(String::from("image has zero width or height")));
    }

    Ok(rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texel(texels: &[u8], size: u32, x: u32, y: u32) -> [u8; 4] {
        let start: usize = ((y * size + x) * RGBA_BYTES_PER_TEXEL) as usize;
        texels[start..start + 4].try_into().expect("texel is four bytes")
    }

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();

        DynamicImage::ImageRgba8(RgbaImage::new(width, height))
            .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
            .expect("encoding a blank image cannot fail");

        out
    }

    #[test]
    fn checkerboard_fills_every_texel() {
        let texels: Vec<u8> = checkerboard_rgba(CHECKERBOARD_SIZE, CHECKERBOARD_CELL);

        assert_eq!(
            texels.len(),
            (CHECKERBOARD_SIZE * CHECKERBOARD_SIZE * RGBA_BYTES_PER_TEXEL) as usize
        );
    }

    #[test]
    fn checkerboard_alternates_cells() {
        let size: u32 = CHECKERBOARD_SIZE;
        let cell: u32 = CHECKERBOARD_CELL;
        let texels: Vec<u8> = checkerboard_rgba(size, cell);

        assert_eq!(texel(&texels, size, 0, 0), [0x00, 0x00, 0x00, 0xFF]);
        assert_eq!(texel(&texels, size, cell, 0), [0xFF, 0x00, 0xFF, 0xFF]);
        assert_eq!(texel(&texels, size, 0, cell), [0xFF, 0x00, 0xFF, 0xFF]);
        assert_eq!(texel(&texels, size, cell, cell), [0x00, 0x00, 0x00, 0xFF]);
    }

    #[test]
    fn decode_accepts_an_image_inside_the_limit() {
        let rgba: RgbaImage = decode_rgba(&png(32, 16), 64).expect("32x16 fits under a 64 limit");

        assert_eq!(rgba.dimensions(), (32, 16));
    }

    #[test]
    fn decode_rejects_an_image_past_the_limit() {
        assert!(
            decode_rgba(&png(64, 8), 32).is_err(),
            "reaching create_texture oversized is a validation error, and the default handler panics"
        );
    }

    #[test]
    fn decode_measures_both_axes_against_the_limit() {
        assert!(decode_rgba(&png(8, 64), 32).is_err());
    }

    #[test]
    fn decode_rejects_undecodable_input() {
        assert!(decode_rgba(b"", 4096).is_err());
        assert!(decode_rgba(b"not a png at all", 4096).is_err());
    }

    #[test]
    fn checkerboard_is_fully_opaque() {
        let texels: Vec<u8> = checkerboard_rgba(CHECKERBOARD_SIZE, CHECKERBOARD_CELL);

        assert!(
            texels.as_chunks::<4>().0.iter().all(|t: &[u8; 4]| t[3] == 0xFF),
            "a translucent marker would blend away the thing it is meant to flag"
        );
    }
}
