# Lumen

A simple, modern photo viewer for Windows, built with Rust.

The project is being set up. A graphical interface and image loading are not implemented yet.

## Initial scope

- Windows desktop application.
- JPEG, PNG, and WebP support.
- Previous and next image navigation within the same folder.
- Zoom, pan, and full screen.
- A minimal interface in near-black and dark gray tones.

Editing and drawing are out of scope. Rotation, theme settings, additional formats, and other operating systems will be considered for later releases.

## Development

Requires stable Rust with the MSVC toolchain and Windows C++ build tools.

```powershell
cargo run
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

`Cargo.toml` defines package metadata and dependencies. `Cargo.lock` records exact dependency versions. The entry point is `src/main.rs`.

## Next steps

1. Create the first window with a dark theme and an empty state.
2. Open and display a single image.
3. Add folder navigation, zoom, pan, and full screen.
4. Handle errors and large images, then measure performance.

The initial candidates are `egui/eframe` for the interface and `image` for decoding. Dependencies will be added when implementing the first window. Image decoding should keep the interface responsive; memory use and preloading will be bounded and guided by measurements.

See [CONTRIBUTING.md](CONTRIBUTING.md) for the development workflow.
