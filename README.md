# MothRef

A native reference canvas for artists, written in Rust. GPL-3.0-or-later.

M0 is a runnable **volatile preview**: layouts are not saved. Saving, recovery
and undo/redo UI are scheduled for M1. Keep imported source files available.

```sh
cargo run -p mothref --release
cargo run -p mothref --release -- image.png reference.jpg
cargo run -p mothref --release -- --software
```

Drop PNG/JPEG files, or use **Paste image** / Ctrl+V (Cmd+V on macOS). Pan by
left-drag in Pan mode or middle-drag in either mode. Use the wheel or +/− controls
to zoom; Select mode draws a selection rectangle. **Fit board** or Home frames
all items. Touch supports single-contact pan/selection and two-contact pan/pinch.

Previews support up to 10,000 items and 2048 pixels of detail per image. Native
Wayland file drops are unavailable in winit 0.30; use X11/XWayland, image arguments
or clipboard paste. Clipboard originals are staged in the OS temporary directory
under `mothref-recovery` (512 MiB / 10,000-entry budget); their `.rgba` files can be
opened as image arguments, but contain no layout information.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
cargo xtask bench --quick
cargo xtask corpus
cargo xtask paste
cargo xtask soak 1800
```

`xtask` runs optimized builds so measurements do not include debug-code overhead.
Use `bench --quick --enforce` on reference hardware to enforce 8 ms p99, and
`bench --quick --software` to exercise a CPU adapter. The paste measurement starts
after OS clipboard transfer and includes durable staging, preview construction,
upload and GPU completion; it is not an end-to-end clipboard claim.

For decoder fuzzing, install `cargo-fuzz` and nightly, then run:

```sh
mkdir -p fuzz/corpus/image_reader
cp corpus/images/* fuzz/corpus/image_reader/
cargo +nightly fuzz run image_reader fuzz/corpus/image_reader -- -max_total_time=300
```

See [ROADMAP.md](ROADMAP.md), [architecture](docs/ARCHITECTURE.md) and
[validation notes](docs/M0-VALIDATION.md) for scope, budgets and remaining checks.
