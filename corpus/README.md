# Image and board corpus

- `bee/invalid/cyclic.bin`: fuzz-generated corrupt SQLite/BeeRef database with
  cyclic table-page references. Import must return an error; the `.bin` suffix
  keeps this negative fixture out of the successful-import corpus pass.

Synthetic fixtures, generated for this repository (GPL-3.0-or-later):

- `images/rows.png`: 31×17 red-to-blue gradient, 8-bit RGBA.
- `images/adam7.png`: the same pixels with PNG Adam7 interlacing.
- `images/4k.jpg`: 3840×2160 red-to-blue gradient, 4:2:0 JPEG.
- `images/clipboard.rgba`: 16×8 solid RGBA (20,40,60,255), encoded with
  `moth_io::write_rgba`, for scratch-record validation/fuzzing.

PNG/JPEG fixtures were generated with ImageMagick's normal image encoders.
Unit tests check area averaging, Adam7 equivalence, DCT preview allocation,
scratch-record identity/checksum and truncation at every byte.

- `moth/v1.moth`: schema-one checkpoint with one UUIDv7 image and its original
  `rows.png` bytes. Regenerate intentionally with `cargo xtask fixture`.

`cargo xtask corpus` decodes every image fixture and round-trips every `.moth`
fixture, comparing all item metadata. `.bee` and `.pur` fixtures arrive in M2.

- `images/large-dct.jpg`: 8192×4097 solid RGB (120,90,60), baseline JPEG;
  exceeds the old source-pixel ceiling but fits a DCT-reduced decode budget.
- `images/cmyk.jpg`: 8×4 CMYK (0,128,255,0), JPEG quality 95.
- `images/oriented.jpg`: 8×4 RGB (120,90,60), EXIF orientation 6 (90° clockwise).

These three synthetic JPEGs were generated with Pillow's normal encoder. Tests
check reduced allocations, progressive coefficient rejection, CMYK colors and
oriented preview/geometry dimensions.

TD-07 synthetic fixtures (GPL-3.0-or-later):

- `images/rows.gif`, `interlaced.gif`: 31×17 red-to-blue indexed gradient,
  ordinary and GIF-interlaced; generated with ImageMagick.
- `images/offset.gif`: 7×5 logical canvas, transparent 3×2 red/blue frame at
  (2,1); generated with Pillow and explicit GIF logical/frame dimensions.
- `images/rows.bmp`: 31×17 24-bit bottom-up gradient, BMP v3.
- `images/palette.bmp`: the gradient quantized to eight colors, paletted BMP v3.
- `images/alpha.bmp`: the gradient at 50% alpha, BMP with explicit channel masks.
- `images/4k.gif`: 3840×2160 solid red, for row-decoder allocation checks.
- `images/4k.bmp`: 3840×2160 monochrome checkerboard, 1-bit uncompressed BMP.

BMP/GIF gradient and 4K fixtures use ImageMagick's ordinary encoders. Tests
compare area-average pixels against reference decoders, including interlacing,
transparent frame offsets, BMP palettes/alpha, top-down rows and invalid layouts.

WebP fixtures, generated with ImageMagick's ordinary encoders unless noted:

- `images/lossless.webp`, `lossy.webp`: 31×17 red-to-blue gradient, VP8L/VP8.
- `images/alpha.webp`: the gradient at 50% alpha, lossless WebP.
- `images/oriented.webp`: the lossless gradient with EXIF orientation 6,
  generated with Pillow.
- `images/budget.webp`: 2048×1024 solid RGB (128,80,64), exactly the admitted
  8 MiB RGBA frame ceiling. The oversize test changes its header dimensions
  to 3840×2160 and checks rejection before decoding pixels.

The negative animation fixture in `crates/moth-io/tests/fixtures/animated.webp`
contains two 2×2 solid red/blue frames, generated with Pillow (synthetic,
GPL-3.0-or-later). It is outside the successful-import corpus because static
WebP admission intentionally rejects it.

PureRef baseline corpus (LazyRef, MIT; see `THIRD_PARTY.md` and
`third_party/purcore/LICENSE`): `pur/v1.10-redbar.pur` and
`pur/v2.1-redbar.pur`, captured through the normal PureRef UI upstream.
Both hold the same original 64×48 PNG, displayed at 1000×750 at (0,0).
`cargo xtask corpus` imports them and checks conversion to `.moth` and reload.
These two versions do not qualify all historical PureRef variants.

Private local stress input: `FemaleFighterPosesPack.pur` (175 images, 198.54 MiB,
PureRef 2.1.2), intentionally excluded from the committed corpus. Run
`cargo xtask pur FemaleFighterPosesPack.pur` to check the real storage worker,
`.moth` save/reload, original blob identities and grouped undo/redo. The schema
column-order regression is represented in unit tests without redistributing art.
