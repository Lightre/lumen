# Lumen project guidance

Apply these instructions to all files in this repository.

## Product and code

- Lumen is a minimal Rust photo viewer for Windows. Keep features focused on browsing and viewing photos.
- JPEG, PNG, and WebP are the supported formats. Preserve current size limits, EXIF orientation, natural folder ordering, and background loading.
- Follow the existing module boundaries: `app`, `chrome`, `image_loader`, `navigation`, and `view`.
- Match the existing dark, restrained interface. Verify layout at the 480 × 360 logical-pixel minimum window size when changing UI.
- Write source code, UI text, comments, and documentation in English.
- Keep user-facing content focused on Lumen; do not add conversation history or tool-process notes to product docs.
- Prefer existing dependencies. Explain the concrete need before adding a dependency.

## Changes and validation

- Keep changes focused and preserve behavior outside the requested scope.
- For Rust changes, run `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, and `cargo test --locked`.
- For packaging or Windows resource changes, also build and verify with `scripts/package-windows.ps1` on the Windows MSVC toolchain.
- Graphical changes need manual Windows verification at minimum and typical window sizes.
- Commit `Cargo.lock` changes with their dependency changes. Use `--locked` for reproducible Cargo validation and builds.
- Keep user-facing README content concise; put detailed usage and contributor guidance in their existing documents.
- Preserve copyright, GPL-3.0-only, third-party, bundled-font, and branding notices. Regenerate third-party notices after changing dependencies and review them.
