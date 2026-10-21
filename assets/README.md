# Visual assets

`icon.svg` is the editable application icon master. `mark.svg` contains the mark
on a transparent background. Both use a 64-unit grid with outlined geometry and
no font dependencies.

`icon.png` is the 256 × 256 window icon. `icon.ico` contains PNG frames at 16, 24,
32, 48, 64, 128, and 256 pixels for Windows Explorer and the taskbar. These exports
are committed so application builds do not require a vector editor.

When changing the SVG, regenerate both raster exports from the master and inspect
the small sizes before committing.

The README banner and static badges are in `readme/`. They use SVG geometry and
system font references, with no embedded fonts or external image dependencies.

Original assets are licensed under GPL-3.0-only. See [LICENSE](../LICENSE),
[COPYRIGHT](../COPYRIGHT), and [BRANDING.md](../BRANDING.md) for the project's
licensing and brand identification policy.
