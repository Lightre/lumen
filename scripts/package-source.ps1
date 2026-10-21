#Requires -Version 7.0
param(
    [switch]$AllowDirty
)

$ErrorActionPreference = 'Stop'
$projectRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$gitOptions = @('-c', "safe.directory=$($projectRoot.Replace('\', '/'))", '-C', $projectRoot)
$revision = & git @gitOptions rev-parse HEAD
if ($LASTEXITCODE -ne 0) { throw 'Could not read the source revision.' }
$status = & git @gitOptions status --porcelain
if ($LASTEXITCODE -ne 0) { throw 'Could not read the working tree status.' }
if ($status -and -not $AllowDirty) {
    throw 'Source packaging requires a clean working tree. Use -AllowDirty only for local verification.'
}
$metadataJson = & cargo metadata --manifest-path (Join-Path $projectRoot 'Cargo.toml') --locked --no-deps --format-version 1
if ($LASTEXITCODE -ne 0) { throw 'Could not read package metadata.' }
$metadata = $metadataJson | ConvertFrom-Json
$package = @($metadata.packages | Where-Object { $_.name -eq 'lumen' })
if ($package.Count -ne 1) { throw 'Expected exactly one Lumen package.' }
$version = $package[0].version
$name = "lumen-$version-source"
$outputRoot = Join-Path $projectRoot 'target/packages'
$staging = Join-Path $outputRoot ([System.Guid]::NewGuid().ToString('N'))
$directory = Join-Path $staging $name
New-Item -ItemType Directory -Path $directory -Force | Out-Null

# Copy the current source tree, including local changes only when explicitly requested.
# Git metadata, ignored build outputs, and local agent guidance are not source inputs.
$files = & git @gitOptions -c core.quotePath=false ls-files --cached --others --exclude-standard
if ($LASTEXITCODE -ne 0) { throw 'Could not enumerate source files.' }
foreach ($relative in $files) {
    if ($relative -eq 'AGENTS.md') { continue }
    $source = Join-Path $projectRoot $relative
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) { continue }
    $destination = Join-Path $directory $relative
    New-Item -ItemType Directory -Path ([System.IO.Path]::GetDirectoryName($destination)) -Force | Out-Null
    Copy-Item -LiteralPath $source -Destination $destination
}

Push-Location $directory
try {
    $config = & cargo vendor --locked --versioned-dirs vendor
    if ($LASTEXITCODE -ne 0) { throw 'Could not vendor locked dependency sources.' }
    New-Item -ItemType Directory -Path '.cargo' -Force | Out-Null
    $config | Set-Content -LiteralPath '.cargo/config.toml' -Encoding utf8NoBOM
    # Confirm that Cargo resolves the complete graph using only the bundled sources.
    & cargo metadata --frozen --format-version 1 | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'The bundled dependency graph could not be resolved offline.' }
} finally {
    Pop-Location
}

$rustVersion = & rustc --version
if ($LASTEXITCODE -ne 0) { throw 'Could not read the Rust compiler version.' }
$cargoVersion = & cargo --version
if ($LASTEXITCODE -ne 0) { throw 'Could not read the Cargo version.' }
@"
Lumen $version source

Git revision: $revision
Local changes included: $([bool]$status)
Build tools used to prepare this archive:
$rustVersion
$cargoVersion

This archive contains Lumen's source, assets, build scripts, Cargo.lock, and
the locked dependency sources in vendor/. Their original notices are preserved.
See LICENSE, COPYRIGHT, BRANDING.md, and THIRD_PARTY_NOTICES.txt.

Building on Windows x64
1. Install Rust with the x86_64-pc-windows-msvc toolchain and Visual Studio
   C++ build tools, including the Windows SDK resource compiler.
2. Extract the archive and open a terminal in the directory containing Cargo.toml.
3. Run: cargo build --release --frozen --target x86_64-pc-windows-msvc
4. Find the executable at target/x86_64-pc-windows-msvc/release/lumen.exe.

.cargo/config.toml directs Cargo to vendor/. --frozen uses Cargo.lock and
prevents network access. The build tools above must already be installed;
general-purpose tools and Windows system libraries are not bundled.
The recorded tool versions identify the preparation environment, without
promising byte-for-byte identical binaries on other systems.

Distribute this archive alongside the matching Windows binary package and
provide equivalent access to both. If you modify Lumen, preserve the notices
and identify your changes. See docs/development.md for further guidance.
"@ | Set-Content -LiteralPath (Join-Path $directory 'SOURCE-README.txt') -Encoding utf8NoBOM

$checksumPath = Join-Path $directory 'SOURCE-SHA256SUMS'
Get-ChildItem -LiteralPath $directory -Recurse -File | Sort-Object FullName | ForEach-Object {
    $relative = [System.IO.Path]::GetRelativePath($directory, $_.FullName).Replace('\', '/')
    "$((Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant())  $relative"
} | Set-Content -LiteralPath $checksumPath -Encoding utf8NoBOM

# ZipFile includes dot-directories such as .cargo, unlike Compress-Archive.
$temporaryArchive = Join-Path $staging "$name.zip"
[System.IO.Compression.ZipFile]::CreateFromDirectory($directory, $temporaryArchive, 'Optimal', $true)
$archive = Join-Path $outputRoot "$name.zip"
Move-Item -LiteralPath $temporaryArchive -Destination $archive -Force
[pscustomobject]@{
    Name = $name
    Directory = $directory
    Archive = $archive
}
