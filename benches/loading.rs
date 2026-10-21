use std::hint::black_box;
use std::path::Path;
use std::time::{Duration, Instant};

#[allow(dead_code)]
#[path = "../src/image_loader.rs"]
mod image_loader;
#[allow(dead_code, unused_imports)]
#[path = "../src/navigation.rs"]
mod navigation;

fn measure(label: &str, mut operation: impl FnMut()) {
    operation();
    let mut samples = Vec::with_capacity(5);
    for _ in 0..5 {
        let start = Instant::now();
        operation();
        samples.push(start.elapsed());
    }
    samples.sort_unstable();
    println!(
        "{label}: median {:.2} ms, range {:.2}–{:.2} ms",
        millis(samples[2]),
        millis(samples[0]),
        millis(samples[4]),
    );
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn fixture(width: u32, height: u32) -> image::RgbImage {
    image::RgbImage::from_fn(width, height, |x, y| {
        let grain = x.wrapping_mul(73) ^ y.wrapping_mul(151);
        image::Rgb([
            ((x / 16 + grain % 29) % 256) as u8,
            ((y / 16 + grain % 31) % 256) as u8,
            ((x / 32 + y / 32 + grain % 37) % 256) as u8,
        ])
    })
}

fn load(path: &Path, max_side: u32) {
    let loaded = image_loader::load(path, max_side).expect("Fixture must decode");
    black_box((
        loaded.path,
        loaded.dimensions,
        loaded.pixels,
        loaded.reduced,
    ));
}

fn main() {
    println!("Synthetic fixtures; one warm-up and five measured samples per case.");
    println!("File creation/encoding excluded; warm filesystem cache; no GPU upload or UI timing.");
    let directory = tempfile::tempdir().expect("Temporary directory must be available");
    let pixels = fixture(3840, 2160);
    for extension in ["jpg", "png", "webp"] {
        let path = directory.path().join(format!("4k.{extension}"));
        pixels.save(&path).expect("Fixture must encode");
        measure(&format!("4K {extension}, full preview"), || {
            load(&path, 8192)
        });
    }
    drop(pixels);
    let large = directory.path().join("24mp.jpg");
    fixture(6000, 4000)
        .save(&large)
        .expect("Large fixture must encode");
    measure("24 MP JPEG, full preview", || load(&large, 8192));
    measure("24 MP JPEG, 2048 px preview", || load(&large, 2048));

    let folder = tempfile::tempdir().expect("Temporary directory must be available");
    let current = folder.path().join("photo500.jpg");
    for index in 0..1000 {
        std::fs::write(folder.path().join(format!("photo{index}.jpg")), [])
            .expect("Folder fixture must be created");
    }
    for index in 0..10000 {
        std::fs::write(folder.path().join(format!("document{index}.txt")), [])
            .expect("Folder fixture must be created");
    }
    measure("Folder scan, 1,000 photos and 10,000 other files", || {
        let folder = navigation::Folder::scan(&current).expect("Fixture folder must scan");
        assert_eq!(folder.position(), (501, 1000));
        black_box(folder);
    });
}
