# Third-party assets

- `crates/moth-ui/fonts/Hack-Regular.ttf`: Copyright 2018 Source Foundry Authors;
  Bitstream Vera Sans Mono Copyright 2003 Bitstream Inc.; DejaVu contributions
  in the public domain. MIT and Bitstream Vera licenses, reproduced in
  `crates/moth-ui/fonts/LICENSE-Hack.txt`.
  Copied unchanged from egui `epaint_default_fonts` 0.36.2,
  commit `49682f8baa058bf49e011035cfbd6e825f88a5ef`,
  https://github.com/emilk/egui/tree/49682f8baa058bf49e011035cfbd6e825f88a5ef/crates/epaint_default_fonts/fonts.

- `crates/moth-ui/fonts/NotoSans-Regular.ttf`: Noto Sans, Copyright 2022 The Noto Project Authors;
  copied unchanged from the Arch Linux `noto-fonts` package (font version 2.015). SIL Open Font License 1.1,
  reproduced in `crates/moth-ui/fonts/LICENSE-Noto.txt`.

Rust dependencies and their license expressions are recorded in `Cargo.lock`
and checked with `cargo deny check`.

## Format references

- `.bee` (BeeRef native format): the on-disk layout in `crates/moth-import` was
  learned from BeeRef's `beeref/fileio` (GPL-3.0-or-later, rbreu/beeref). No
  BeeRef source code is copied; the reader is an independent implementation of
  the documented SQLite layout.
- SQLite file format: `third_party/purcore/src/sqlite.rs` implements a read-only
  subset of the public SQLite 3 file format specification,
  https://www.sqlite.org/fileformat.html (public domain).

- `third_party/purcore`: local fork of LazyRef's `purcore`, copyright (c) 2026
  achrllrogia45, MIT; source commit
  `e59ff1fb63a7ad3b4c6d52b0ab2de540e957b654`,
  https://github.com/achrllrogia45/LazyRef. Original notice retained in
  `third_party/purcore/LICENSE`. MothRef's SQLite reader and integration changes
  are GPL-3.0-or-later. See the fork README for modifications and budgets.
- `corpus/pur/v1.10-redbar.pur` and `v2.1-redbar.pur`: captured synthetic
  red-rectangle boards copied unchanged from LazyRef's MIT-licensed test corpus
  at the same revision (`legacy/S1_redbar.pur` and `S1_redbar-new.pur`).
  Attribution and license are retained in `third_party/purcore/LICENSE`.
