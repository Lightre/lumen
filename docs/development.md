# Development

Run the commands below from the repository root.

Requires stable Rust with the MSVC toolchain and Windows C++ build tools, including
the Windows SDK resource compiler for the application icon and version information.
HEIC decoding also requires CMake 3.24+ and a C++20 compiler. Install Visual Studio's
C++ CMake tools, put CMake on PATH, or set `CMAKE` to its executable path. The bundled
native sources build offline and link statically; see [native/](../native/README.md).

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
workflow provides permanent downloads only after a draft is manually published.

## Releases

`Release` reuses the CI and Windows Package workflows. A pushed `v*` tag validates
the version, runs checks, and builds both packages before creating an unpublished
GitHub Release. The draft contains the Windows ZIP, matching source ZIP, and
`SHA256SUMS` for both archives. Tags must point to a commit already on `main`.
The tag must exactly match `v<Cargo.toml version>`, and Cargo.lock must agree.
Supported versions are `X.Y.Z` and `X.Y.Z-alpha.N`, `-beta.N`, or `-rc.N`.
Prerelease versions are marked as prereleases automatically.

To preview the full build without creating a tag or release, use **Actions →
Release → Run workflow → main**. Manual runs always produce artifacts only,
including `release-notes-preview` and `release-assets`. Pull requests that change
release metadata run the fast validation and notes preview; regular CI and
Windows Package checks remain separate. Previews use `Unreleased`, falling back
to the current version's dated entry when `Unreleased` is empty after release
preparation. They do not include GitHub's generated PR list.

Before a release:

1. Update the version in Cargo.toml and Cargo.lock when needed. Keep dependency
   notices synchronized with Cargo.lock.
2. Move the relevant `Unreleased` changes into a heading such as
   `## [0.1.0] - YYYY-MM-DD`, using the actual release date. Keep an `Unreleased`
   heading for future changes. Commit these updates through a pull request.
3. After checks and package verification pass, create an annotated tag on the
   intended `main` commit and push that specific tag: `git tag -a v0.1.0 -m
   "Lumen 0.1.0"`, then `git push origin v0.1.0`. Do not move published tags.
4. Review the generated draft, download and test its Windows package, and verify
   that the source ZIP and checksums are attached. Edit the notes if needed, then
   publish manually. Select **Set as latest release** for the intended stable
   version; keep preview versions marked as prereleases.

Notes start with the curated changelog entry and download guidance, followed by
GitHub's generated list of merged PRs. `.github/release.yml` groups `enhancement`
and `bug` labels under Features and Fixes, with a catch-all for other changes.
Use `ignore-for-release` on PRs that should not appear in that generated list.
Review the complete notes before publishing, especially for the first release.

A failed draft creation can be rerun. Existing drafts receive refreshed package
assets without overwriting manually edited notes. The workflow refuses to update
an already published release and never publishes a draft or marks it as latest.
The repository remains private; publishing a release does not change its visibility.

Local metadata validation requires Python 3.11 or newer:

```powershell
python -m unittest discover -s scripts -p test_release_metadata.py
python scripts/release_metadata.py
```

Pass `--tag v0.1.0` to validate the dated changelog entry for that release.
