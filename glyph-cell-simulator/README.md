# glyph-cell-simulator

Desktop simulator for previewing `glyph-cell` layout on a PC.

The UI uses `egui` through `eframe`, which keeps the simulator Rust-native and
lets it share the same workspace, types, and rendering path as the runtime
crate.

The simulator loads a CJK-capable system font into `egui` on startup so Chinese
text in controls and code previews does not render as tofu boxes. Preview glyphs
are rasterized at runtime with FreeType from either:

- a discovered system `.ttf`, `.otf`, or `.ttc` font
- a user-provided font file path

For TTC files, use the collection index control to choose a face.

In monospace layout, generated `FontData` supplies each cell width and bitmap
offset. `ASCII cell width` is a font generation parameter; non-ASCII characters
use the raster height as their cell width. Proportional layout uses each glyph's
actual bitmap width while keeping the raster height for its display cell. Both
layout modes expose character spacing and line spacing controls.

The bpp control matches the macro's bitmap depth. Multi-bpp previews use
coverage blending instead of binary dithering, so 4bpp and 8bpp output show the
anti-aliased shape FreeType produced. The simulator also warns when glyphs are
missing from the selected font or clipped by the generated cell.

The `Glyph y_offset tweaks` field accepts one tweak per line, for example
`g: -1` or `A: 1`. These preview tweaks use the same pixel-delta semantics as
the macro's block-local `y_offset` map.

Run it with:

```bash
cargo run -p glyph-cell-simulator
```
