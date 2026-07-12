# glyph-cell-macros

`glyph-cell-macros` implements the `font_data!` procedural macro used
by `glyph-cell`.

Most users should import the macro from the public runtime crate instead:

```rust
use glyph_cell::{font_data, FontData};
```

## Macro Shape

```rust
const FONT: FontData<'static> = font_data! {
    size: 24,
    // `height: 24` is also accepted.
    ascii_width: 12,
    path: "assets/ascii.ttf",
    index: "Hello Rust!",
    y_offset: {
        'g': -1,
    },
    path: "assets/cjk.otf",
    index: "你好世界",
};
```

Each invocation creates one raster height. Multiple `path` + `index` blocks may
be mixed in the same invocation, for example one ASCII block and one CJK block. The
macro deduplicates repeated characters inside one block, rejects duplicate
characters across blocks, and emits the rasterized bitmap plus raw glyph bounds,
advance width, generated cell width, and generated bitmap offset for runtime
layout.

For each font block, the macro applies one shared vertical adjustment for ASCII
glyphs and another shared vertical adjustment for non-ASCII glyphs so both
groups fit the requested raster height as consistently as possible. Add an
optional `y_offset` map after a block's `index` to nudge individual glyphs after
that shared adjustment. Values are pixel deltas and characters must appear in
that block's `index`.

`ascii_width` is optional and defaults to `size`. ASCII characters get that cell
width; every other character gets `size` as its cell width.
