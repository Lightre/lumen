# Native HEIC decoder

Lumen statically links libheif and its libde265 HEVC backend. Users do not need
codec extensions, a separate C++ runtime installer, or additional decoder DLLs.
Other codecs, encoders, dynamic plugins, examples and tools are disabled.

`sources/manifest.json` records the versions, upstream revisions, licenses and
SHA-256 hashes of the complete source archives. Archives were produced with
`git archive --format=tar.gz --prefix=<name>/ <revision>` from the recorded commits.
Upstream code and notices are unmodified. The CMake build extracts sources under
Cargo's build output directory. It never downloads dependencies.

Requires Windows x64 MSVC, Windows SDK, CMake 3.24+ and a C++20 compiler. CMake can
be on PATH, selected with `CMAKE`, or supplied by Visual Studio's C++ CMake tools.
Native libraries use optimized builds even for Rust debug builds. Both Rust and
C++ runtimes link statically through `.cargo/config.toml` and CMake settings.

`heic.cpp` exposes a small C ABI for the image loader. It returns the main image
as 8-bit RGBA, applies HEIF crop/rotation/mirroring once, and keeps native pixel
storage alive until Rust finishes copying it. C++ exceptions cannot cross the ABI.
Context limits bound image area and libheif-managed allocations; libde265 also
receives the image-area limit. These are decoder limits, not a process memory cap.
Tile decoding uses at most two library workers and one codec worker per tile.

When updating either archive, update the manifest and CMake filename together,
review upstream fixes and notices, regenerate `THIRD_PARTY_NOTICES.txt`, and verify
the Windows package and an offline build of the matching source archive.
