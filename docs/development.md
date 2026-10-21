# Development

Run the commands below from the repository root.

Requires stable Rust with the MSVC toolchain and Windows C++ build tools, including
the Windows SDK resource compiler for the application icon and version information.

```powershell
cargo run --locked
```

Pass a photo path during development with `cargo run --locked -- "C:\Photos\example.jpg"`.

Create an optimized release build with `cargo build --release --locked`. The executable is written to `target/release/lumen.exe`. Release builds run without a console window. No installer or automatic file association registration is included.

See [CONTRIBUTING.md](../CONTRIBUTING.md) for contribution and validation guidelines.
Editable icon masters and their Windows exports are in [assets/](../assets/README.md).

### Loading benchmarks

Run `cargo bench --locked --bench loading` to measure the image loader and folder
scanner in an optimized build. The benchmark uses temporary synthetic 4K JPEG,
PNG, and WebP images, a 24-megapixel JPEG, and a folder containing 1,000 image
entries and 10,000 unrelated files. Temporary fixtures are removed after the run.

Each case reports the median and range of five samples after one warm-up. Fixture
creation and encoding are excluded from the measured operations. Results include
image decoding and conversion to display pixels, plus preview resizing when
needed. They do not measure application startup, GPU upload, or input-to-display
latency. The filesystem cache is warm; compare results on the same machine and
avoid treating synthetic-image timings as guarantees for real photos.

## Packaging

After building a release, create a portable ZIP with PowerShell 7:

```powershell
./scripts/package-windows.ps1
```

Packages are written to `target/packages/` and contain `lumen.exe`, a usage guide,
license and branding notices, third-party notices, and `SHA256SUMS`. The script verifies that the input is an x64 Windows GUI executable.

On GitHub, run **Actions → Windows Package → Run workflow** to test, build, and
package a selected branch. Download the ZIP from the run's Artifacts section and
extract it before running. Artifacts are retained for seven days. Packaging changes
also run this workflow on pull requests. These are unsigned development packages;
the workflow does not create GitHub Releases or register Windows file associations.

## License notices

Lumen uses `GPL-3.0-only`; third-party dependencies keep their original licenses.
After changing dependencies, regenerate and review their notices with Python 3:

```powershell
python scripts/update-third-party-notices.py
```

The script reads the locked Windows dependency graph and local crate sources. It
fetches missing upstream license texts at recorded revisions (or the matching
published version tag for egui if its revision is unavailable) and caches them
under `target/license-cache/`. The notice snapshot
includes build and development dependencies as well as runtime dependencies; it
is not a claim that every listed component is embedded in the executable.

Review the generated text and bundled font notices before distribution. The
package script rejects a notice snapshot generated for a different `Cargo.lock`.
The lockfile hash normalizes line endings to LF so Git checkout settings do not
invalidate the snapshot.
The package also includes the active Rust toolchain's standard library notices.
Build and package with the same toolchain.

## Distributing builds

A binary package and license notices alone do not satisfy GPL source-distribution
requirements. When distributing Lumen, provide the matching Corresponding Source
in a manner permitted by GPLv3 section 6, including the build scripts and required
dependency sources. Preserve third-party notices and identify modified versions.
A private repository link is not a source delivery mechanism for recipients who
cannot access it.

From a clean checkout, prepare the matching source archive with PowerShell 7:

```powershell
./scripts/package-source.ps1
```

The archive is written to `target/packages/lumen-<version>-source.zip`. It contains
the project files, legal notices, build instructions, source checksums, the Git
revision and tool versions, and all locked dependency sources produced by
`cargo vendor`. `.cargo/config.toml` selects these bundled sources. Git metadata,
ignored files, and local agent guidance are excluded. `-AllowDirty` is available
for local verification; such archives explicitly record that local changes were
included and must not be presented as a clean release revision.

Extract the archive and, from its root, verify a build without dependency downloads:

```powershell
cargo build --release --frozen --target x86_64-pc-windows-msvc
```

`--frozen` combines `--locked` and `--offline`. Rust and the Windows C++ build tools
must already be installed. Use the recorded Rust version when reproducing a build;
the archive does not promise byte-for-byte reproducibility.

The Windows Package workflow uploads the Windows and source packages as separate
artifacts with the same retention period. Build and package both from the same
clean revision and toolchain. When distributing, make both available together
with equivalent access; do not rely on an expired Actions artifact or GitHub's
automatic source ZIP, which does not contain the vendored dependencies. A release
workflow and permanent downloads are separate from this development packaging step.
