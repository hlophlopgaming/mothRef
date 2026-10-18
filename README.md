# MothRef

A native reference canvas for artists, written in Rust. GPL-3.0-or-later.

![MothRef reference board with the English interface](docs/screenshots/board.png)

<details>
<summary>Board library (English interface)</summary>

![MothRef library on first launch](docs/screenshots/library.png)

</details>

Artwork shown, by [David Revoy](https://www.davidrevoy.com):
[Electron donor](https://www.peppercarrot.com/en/viewer/misc__2010-08-19-electron-donor_by-David-Revoy.html),
[Peacock dragon](https://commons.wikimedia.org/wiki/File:2013-04-28_peacock-dragon_by-David-Revoy.jpg)
and [Alice in Wonderland](https://www.davidrevoy.com/article36/alice-in-wonderland),
licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).
The illustrations are scaled for display in the screenshot.

Download experimental binaries from [GitHub Releases](https://github.com/hlophlopgaming/mothRef/releases).
Windows, Linux x86_64 and macOS (Apple Silicon / Intel) archives are available.

The M1 preview supports `.moth` saves, recovery journals and undo/redo. It is an
experimental early release: full reliability qualification and compressed textures
remain open. Keep separate copies of important boards and original references.

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
or clipboard paste. Enter a `.moth` destination in **Board path** and choose
**Save**; use **Open** from an empty canvas, or pass a board filename at startup.
Recovery journals and staged originals live in application data. Explicit discard
removes that session's recovery files; saved-board backups are retained.


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
[M1 validation notes](docs/M1-VALIDATION.md) for scope, budgets and remaining checks.
See [releasing](docs/RELEASING.md) for the GitHub binary pipeline.
