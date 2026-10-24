use std::io::{BufRead, Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use eframe::egui;
use image::{AnimationDecoder, DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};

#[cfg(windows)]
#[path = "image_loader/heic.rs"]
mod heic;

const MAX_PIXELS: u64 = 32_000_000;
const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_DECODE_BYTES: u64 = 256 * 1024 * 1024;
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
    if reader.format().is_none() {
        let mut source = reader.into_inner();
        source
            .rewind()
            .map_err(|_| "This file could not be read.")?;
        let mut signature = [0; 8];
        if source.read_exact(&mut signature).is_err() || &signature[4..8] != b"ftyp" {
            return Err(
                "Unsupported image. Choose a JPEG, PNG, WebP, GIF, BMP, TIFF, or HEIC file."
                    .to_owned(),
            );
        }
        source
            .rewind()
            .map_err(|_| "This file could not be read.")?;
        let mut bytes = Vec::new();
        source
            .take(MAX_FILE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "This file could not be read.")?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err("This file is too large. The current limit is 256 MiB.".to_owned());
        }
        #[cfg(windows)]
        if bytes.get(4..8) == Some(b"ftyp") {
            let decoded = heic::decode(&bytes)?;
            let dimensions = [decoded.width(), decoded.height()];
            let (pixels, reduced) = preview(decoded, max_side);
            return Ok((dimensions, pixels, reduced, None));
        }
        return Err(
            "Unsupported image. Choose a JPEG, PNG, WebP, GIF, BMP, TIFF, or HEIC file.".to_owned(),
        );
    }
    if !matches!(
        reader.format(),
        Some(
            ImageFormat::Jpeg
                | ImageFormat::Png
                | ImageFormat::WebP
                | ImageFormat::Gif
                | ImageFormat::Bmp
                | ImageFormat::Tiff
        )
    ) {
        return Err(
            "Unsupported image. Choose a JPEG, PNG, WebP, GIF, BMP, TIFF, or HEIC file.".to_owned(),
        );
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
    let is_tiff = reader.format() == Some(ImageFormat::Tiff);
    let mut decoder = reader.into_decoder().map_err(decode_error)?;
    let (width, height) = decoder.dimensions();
    check_dimensions(width, height)?;
    if is_tiff && decoder.total_bytes() > MAX_DECODE_BYTES {
        return Err("This image exceeds the current decoding limits.".to_owned());
    }
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
    limits.max_alloc = Some(MAX_DECODE_BYTES);
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
    let size = [decoded.width() as usize, decoded.height() as usize];
    let pixels = match decoded {
        DynamicImage::ImageRgb8(rgb) => egui::ColorImage::from_rgb(size, rgb.as_raw()),
        decoded => {
            let rgba = decoded.into_rgba8();
            egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw())
        }
    };
    (Arc::new(pixels), reduced)
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

    #[cfg(windows)]
    #[test]
    fn decodes_heic_content_and_reduces_preview() {
        let bytes = include_bytes!("../tests/data/heic/rainbow.heic");
        let (dimensions, pixels, reduced, animation) = decode(Cursor::new(bytes), 1024).unwrap();
        assert_eq!(dimensions, [451, 461]);
        assert_eq!(pixels.size, [451, 461]);
        assert!(!reduced);
        assert!(animation.is_none());
        let (dimensions, pixels, reduced, _) = decode(Cursor::new(bytes), 100).unwrap();
        assert_eq!(dimensions, [451, 461]);
        assert_eq!(pixels.size[1], 100);
        assert!(reduced);
        // The decoder selects by content, even if the file extension is different.
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("photo.png");
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(load(&path, 1024).unwrap().dimensions, [451, 461]);
    }

    #[cfg(windows)]
    #[test]
    fn preserves_heic_alpha_and_applies_transforms_once() {
        let (_, pixels, _, _) = decode(
            Cursor::new(include_bytes!("../tests/data/heic/alpha.heic")),
            1024,
        )
        .unwrap();
        assert!(pixels.pixels.iter().any(|pixel| pixel.a() == 0));
        assert!(pixels.pixels.iter().any(|pixel| pixel.a() == 255));
        for bytes in [
            include_bytes!("../tests/data/heic/orientation-2.heic").as_slice(),
            include_bytes!("../tests/data/heic/orientation-6.heic").as_slice(),
        ] {
            let (dimensions, pixels, reduced, _) = decode(Cursor::new(bytes), 2048).unwrap();
            assert_eq!(dimensions, [1200, 800]);
            assert_eq!(pixels.size, [1200, 800]);
            assert!(!reduced);
        }
    }

    #[cfg(windows)]
    #[test]
    fn rejects_truncated_and_oversized_heic() {
        let original = include_bytes!("../tests/data/heic/rainbow.heic");
        assert!(decode(Cursor::new(&original[..64]), 1024).is_err());
        let mut bytes = original.to_vec();
        let ispe = bytes
            .windows(4)
            .position(|window| window == b"ispe")
            .unwrap();
        bytes[ispe + 8..ispe + 12].copy_from_slice(&40_000_u32.to_be_bytes());
        bytes[ispe + 12..ispe + 16].copy_from_slice(&40_000_u32.to_be_bytes());
        assert!(
            decode(Cursor::new(bytes), 1024)
                .unwrap_err()
                .contains("limits")
        );
    }

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

    fn tiff_bytes(
        compression: tiff::encoder::Compression,
        orientation: u16,
        multiple: bool,
    ) -> Vec<u8> {
        use tiff::encoder::{TiffEncoder, colortype};
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut encoder = TiffEncoder::new(&mut bytes)
                .unwrap()
                .with_compression(compression);
            let mut first = encoder.new_image::<colortype::RGB8>(2, 1).unwrap();
            first
                .encoder()
                .write_tag(tiff::tags::Tag::Orientation, orientation)
                .unwrap();
            first.write_data(&[255, 0, 0, 0, 0, 255]).unwrap();
            if multiple {
                encoder
                    .write_image::<colortype::Gray8>(1, 3, &[0, 128, 255])
                    .unwrap();
            }
        }
        bytes.into_inner()
    }

    #[test]
    fn tiff_decodes_common_compression_and_only_the_first_image() {
        use tiff::encoder::Compression;
        for compression in [
            Compression::Uncompressed,
            Compression::Lzw,
            Compression::Deflate(Default::default()),
            Compression::Packbits,
        ] {
            let (dimensions, pixels, reduced, animation) =
                decode(Cursor::new(tiff_bytes(compression, 1, true)), 1024).unwrap();
            assert_eq!(dimensions, [2, 1]);
            assert_eq!(pixels.pixels, [egui::Color32::RED, egui::Color32::BLUE]);
            assert!(!reduced);
            assert!(animation.is_none());
        }
    }

    #[test]
    fn tiff_applies_orientation_and_converts_16_bit_grayscale() {
        use tiff::encoder::{Compression, TiffEncoder, colortype};
        let (dimensions, pixels, _, _) = decode(
            Cursor::new(tiff_bytes(Compression::Uncompressed, 6, false)),
            1024,
        )
        .unwrap();
        assert_eq!(dimensions, [1, 2]);
        assert_eq!(pixels.pixels, [egui::Color32::RED, egui::Color32::BLUE]);
        let mut bytes = Cursor::new(Vec::new());
        TiffEncoder::new(&mut bytes)
            .unwrap()
            .write_image::<colortype::Gray16>(3, 1, &[0, 32768, 65535])
            .unwrap();
        let (dimensions, pixels, _, _) = decode(Cursor::new(bytes.into_inner()), 1024).unwrap();
        assert_eq!(dimensions, [3, 1]);
        assert_eq!(
            pixels.pixels,
            [
                egui::Color32::BLACK,
                egui::Color32::from_gray(128),
                egui::Color32::WHITE
            ]
        );
    }

    #[test]
    fn tiff_rejects_truncated_pixels_oversized_canvas_and_large_decode_buffers() {
        use tiff::encoder::{Compression, TiffEncoder, colortype};
        let mut bytes = tiff_bytes(Compression::Uncompressed, 1, false);
        assert!(decode(Cursor::new(&bytes[..7]), 1024).is_err());
        // Move the first strip outside the file without damaging its directory.
        let directory = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let tags = u16::from_le_bytes(bytes[directory..directory + 2].try_into().unwrap()) as usize;
        for tag in 0..tags {
            let offset = directory + 2 + tag * 12;
            if u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap()) == 273 {
                bytes[offset + 8..offset + 12].copy_from_slice(&u32::MAX.to_le_bytes());
            }
        }
        assert!(decode(Cursor::new(bytes), 1024).is_err());
        for (width, height, message) in [
            (8192, 8192, "32 megapixels"),
            (4096, 4097, "decoding limits"),
        ] {
            let mut bytes = Cursor::new(Vec::new());
            {
                let mut encoder = TiffEncoder::new(&mut bytes).unwrap();
                let mut frame = encoder.new_image::<colortype::RGBA32Float>(1, 1).unwrap();
                frame
                    .encoder()
                    .write_tag(tiff::tags::Tag::ImageWidth, width as u32)
                    .unwrap();
                frame
                    .encoder()
                    .write_tag(tiff::tags::Tag::ImageLength, height as u32)
                    .unwrap();
                frame.write_data(&[1.0, 0.0, 0.0, 1.0]).unwrap();
            }
            assert!(
                decode(Cursor::new(bytes.into_inner()), 1024)
                    .unwrap_err()
                    .contains(message)
            );
        }
    }

    fn bmp(
        width: i32,
        height: i32,
        bits: u16,
        compression: u32,
        palette: &[[u8; 4]],
        pixels: &[u8],
    ) -> Vec<u8> {
        let offset = 54 + palette.len() as u32 * 4;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"BM");
        bytes.extend_from_slice(&(offset + pixels.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(&offset.to_le_bytes());
        bytes.extend_from_slice(&40u32.to_le_bytes());
        bytes.extend_from_slice(&width.to_le_bytes());
        bytes.extend_from_slice(&height.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&bits.to_le_bytes());
        bytes.extend_from_slice(&compression.to_le_bytes());
        bytes.extend_from_slice(&(pixels.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&[0; 8]);
        bytes.extend_from_slice(&(palette.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        for color in palette {
            bytes.extend_from_slice(color);
        }
        bytes.extend_from_slice(pixels);
        bytes
    }

    #[test]
    fn bmp_decodes_padded_rgb_rows_in_both_directions() {
        let bottom_up = [255, 0, 0, 255, 255, 255, 0, 0, 0, 0, 255, 0, 255, 0, 0, 0];
        let top_down = [&bottom_up[8..], &bottom_up[..8]].concat();
        for (height, data) in [(2, bottom_up.as_slice()), (-2, top_down.as_slice())] {
            let (dimensions, pixels, reduced, animation) =
                decode(Cursor::new(bmp(2, height, 24, 0, &[], data)), 1024).unwrap();
            assert_eq!(dimensions, [2, 2]);
            assert_eq!(
                pixels.pixels,
                [
                    egui::Color32::RED,
                    egui::Color32::GREEN,
                    egui::Color32::BLUE,
                    egui::Color32::WHITE
                ]
            );
            assert!(!reduced);
            assert!(animation.is_none());
        }
    }

    #[test]
    fn bmp_decodes_indexed_and_rle8_pixels() {
        let palette = [[0, 0, 0, 0], [0, 0, 255, 0], [0, 255, 0, 0], [255, 0, 0, 0]];
        for (compression, data) in [
            (0, [1, 2, 3, 0].as_slice()),
            (1, [1, 1, 1, 2, 1, 3, 0, 1].as_slice()),
        ] {
            let (dimensions, pixels, _, _) =
                decode(Cursor::new(bmp(3, 1, 8, compression, &palette, data)), 1024).unwrap();
            assert_eq!(dimensions, [3, 1]);
            assert_eq!(
                pixels.pixels,
                [
                    egui::Color32::RED,
                    egui::Color32::GREEN,
                    egui::Color32::BLUE
                ]
            );
        }
    }

    #[test]
    fn bmp_rejects_invalid_headers_truncation_and_oversized_canvas() {
        let bytes = bmp(2, 2, 24, 0, &[], &[0; 16]);
        assert!(decode(Cursor::new(&bytes[..30]), 1024).is_err());
        assert!(decode(Cursor::new(&bytes[..bytes.len() - 4]), 1024).is_err());
        assert!(decode(Cursor::new(bmp(-2, 2, 24, 0, &[], &[0; 16])), 1024).is_err());
        assert!(
            decode(Cursor::new(bmp(8192, 8192, 24, 0, &[], &[])), 1024)
                .unwrap_err()
                .contains("32 megapixels")
        );
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
        for format in [
            ImageFormat::Jpeg,
            ImageFormat::Png,
            ImageFormat::WebP,
            ImageFormat::Bmp,
            ImageFormat::Tiff,
        ] {
            let (dimensions, pixels, reduced, animation) =
                decode(Cursor::new(encoded(format)), 1024).unwrap();
            assert_eq!(dimensions, [6, 4]);
            assert_eq!(pixels.size, [6, 4]);
            assert!(!reduced);
            assert!(animation.is_none());
        }
    }

    #[test]
    fn direct_rgb_preview_matches_rgba_conversion_exactly() {
        let rgb = image::RgbImage::from_fn(17, 17, |x, y| {
            let value = (x + y * 17) as u8;
            image::Rgb([value, value.wrapping_mul(73), 255 - value])
        });
        let rgba = image::RgbaImage::from_fn(17, 17, |x, y| {
            let value = (x + y * 17) as u8;
            image::Rgba([value, value.wrapping_mul(73), 255 - value, value])
        });
        for original in [DynamicImage::ImageRgb8(rgb), DynamicImage::ImageRgba8(rgba)] {
            for max_side in [8192, 7] {
                let expected = if max_side < 17 {
                    original.resize(max_side, max_side, image::imageops::FilterType::Triangle)
                } else {
                    original.clone()
                }
                .into_rgba8();
                let size = [expected.width() as usize, expected.height() as usize];
                let expected = egui::ColorImage::from_rgba_unmultiplied(size, expected.as_raw());
                let (actual, reduced) = preview(original.clone(), max_side);
                assert_eq!(actual.size, expected.size);
                assert_eq!(actual.pixels, expected.pixels);
                assert_eq!(reduced, max_side < 17);
            }
        }
    }

    #[test]
    fn preserves_transparency() {
        for format in [
            ImageFormat::Png,
            ImageFormat::WebP,
            ImageFormat::Bmp,
            ImageFormat::Tiff,
        ] {
            let (_, pixels, _, _) = decode(Cursor::new(encoded(format)), 1024).unwrap();
            assert_eq!(pixels.pixels[0].a(), 128);
        }
    }

    #[test]
    fn reduces_preview_without_losing_original_dimensions() {
        for format in [ImageFormat::Png, ImageFormat::Bmp, ImageFormat::Tiff] {
            let (dimensions, pixels, reduced, _) = decode(Cursor::new(encoded(format)), 3).unwrap();
            assert_eq!(dimensions, [6, 4]);
            assert_eq!(pixels.size, [3, 2]);
            assert!(reduced);
        }
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
