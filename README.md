# Lumen

A minimal photo viewer for Windows, built with Rust.

## Status

Early development. Open JPEG, PNG, and WebP images in a resizable, dark-themed window. Images fit within the window while preserving their aspect ratio.

## Usage

- Click **Open image** or press **Ctrl+O** to select a photo.
- Drop an image file onto the window to open it. When dropping multiple files, only the first is opened.
- Use **Previous** / **Next** or the **Left** / **Right** arrow keys to navigate the current folder. Files are sorted naturally by name (`photo1`, `photo2`, `photo10`), ignoring case.
- Navigation stops at the first and last file. Unreadable images are skipped, and the counter includes all matching files, including unreadable ones. The folder list refreshes when an image is opened explicitly; new files added afterward appear on the next explicit open.
- The status bar shows the file name and image dimensions. Cancelling a selection or failing to open a new file preserves the current image.

Images load in the background. Only one open operation runs at a time. The current limits are 256 MiB per file and 32 megapixels per image, with additional decoder resource limits. Images that exceed the display texture limit use a reduced preview, indicated in the status bar. Animated images and color profile management are not supported yet.

## Planned features

- Zoom, pan, and full screen.

## Development

Requires stable Rust with the MSVC toolchain and Windows C++ build tools.

```powershell
cargo run --locked
```

Create a release build with `cargo build --release`. The executable is written to `target/release/lumen.exe`.

See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution and validation guidelines.
