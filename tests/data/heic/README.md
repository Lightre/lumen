# HEIC fixtures

- `rainbow.heic` and `alpha.heic` are unmodified `rainbow-451x461.heic` and
  `with-alpha-512x512.heic` from strukturag/libheif at
  `f81f28ac014b1c28dee483ac45b72c6b05bc421a`, under its LGPL-3.0-or-later terms.
  Their complete original source and notices are in `native/sources/libheif-1.23.6.tar.gz`
  and the generated `THIRD_PARTY_NOTICES.txt`.
- `orientation-2.heic` and `orientation-6.heic` are CC0 procedural charts from
  https://heic.fyi/heic-sample-files; see `LICENSE-CC0.txt`. They encode HEIF
  transforms and EXIF orientation together, to detect missing or double transforms.
  Both decode upright to 1200 x 800. Original SHA-256 values:
  - orientation-2: `3a316089f67cb43b6f92667b964c3ebff2542c72db1f1d0328f400fa5db48111`
  - orientation-6: `80b615ae433d7ec53e9aaaeee081ecd11c0d9931350da13d6ce4e2b363d12c12`
