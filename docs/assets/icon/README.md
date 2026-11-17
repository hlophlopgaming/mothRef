# MothRef icons

| File | Role |
| --- | --- |
| `mothref-icon.svg` | Main app icon: the moth holding a gallery. Transparent background. |
| `mothref-icon-engraved.svg` | Alternative icon in an engraved style, for a future "change app icon" option. |

PNG exports sit next to each SVG (`-16` … `-1024` px for the main icon, `-64` … `-1024` px for the engraved one).
Regenerate them with `rsvg-convert -w N -h N <file>.svg -o <file>-N.png`.

The engraved icon uses "Moth 5" by Firkin from Openclipart (public domain); see `THIRD_PARTY.md`.

Palette: tile `#d68a55` (engraved icon); main icon body `#ffffff`, outline `#d6b58b`,
antennae `#e8cfae` / `#c99f74` with shafts `#a8764b`, eyes `#3a2619`.
