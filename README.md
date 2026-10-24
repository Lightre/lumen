<p align="center">
  <img src="assets/readme/banner.svg" alt="Lumen — a minimal photo viewer for Windows" width="880">
</p>

<p align="center">
  <a href="https://github.com/Lightre/lumen/actions/workflows/ci.yml"><img src="https://github.com/Lightre/lumen/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI status"></a>
  <img src="assets/readme/windows.svg" alt="Windows x64" height="24">
  <a href="LICENSE"><img src="assets/readme/license.svg" alt="GPLv3 only" height="24"></a>
</p>

Lumen is a small, dark-themed photo viewer built with Rust. Open a photo, move
through its folder, and focus on the image.

**Early development · Windows only · JPEG, PNG, WebP, GIF, BMP, TIFF, and HEIC**

<p align="center">
  <img src="assets/readme/screenshot.png" alt="Lumen displaying an alpine lake at dawn, with its dark toolbar and image controls" width="880">
</p>

## Features

- Open photos with a file picker, drag and drop, or Windows **Open with**.
- Browse the current folder in natural filename order.
- Rotate, fit, zoom, and pan, with a pixel-accurate 100% view and full screen.
- Load images in the background and respect JPEG orientation metadata.

## Try it

Download the Windows x64 ZIP from the **[latest release](https://github.com/Lightre/lumen/releases/latest)**.
Extract the ZIP before running `lumen.exe`.

To run from source, install stable Rust with the MSVC toolchain and the Windows
C++ build tools, including the Windows SDK, then run:

```powershell
cargo run --locked
```

## Controls

**Ctrl+O** opens a photo. **Left / Right** moves through the folder.
Scroll to zoom and drag to pan. **Home** fits the image; **F11** toggles full screen.
**Space** pauses or resumes animated GIFs.

See the [usage guide](docs/usage.md) for all controls and current limits.
Animated PNG and WebP remain static; color profile management is not supported yet.
TIFF displays the first image only.
HEIC/HEIF displays the main HEVC photo without installing codecs; HDR display and Live Photo video are not supported.

## Project

[Development and packaging](docs/development.md) ·
[Contributing](CONTRIBUTING.md) ·
[Visual assets](assets/README.md)

## License

Copyright (C) 2026 Lightre. Lumen's original code, documentation, and artwork are
licensed under **[GNU GPL version 3 only](LICENSE)** (`GPL-3.0-only`), without warranty.
Third-party components retain their own licenses; see [third-party notices](THIRD_PARTY_NOTICES.txt).
The [branding policy](BRANDING.md) explains use of the Lumen name and logo.

<p align="center"><sub>A little light, one pixel at a time. ✦</sub></p>
