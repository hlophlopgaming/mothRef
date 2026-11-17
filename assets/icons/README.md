# Native application icons

The artwork and PNG exports live in [`docs/assets/icon`](../../docs/assets/icon/README.md).
The moth holding a gallery is the default; the engraved mark is optional.

- `moth.rgba` / `engraved.rgba`: unpremultiplied RGBA8 bytes of the canonical
  64×64 PNGs, for egui/winit without decoding on the event-loop thread.
- `moth.ico`: Windows icon, exported with Pillow from `mothref-icon-1024.png`,
  at 16, 24, 32, 48, 64, 128 and 256 px.
- `moth.icns`: macOS bundle/document icon, exported with Pillow from the same PNG.

The two egui textures belong to their window context and are freed with it.
