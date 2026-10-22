use std::ffi::c_void;

#[repr(C)]
struct NativeImage {
    pixels: *const u8,
    owner: *mut c_void,
    stride: usize,
    width: u32,
    height: u32,
}

unsafe extern "C" {
    fn lumen_heic_decode(
        data: *const u8,
        length: usize,
        max_pixels: u64,
        max_bytes: u64,
        max_side: u32,
        output: *mut NativeImage,
    ) -> i32;
    fn lumen_heic_free(owner: *mut c_void);
}

impl Drop for NativeImage {
    fn drop(&mut self) {
        if !self.owner.is_null() {
            // Only the native library releases its own image allocation.
            unsafe { lumen_heic_free(self.owner) };
        }
    }
}

pub(super) fn decode(data: &[u8]) -> Result<image::DynamicImage, String> {
    let mut native = NativeImage {
        pixels: std::ptr::null(),
        owner: std::ptr::null_mut(),
        stride: 0,
        width: 0,
        height: 0,
    };
    // The bridge borrows data only during this call and returns an owning image.
    // It catches C++ exceptions, validates decoded plane dimensions and stride,
    // and retains the plane until NativeImage is dropped.
    let status = unsafe {
        lumen_heic_decode(
            data.as_ptr(),
            data.len(),
            super::MAX_PIXELS,
            super::MAX_DECODE_BYTES,
            32_768,
            &mut native,
        )
    };
    match status {
        0 => {}
        2 => return Err("This image exceeds the current decoding limits.".to_owned()),
        3 => return Err("Unsupported HEIF compression. Only HEIC (HEVC) is supported.".to_owned()),
        _ => {
            return Err(
                "This HEIC image could not be decoded. The file may be damaged.".to_owned(),
            );
        }
    }
    let row_bytes = native.width as usize * 4;
    let mut rgba = image::RgbaImage::new(native.width, native.height);
    for (row, destination) in rgba.as_mut().chunks_exact_mut(row_bytes).enumerate() {
        // Each row is bounded by the actual decoded plane, not container metadata.
        let source = unsafe {
            std::slice::from_raw_parts(native.pixels.add(row * native.stride), row_bytes)
        };
        destination.copy_from_slice(source);
    }
    Ok(image::DynamicImage::ImageRgba8(rgba))
}
