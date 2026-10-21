# Lumen

A minimal photo viewer for Windows, built with Rust.

## Status

Early development. The application currently provides a resizable, dark-themed window and an empty state. Image loading is not implemented yet.

## Planned features

- JPEG, PNG, and WebP support.
- Previous and next image navigation within a folder.
- Zoom, pan, and full screen.

## Development

Requires stable Rust with the MSVC toolchain and Windows C++ build tools.

```powershell
cargo run
```

Create a release build with `cargo build --release`. The executable is written to `target/release/lumen.exe`.

See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution and validation guidelines.
