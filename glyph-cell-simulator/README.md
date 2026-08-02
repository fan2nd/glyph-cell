# glyph-cell-simulator

Desktop simulator for previewing `glyph-cell` layout on a PC.

The simulator is a Tauri desktop app. The UI is static HTML/CSS/JS, while Rust
commands keep ownership of font discovery, FreeType rasterization, the real
`DrawableText` render path, and generated example code.

Preview glyphs are rasterized at runtime with FreeType from either:

- a discovered system `.ttf`, `.otf`, or `.ttc` font
- a user-selected font file

For TTC files, use the collection index control to choose a face.

In monospace layout, `ASCII cell width` is a layout parameter; non-ASCII
characters use the raster height as their cell width. The simulator keeps this
out of the rasterization cache and applies it to the preview render data.
Proportional layout uses each glyph's actual bitmap width while keeping the
raster height for its display cell. Both layout modes expose character spacing
and line spacing controls.

The simulator renders the same generated 1bpp bitmap path as the runtime. It
also warns when glyphs are missing from the selected font or vertically clipped
by the generated cell.

The `Glyph y_offset tweaks` field accepts one tweak per line, for example
`g: -1` or `A: 1`. These preview tweaks use the same pixel-delta semantics as
the macro's block-local `y_offset` map.

Run it with:

```bash
cargo run -p glyph-cell-simulator
```

The Tauri app embeds `ui/index.html`, so no Node package install or frontend
build step is required.
