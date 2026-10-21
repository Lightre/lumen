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
$readme = Join-Path $directory 'README.txt'
@"
Lumen $version

A minimal photo viewer for Windows x64.

Extract the entire ZIP before running lumen.exe. Use Open image or Ctrl+O to
select a JPEG, PNG, or WebP file. You can also drop a photo onto the window
or the executable, or pass its path when launching Lumen from a terminal.

Left / Right: previous / next image
Mouse wheel: zoom around the pointer
Left mouse drag: pan a zoomed image
Home: fit to window
Double-click: toggle Fit / 100%
F11: toggle full screen
Esc: exit full screen

This is an early development build. Images are limited to 32 megapixels and
256 MiB per file. Large dimensions may use a reduced preview. Animated images
and color profile management are not supported. No installer or automatic
file association registration is included.

SHA256SUMS contains checksums for lumen.exe and this README.
"@ | Set-Content -LiteralPath $readme -Encoding utf8
$checksums = Join-Path $directory 'SHA256SUMS'
@($packagedExe, $readme) | ForEach-Object {
    $hash = (Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $([System.IO.Path]::GetFileName($_))"
} | Set-Content -LiteralPath $checksums -Encoding ascii

$archive = Join-Path $outputRoot "$name.zip"
Compress-Archive -LiteralPath @($packagedExe, $readme, $checksums) -DestinationPath $archive -Force
[pscustomobject]@{
    Name = $name
    Directory = $directory
    Archive = $archive
}
