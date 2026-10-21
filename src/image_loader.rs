use std::io::{BufRead, Seek};
use std::path::{Path, PathBuf};

use eframe::egui;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};

const MAX_PIXELS: u64 = 32_000_000;
const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;

pub struct LoadedImage {
    pub path: PathBuf,
    pub dimensions: [u32; 2],
    pub pixels: egui::ColorImage,
    pub reduced: bool,
}

pub fn load(path: &Path, max_side: u32) -> Result<LoadedImage, String> {
    let file =
        std::fs::File::open(path).map_err(|_| "This file could not be opened.".to_owned())?;
    if file
        .metadata()
        .map_err(|_| "This file could not be read.")?
        .len()
        > MAX_FILE_BYTES
    {
        return Err("This file is too large. The current limit is 256 MiB.".to_owned());
    }
    let (dimensions, pixels, reduced) = decode(std::io::BufReader::new(file), max_side)?;
    Ok(LoadedImage {
        path: path.to_owned(),
        dimensions,
        pixels,
        reduced,
    })
}

fn decode(
    source: impl BufRead + Seek,
    max_side: u32,
) -> Result<([u32; 2], egui::ColorImage, bool), String> {
    let mut reader = ImageReader::new(source)
        .with_guessed_format()
        .map_err(|_| "This file could not be read.".to_owned())?;
    if !matches!(
        reader.format(),
        Some(ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP)
    ) {
        return Err("Unsupported image. Choose a JPEG, PNG, or WebP file.".to_owned());
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(32_768);
    limits.max_image_height = Some(32_768);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(decode_error)?;
    let (width, height) = decoder.dimensions();
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err("This image is too large. The current limit is 32 megapixels.".to_owned());
    }
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut decoded = DynamicImage::from_decoder(decoder).map_err(decode_error)?;
    decoded.apply_orientation(orientation);
    let dimensions = [decoded.width(), decoded.height()];
    let max_side = max_side.clamp(1, 8192);
    let reduced = dimensions.iter().any(|side| *side > max_side);
    if reduced {
        decoded = decoded.resize(max_side, max_side, image::imageops::FilterType::Triangle);
    }
    let rgba = decoded.into_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Ok((
        dimensions,
        egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()),
        reduced,
    ))
}

fn decode_error(error: image::ImageError) -> String {
    match error {
        image::ImageError::Limits(_) => {
            "This image exceeds the current decoding limits.".to_owned()
        }
        _ => "This image could not be decoded. The file may be damaged.".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn encoded(format: ImageFormat) -> Vec<u8> {
        let pixels = image::RgbaImage::from_pixel(6, 4, image::Rgba([180, 80, 30, 128]));
        let image = DynamicImage::ImageRgba8(pixels);
        let mut bytes = Cursor::new(Vec::new());
        let image = if format == ImageFormat::Jpeg {
            DynamicImage::ImageRgb8(image.to_rgb8())
        } else {
            image
        };
        image.write_to(&mut bytes, format).unwrap();
        bytes.into_inner()
    }

    #[test]
    fn decodes_supported_formats_from_contents() {
        for format in [ImageFormat::Jpeg, ImageFormat::Png, ImageFormat::WebP] {
            let (dimensions, pixels, reduced) = decode(Cursor::new(encoded(format)), 1024).unwrap();
            assert_eq!(dimensions, [6, 4]);
            assert_eq!(pixels.size, [6, 4]);
            assert!(!reduced);
        }
    }

    #[test]
    fn preserves_transparency() {
        for format in [ImageFormat::Png, ImageFormat::WebP] {
            let (_, pixels, _) = decode(Cursor::new(encoded(format)), 1024).unwrap();
            assert_eq!(pixels.pixels[0].a(), 128);
        }
    }

    #[test]
    fn reduces_preview_without_losing_original_dimensions() {
        let (dimensions, pixels, reduced) =
            decode(Cursor::new(encoded(ImageFormat::Png)), 3).unwrap();
        assert_eq!(dimensions, [6, 4]);
        assert_eq!(pixels.size, [3, 2]);
        assert!(reduced);
    }

    #[test]
    fn applies_jpeg_exif_orientation() {
        let jpeg = encoded(ImageFormat::Jpeg);
        let exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
        let mut rotated = jpeg[..2].to_vec();
        rotated.extend_from_slice(&[0xff, 0xe1]);
        rotated.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        rotated.extend_from_slice(exif);
        rotated.extend_from_slice(&jpeg[2..]);
        let (dimensions, pixels, _) = decode(Cursor::new(rotated), 1024).unwrap();
        assert_eq!(dimensions, [4, 6]);
        assert_eq!(pixels.size, [4, 6]);
    }

    #[test]
    fn rejects_non_images_and_truncated_images() {
        assert!(
            decode(Cursor::new(b"not an image"), 1024)
                .unwrap_err()
                .contains("Unsupported")
        );
        let mut png = encoded(ImageFormat::Png);
        png.truncate(40);
        assert!(decode(Cursor::new(png), 1024).is_err());
    }
}
