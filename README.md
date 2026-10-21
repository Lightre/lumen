# Lumen

A minimal photo viewer for Windows, built with Rust.

## Status

Early development. Open JPEG, PNG, and WebP images in a resizable, dark-themed window. Images fit within the window while preserving their aspect ratio.

## Usage

- Click **Open image** or press **Ctrl+O** to select a photo.
- Drop an image file onto the window to open it. When dropping multiple files, only the first is opened.
- The status bar shows the file name and image dimensions. Cancelling a selection or failing to open a new file preserves the current image.

Images load in the background. Only one open operation runs at a time. The current limits are 256 MiB per file and 32 megapixels per image, with additional decoder resource limits. Images that exceed the display texture limit use a reduced preview, indicated in the status bar. Animated images and color profile management are not supported yet.

## Planned features

- Previous and next image navigation within a folder.
- Zoom, pan, and full screen.

## Development

Requires stable Rust with the MSVC toolchain and Windows C++ build tools.

```powershell
cargo run --locked
```

Create a release build with `cargo build --release`. The executable is written to `target/release/lumen.exe`.

See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution and validation guidelines.
