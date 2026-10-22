use std::io::{BufRead, Seek};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use eframe::egui;
use image::{AnimationDecoder, DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};

const MAX_PIXELS: u64 = 32_000_000;
const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ANIMATION_BYTES: usize = 128 * 1024 * 1024;
const MAX_ANIMATION_FRAMES: usize = 1000;

#[derive(Debug)]
pub struct AnimationFrame {
    pub pixels: Arc<egui::ColorImage>,
    pub delay: Duration,
}

#[derive(Debug)]
pub struct Animation {
    pub frames: Vec<AnimationFrame>,
    /// Total plays, including the initial play; None means infinite.
    pub plays: Option<u32>,
}

pub struct LoadedImage {
    pub path: PathBuf,
    pub dimensions: [u32; 2],
    pub pixels: Arc<egui::ColorImage>,
    pub reduced: bool,
    pub animation: Option<Animation>,
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
    let (dimensions, pixels, reduced, animation) = decode(std::io::BufReader::new(file), max_side)?;
    Ok(LoadedImage {
        path: path.to_owned(),
        dimensions,
        pixels,
        reduced,
        animation,
    })
}

type Decoded = ([u32; 2], Arc<egui::ColorImage>, bool, Option<Animation>);

fn decode(source: impl BufRead + Seek, max_side: u32) -> Result<Decoded, String> {
    let mut reader = ImageReader::new(source)
        .with_guessed_format()
        .map_err(|_| "This file could not be read.".to_owned())?;
    if !matches!(
        reader.format(),
        Some(ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP | ImageFormat::Gif)
    ) {
        return Err("Unsupported image. Choose a JPEG, PNG, WebP, or GIF file.".to_owned());
    }
    if reader.format() == Some(ImageFormat::Gif) {
        return decode_gif(
            reader.into_inner(),
            max_side,
            MAX_ANIMATION_BYTES,
            MAX_ANIMATION_FRAMES,
        );
    }
    reader.limits(decoding_limits());
    let mut decoder = reader.into_decoder().map_err(decode_error)?;
    let (width, height) = decoder.dimensions();
    check_dimensions(width, height)?;
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut decoded = DynamicImage::from_decoder(decoder).map_err(decode_error)?;
    decoded.apply_orientation(orientation);
    let dimensions = [decoded.width(), decoded.height()];
    let (pixels, reduced) = preview(decoded, max_side);
    Ok((dimensions, pixels, reduced, None))
}

fn decoding_limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(32_768);
    limits.max_image_height = Some(32_768);
    limits.max_alloc = Some(256 * 1024 * 1024);
    limits
}

fn check_dimensions(width: u32, height: u32) -> Result<(), String> {
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err("This image is too large. The current limit is 32 megapixels.".to_owned());
    }
    Ok(())
}

fn preview(mut decoded: DynamicImage, max_side: u32) -> (Arc<egui::ColorImage>, bool) {
    let dimensions = [decoded.width(), decoded.height()];
    let max_side = max_side.clamp(1, 8192);
    let reduced = dimensions.iter().any(|side| *side > max_side);
    if reduced {
        decoded = decoded.resize(max_side, max_side, image::imageops::FilterType::Triangle);
    }
    let rgba = decoded.into_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    (
        Arc::new(egui::ColorImage::from_rgba_unmultiplied(
            size,
            rgba.as_raw(),
        )),
        reduced,
    )
}

fn decode_gif(
    mut source: impl BufRead + Seek,
    max_side: u32,
    max_bytes: usize,
    max_frames: usize,
) -> Result<Decoded, String> {
    // Read the original repeat value: image's loop_count maps an absent GIF
    // loop extension to infinite, and finite values omit the initial play.
    let metadata = gif::DecodeOptions::new()
        .read_info(&mut source)
        .map_err(|_| "This GIF could not be decoded. The file may be damaged.".to_owned())?;
    let plays = match metadata.repeat() {
        gif::Repeat::Infinite => None,
        gif::Repeat::Finite(repeats) => Some(u32::from(repeats) + 1),
    };
    drop(metadata);
    source
        .rewind()
        .map_err(|_| "This file could not be read.")?;
    let mut decoder = image::codecs::gif::GifDecoder::new(source).map_err(decode_error)?;
    decoder
        .set_limits(decoding_limits())
        .map_err(decode_error)?;
    let (width, height) = decoder.dimensions();
    check_dimensions(width, height)?;
    let mut frames = Vec::new();
    let mut bytes = 0usize;
    let mut reduced = false;
    for frame in decoder.into_frames() {
        let frame = frame.map_err(decode_error)?;
        if frames.len() >= max_frames {
            return Err("This GIF exceeds the animation frame limit (1000 frames).".to_owned());
        }
        let (numerator, denominator) = frame.delay().numer_denom_ms();
        // Zero-delay GIFs otherwise cause a continuous repaint loop.
        let delay = if numerator == 0 {
            Duration::from_millis(100)
        } else {
            Duration::from_secs_f64(f64::from(numerator) / f64::from(denominator) / 1000.0)
                .max(Duration::from_millis(10))
        };
        let (pixels, frame_reduced) =
            preview(DynamicImage::ImageRgba8(frame.into_buffer()), max_side);
        let frame_bytes = pixels.pixels.len() * std::mem::size_of::<egui::Color32>();
        bytes = bytes
            .checked_add(frame_bytes)
            .ok_or("This GIF is too large.")?;
        if bytes > max_bytes {
            return Err("This GIF exceeds the animation memory limit (128 MiB).".to_owned());
        }
        reduced |= frame_reduced;
        frames.push(AnimationFrame { pixels, delay });
    }
    let pixels = frames
        .first()
        .ok_or("This GIF contains no frames.")?
        .pixels
        .clone();
    let animation = (frames.len() > 1).then_some(Animation { frames, plays });
    Ok(([width, height], pixels, reduced, animation))
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

    fn gif_bytes(repeat: Option<gif::Repeat>, disposal: gif::DisposalMethod) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = gif::Encoder::new(
                &mut bytes,
                3,
                1,
                &[0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255],
            )
            .unwrap();
            if let Some(repeat) = repeat {
                encoder.set_repeat(repeat).unwrap();
            }
            for (left, pixels, delay, dispose) in [
                (0, vec![1, 1, 1], 2, gif::DisposalMethod::Keep),
                (1, vec![2], 7, disposal),
                (2, vec![3], 0, gif::DisposalMethod::Keep),
            ] {
                let frame = gif::Frame {
                    width: pixels.len() as u16,
                    height: 1,
                    left,
                    buffer: pixels.into(),
                    delay,
                    dispose,
                    transparent: Some(0),
                    ..Default::default()
                };
                encoder.write_frame(&frame).unwrap();
            }
        }
        bytes
    }

    #[test]
    fn gif_frames_preserve_timing_dimensions_and_original_repeat_semantics() {
        for (repeat, plays) in [
            (None, Some(1)),
            (Some(gif::Repeat::Finite(2)), Some(3)),
            (Some(gif::Repeat::Infinite), None),
        ] {
            let bytes = gif_bytes(repeat, gif::DisposalMethod::Keep);
            let (dimensions, pixels, reduced, animation) = decode(Cursor::new(bytes), 2).unwrap();
            assert_eq!(dimensions, [3, 1]);
            assert_eq!(pixels.size, [2, 1]);
            assert!(reduced);
            let animation = animation.unwrap();
            assert_eq!(animation.plays, plays);
            assert!(Arc::ptr_eq(&pixels, &animation.frames[0].pixels));
            assert_eq!(
                animation
                    .frames
                    .iter()
                    .map(|frame| frame.delay.as_millis())
                    .collect::<Vec<_>>(),
                [20, 70, 100]
            );
        }
    }

    #[test]
    fn gif_composes_partial_frames_and_disposal_with_transparency() {
        for disposal in [
            gif::DisposalMethod::Background,
            gif::DisposalMethod::Previous,
        ] {
            let (_, _, _, animation) =
                decode(Cursor::new(gif_bytes(None, disposal)), 1024).unwrap();
            let frames = animation.unwrap().frames;
            assert_eq!(
                frames[1].pixels.pixels,
                [egui::Color32::RED, egui::Color32::GREEN, egui::Color32::RED]
            );
            let middle = if disposal == gif::DisposalMethod::Background {
                egui::Color32::TRANSPARENT
            } else {
                egui::Color32::RED
            };
            assert_eq!(
                frames[2].pixels.pixels,
                [egui::Color32::RED, middle, egui::Color32::BLUE]
            );
        }
    }

    #[test]
    fn gif_rejects_cache_limits_damaged_frames_and_oversized_canvas() {
        let bytes = gif_bytes(None, gif::DisposalMethod::Keep);
        assert!(
            decode_gif(Cursor::new(&bytes), 1024, 35, 1000)
                .unwrap_err()
                .contains("memory limit")
        );
        assert!(
            decode_gif(Cursor::new(&bytes), 1024, 128 * 1024 * 1024, 2)
                .unwrap_err()
                .contains("frame limit")
        );
        assert!(decode(Cursor::new(&bytes[..bytes.len() - 5]), 1024).is_err());
        let mut large = bytes;
        large[6..8].copy_from_slice(&8192u16.to_le_bytes());
        large[8..10].copy_from_slice(&8192u16.to_le_bytes());
        assert!(
            decode(Cursor::new(large), 1024)
                .unwrap_err()
                .contains("32 megapixels")
        );
    }

    #[test]
    fn single_frame_gif_is_static() {
        let (_, _, _, animation) = decode(Cursor::new(encoded(ImageFormat::Gif)), 1024).unwrap();
        assert!(animation.is_none());
    }

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
            let (dimensions, pixels, reduced, animation) =
                decode(Cursor::new(encoded(format)), 1024).unwrap();
            assert_eq!(dimensions, [6, 4]);
            assert_eq!(pixels.size, [6, 4]);
            assert!(!reduced);
            assert!(animation.is_none());
        }
    }

    #[test]
    fn preserves_transparency() {
        for format in [ImageFormat::Png, ImageFormat::WebP] {
            let (_, pixels, _, _) = decode(Cursor::new(encoded(format)), 1024).unwrap();
            assert_eq!(pixels.pixels[0].a(), 128);
        }
    }

    #[test]
    fn reduces_preview_without_losing_original_dimensions() {
        let (dimensions, pixels, reduced, _) =
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
        let (dimensions, pixels, _, _) = decode(Cursor::new(rotated), 1024).unwrap();
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
