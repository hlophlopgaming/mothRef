# M0 image corpus

Synthetic fixtures, generated for this repository (GPL-3.0-or-later):

- `images/rows.png`: 31×17 red-to-blue gradient, 8-bit RGBA.
- `images/adam7.png`: the same pixels with PNG Adam7 interlacing.
- `images/4k.jpg`: 3840×2160 red-to-blue gradient, 4:2:0 JPEG.
- `images/clipboard.rgba`: 16×8 solid RGBA (20,40,60,255), encoded with
  `moth_io::write_rgba`, for scratch-record validation/fuzzing.

PNG/JPEG fixtures were generated with ImageMagick's normal image encoders.
Unit tests check scanline sampling, Adam7 equivalence, DCT preview allocation,
scratch-record identity/checksum and truncation at every byte.

`cargo xtask corpus` decodes every image fixture. Board-file round-trips begin
when M1 introduces a `.moth` reader/writer; `.bee` and `.pur` fixtures arrive in M2.
