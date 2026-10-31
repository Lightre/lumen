//! Triangle previews with one floating-point row instead of a full intermediate image.
use eframe::egui;
use image::{ImageBuffer, Pixel};

fn weights(input: u32, output: u32, position: u32, values: &mut Vec<f32>) -> usize {
    let ratio = input as f32 / output as f32;
    let support = ratio.max(1.0);
    let center = (position as f32 + 0.5) * ratio;
    let start = ((center - support).floor() as i64).clamp(0, i64::from(input) - 1) as u32;
    let end =
        ((center + support).ceil() as i64).clamp(i64::from(start) + 1, i64::from(input)) as u32;
    let center = center - 0.5;
    values.clear();
    let mut sum = 0.0;
    for source in start..end {
        let distance = ((source as f32 - center) / support).abs();
        let weight = if distance < 1.0 { 1.0 - distance } else { 0.0 };
        values.push(weight);
        sum += weight;
    }
    for value in values {
        *value /= sum;
    }
    start as usize
}

pub(super) fn resize<P: Pixel<Subpixel = u8>>(
    image: &ImageBuffer<P, Vec<u8>>,
    max_side: u32,
) -> egui::ColorImage {
    let (width, height) = image.dimensions();
    let scale = f64::from(max_side) / f64::from(width.max(height));
    let output_width = (f64::from(width) * scale).round().max(1.0) as u32;
    let output_height = (f64::from(height) * scale).round().max(1.0) as u32;
    let channels = usize::from(P::CHANNEL_COUNT);
    debug_assert!(channels == 3 || channels == 4);
    let mut horizontal = Vec::with_capacity(output_width as usize);
    for x in 0..output_width {
        let mut values = Vec::new();
        let start = weights(width, output_width, x, &mut values);
        horizontal.push((start, values));
    }
    let mut row = vec![[0.0f32; 4]; width as usize];
    let mut vertical = Vec::new();
    let source = image.as_raw();
    let mut pixels = Vec::with_capacity(output_width as usize * output_height as usize);
    for y in 0..output_height {
        let start = weights(height, output_height, y, &mut vertical);
        for (x, pixel) in row.iter_mut().enumerate() {
            *pixel = [0.0; 4];
            for (offset, weight) in vertical.iter().enumerate() {
                let index = ((start + offset) * width as usize + x) * channels;
                for channel in 0..channels {
                    pixel[channel] += f32::from(source[index + channel]) * weight;
                }
            }
        }
        for (start, values) in &horizontal {
            let mut pixel = [0.0f32; 4];
            for (offset, weight) in values.iter().enumerate() {
                for channel in 0..channels {
                    pixel[channel] += row[start + offset][channel] * weight;
                }
            }
            // Match image's Triangle rounding before egui's alpha conversion.
            let bytes = pixel.map(|value| value.clamp(0.0, 255.0).round() as u8);
            pixels.push(if channels == 3 {
                egui::Color32::from_rgb(bytes[0], bytes[1], bytes[2])
            } else {
                egui::Color32::from_rgba_unmultiplied(bytes[0], bytes[1], bytes[2], bytes[3])
            });
        }
    }
    egui::ColorImage::new([output_width as usize, output_height as usize], pixels)
}
