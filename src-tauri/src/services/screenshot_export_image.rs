//! Lossless export helpers. Work on selected rows, not a cloned full desktop bitmap.
use screenshots::image::{
    codecs::png::{CompressionType, FilterType, PngEncoder},
    imageops, ColorType, ImageEncoder, RgbaImage,
};

use crate::error::{AppError, AppResult};

pub(super) fn encode_png(image: &RgbaImage) -> AppResult<Vec<u8>> {
    let mut bytes = Vec::new();
    PngEncoder::new_with_quality(&mut bytes, CompressionType::Fast, FilterType::Sub)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            ColorType::Rgba8,
        )
        .map_err(|error| {
            AppError::new("SCREENSHOT_ENCODE_FAILED", "截图编码失败")
                .with_detail("reason", error.to_string())
        })?;
    Ok(bytes)
}

fn validate_crop(
    source_width: u32,
    source_height: u32,
    left: u32,
    top: u32,
    width: u32,
    height: u32,
) -> AppResult<()> {
    if width == 0
        || height == 0
        || left
            .checked_add(width)
            .is_none_or(|right| right > source_width)
        || top
            .checked_add(height)
            .is_none_or(|bottom| bottom > source_height)
    {
        return Err(AppError::new(
            "SCREENSHOT_CAPTURE_FAILED",
            "截图裁剪区域超出像素边界",
        ));
    }
    Ok(())
}

pub(super) fn crop_rgba(
    source: &RgbaImage,
    left: u32,
    top: u32,
    width: u32,
    height: u32,
) -> AppResult<RgbaImage> {
    validate_crop(source.width(), source.height(), left, top, width, height)?;
    let stride = source.width() as usize * 4;
    let row_bytes = width as usize * 4;
    let mut pixels = vec![0; row_bytes * height as usize];
    for (row, output) in pixels.chunks_exact_mut(row_bytes).enumerate() {
        let start = (top as usize + row) * stride + left as usize * 4;
        output.copy_from_slice(&source.as_raw()[start..start + row_bytes]);
    }
    RgbaImage::from_raw(width, height, pixels)
        .ok_or_else(|| AppError::new("SCREENSHOT_CAPTURE_FAILED", "构建裁剪图像失败"))
}

pub(super) fn crop_bgra(
    source_width: u32,
    source_height: u32,
    source: &[u8],
    left: u32,
    top: u32,
    width: u32,
    height: u32,
) -> AppResult<RgbaImage> {
    validate_crop(source_width, source_height, left, top, width, height)?;
    let stride = (source_width as usize)
        .checked_mul(4)
        .ok_or_else(|| AppError::new("SCREENSHOT_CAPTURE_FAILED", "截图位图行数据溢出"))?;
    let expected = stride
        .checked_mul(source_height as usize)
        .ok_or_else(|| AppError::new("SCREENSHOT_CAPTURE_FAILED", "截图位图缓冲区溢出"))?;
    if source.len() != expected {
        return Err(AppError::new(
            "SCREENSHOT_CAPTURE_FAILED",
            "截图位图缓冲区长度异常",
        ));
    }
    let row_bytes = width as usize * 4;
    let mut pixels = vec![0; row_bytes * height as usize];
    for (row, output) in pixels.chunks_exact_mut(row_bytes).enumerate() {
        let start = (top as usize + row) * stride + left as usize * 4;
        for (input, output) in source[start..start + row_bytes]
            .chunks_exact(4)
            .zip(output.chunks_exact_mut(4))
        {
            output.copy_from_slice(&[input[2], input[1], input[0], input[3]]);
        }
    }
    RgbaImage::from_raw(width, height, pixels)
        .ok_or_else(|| AppError::new("SCREENSHOT_CAPTURE_FAILED", "构建裁剪图像失败"))
}

/// Preserve imageops alpha blending for transparent tiles; opaque capture uses row copies.
pub(super) fn composite_tile(output: &mut RgbaImage, tile: &RgbaImage, x: u32, y: u32) {
    if !tile.as_raw().chunks_exact(4).all(|pixel| pixel[3] == 255) {
        imageops::overlay(output, tile, i64::from(x), i64::from(y));
        return;
    }
    let width = tile.width().min(output.width().saturating_sub(x));
    let height = tile.height().min(output.height().saturating_sub(y));
    if width == 0 || height == 0 {
        return;
    }
    let input_stride = tile.width() as usize * 4;
    let output_stride = output.width() as usize * 4;
    let row_bytes = width as usize * 4;
    for row in 0..height as usize {
        let input_start = row * input_stride;
        let output_start = (y as usize + row) * output_stride + x as usize * 4;
        output.as_mut()[output_start..output_start + row_bytes]
            .copy_from_slice(&tile.as_raw()[input_start..input_start + row_bytes]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use screenshots::image::Rgba;

    fn fixture(width: u32, height: u32) -> RgbaImage {
        RgbaImage::from_fn(width, height, |x, y| {
            Rgba([
                (x * 31 + y) as u8,
                (y * 19 + x) as u8,
                (x ^ y) as u8,
                (x * 47 + y * 13) as u8,
            ])
        })
    }

    #[test]
    fn crops_preserve_channels_alpha_and_edge_pixels() {
        let source = fixture(17, 11);
        let bgra: Vec<_> = source
            .as_raw()
            .chunks_exact(4)
            .flat_map(|pixel| [pixel[2], pixel[1], pixel[0], pixel[3]])
            .collect();
        for (x, y, width, height) in [(0, 0, 17, 11), (3, 4, 7, 5), (16, 10, 1, 1), (1, 0, 16, 1)] {
            let expected = imageops::crop_imm(&source, x, y, width, height).to_image();
            assert_eq!(crop_rgba(&source, x, y, width, height).unwrap(), expected);
            assert_eq!(
                crop_bgra(17, 11, &bgra, x, y, width, height).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn invalid_crop_and_truncated_buffers_are_errors_not_panics() {
        let source = fixture(3, 2);
        assert!(crop_rgba(&source, 2, 0, 2, 1).is_err());
        assert!(crop_rgba(&source, u32::MAX, 0, 2, 1).is_err());
        assert!(crop_rgba(&source, 0, 0, 0, 1).is_err());
        assert!(crop_bgra(3, 2, &[0; 23], 0, 0, 1, 1).is_err());
    }

    #[test]
    fn fast_png_is_lossless_and_preserves_dimensions() {
        let source = fixture(73, 39);
        let png = encode_png(&source).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(
            screenshots::image::load_from_memory(&png)
                .unwrap()
                .to_rgba8(),
            source
        );
    }

    #[test]
    fn opaque_and_transparent_composites_match_original_even_when_clipped() {
        for opaque in [true, false] {
            let mut tile = fixture(5, 4);
            if opaque {
                for pixel in tile.pixels_mut() {
                    pixel[3] = 255;
                }
            }
            for (x, y) in [(0, 0), (4, 3), (6, 5), (u32::MAX, 0)] {
                let mut expected = fixture(6, 5);
                let mut actual = expected.clone();
                imageops::overlay(&mut expected, &tile, i64::from(x), i64::from(y));
                composite_tile(&mut actual, &tile, x, y);
                assert_eq!(actual, expected);
            }
        }
    }
}
