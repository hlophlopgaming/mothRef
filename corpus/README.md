# Image and board corpus

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
