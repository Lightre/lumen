param(
    [string]$ExecutablePath = (Join-Path $PSScriptRoot '..\target\release\lumen.exe')
)

$ErrorActionPreference = 'Stop'
$projectRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$metadataJson = & cargo metadata --manifest-path (Join-Path $projectRoot 'Cargo.toml') --locked --no-deps --format-version 1
if ($LASTEXITCODE -ne 0) { throw 'Could not read package metadata.' }
$metadata = $metadataJson | ConvertFrom-Json
$package = @($metadata.packages | Where-Object { $_.name -eq 'lumen' })
if ($package.Count -ne 1) { throw 'Expected exactly one Lumen package.' }
$version = $package[0].version
$executable = (Resolve-Path -LiteralPath $ExecutablePath).Path
$legalFiles = @('LICENSE', 'COPYRIGHT', 'BRANDING.md', 'THIRD_PARTY_NOTICES.txt')
foreach ($legalName in $legalFiles) {
    if (-not (Test-Path -LiteralPath (Join-Path $projectRoot $legalName) -PathType Leaf)) {
        throw "Missing legal notice: $legalName"
    }
}
$lockText = [System.IO.File]::ReadAllText((Join-Path $projectRoot 'Cargo.lock')).Replace("`r`n", "`n")
$lockBytes = [System.Text.Encoding]::UTF8.GetBytes($lockText)
$lockHash = [System.Convert]::ToHexString([System.Security.Cryptography.SHA256]::HashData($lockBytes)).ToLowerInvariant()
$notices = Get-Content -LiteralPath (Join-Path $projectRoot 'THIRD_PARTY_NOTICES.txt') -Raw
if (-not $notices.Contains("Cargo.lock SHA-256: $lockHash")) {
    throw 'Third-party notices are out of date. Run python scripts/update-third-party-notices.py and review the output.'
}
$rustRoot = & rustc --print sysroot
if ($LASTEXITCODE -ne 0) { throw 'Could not locate Rust standard library notices.' }
$rustNotices = Join-Path $rustRoot 'share/doc/rust/COPYRIGHT-library.html'
if (-not (Test-Path -LiteralPath $rustNotices -PathType Leaf)) {
    throw 'Rust standard library notices are missing from the active toolchain.'
}

$reader = [System.IO.BinaryReader]::new([System.IO.File]::OpenRead($executable))
try {
    if ($reader.ReadUInt16() -ne 0x5a4d) { throw 'The input is not a Windows executable.' }
    $reader.BaseStream.Position = 0x3c
    $peOffset = $reader.ReadInt32()
    $reader.BaseStream.Position = $peOffset
    if ($reader.ReadUInt32() -ne 0x4550) { throw 'The executable has an invalid PE header.' }
    if ($reader.ReadUInt16() -ne 0x8664) { throw 'The package requires an x64 executable.' }
    $reader.BaseStream.Position = $peOffset + 24 + 68
    if ($reader.ReadUInt16() -ne 2) { throw 'The package requires a Windows GUI executable.' }
} finally {
    $reader.Dispose()
}

$name = "lumen-$version-windows-x64"
$outputRoot = Join-Path $projectRoot 'target\packages'
$directory = Join-Path $outputRoot $name
New-Item -ItemType Directory -Path $directory -Force | Out-Null
$packagedExe = Join-Path $directory 'lumen.exe'
Copy-Item -LiteralPath $executable -Destination $packagedExe -Force
$packagedLegal = foreach ($legalName in $legalFiles) {
    $destination = Join-Path $directory $legalName
    Copy-Item -LiteralPath (Join-Path $projectRoot $legalName) -Destination $destination -Force
    $destination
}
$packagedRustNotices = Join-Path $directory 'RUST_STANDARD_LIBRARY_NOTICES.html'
Copy-Item -LiteralPath $rustNotices -Destination $packagedRustNotices -Force
$readme = Join-Path $directory 'README.txt'
@"
Lumen $version

A minimal photo viewer for Windows x64.

Extract the entire ZIP before running lumen.exe. Use Open image or Ctrl+O to
select a JPEG, PNG, WebP, GIF, BMP, or TIFF file. You can also drop a photo onto the window
or the executable, or pass its path when launching Lumen from a terminal.

Left / Right: previous / next image
Mouse wheel: zoom around the pointer
Left mouse drag: pan a zoomed image
Home: fit to window
R / Shift+R: rotate right / left (view only)
Space: pause / resume GIF animation (replay after completion)
Double-click: toggle Fit / 100%
F11: toggle full screen
Esc: exit full screen

This is an early development build. Images are limited to 32 megapixels and
256 MiB per file. Large dimensions may use a reduced preview. GIF animations
play automatically; the decoded frame cache is limited to 128 MiB and 1000
frames. Animated PNG/WebP and color profile management are not supported.
TIFF files (.tif and .tiff) display the first image only.
No installer or automatic file association registration is included.

Copyright (C) 2026 Lightre. Lumen is licensed under GNU GPL version 3 only,
without warranty. See LICENSE and COPYRIGHT. Third-party components retain
their own licenses; see THIRD_PARTY_NOTICES.txt and RUST_STANDARD_LIBRARY_NOTICES.html.
See BRANDING.md for the name and logo policy.

The matching source archive is lumen-$version-source.zip, supplied separately
alongside this package. Redistribute both with equivalent access, as required
by GPLv3 section 6. The source archive includes build instructions and locked
dependency sources. A private repository link alone is not source delivery.

SHA256SUMS contains checksums for the executable, this guide, and legal notices.
"@ | Set-Content -LiteralPath $readme -Encoding utf8
$checksums = Join-Path $directory 'SHA256SUMS'
$contents = @($packagedExe, $readme) + @($packagedLegal) + @($packagedRustNotices)
$contents | ForEach-Object {
    $hash = (Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $([System.IO.Path]::GetFileName($_))"
} | Set-Content -LiteralPath $checksums -Encoding ascii

$archive = Join-Path $outputRoot "$name.zip"
Compress-Archive -LiteralPath ($contents + @($checksums)) -DestinationPath $archive -Force
[pscustomobject]@{
    Name = $name
    Directory = $directory
    Archive = $archive
}
