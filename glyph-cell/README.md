# glyph-cell

`glyph-cell` is the public runtime crate. It is `no_std`, targets
`embedded-graphics-core` draw traits, and re-exports `font_data!` from
`glyph-cell-macros`.

## API

- `font_data!`: rasterize one or more font files into `FontData`.
- `FontData`: static glyph index, bitmap bytes, raster height, ASCII cell width,
  bpp, raw glyph metrics, and generated cell placement data.
- `Glyph`: one bitmap glyph's bounds, advance, generated cell placement, and
  bitmap offset.
- `TextStyle`: color, layout mode, character spacing/line spacing, and 3x3
  alignment.
- `DrawableText`: horizontal or vertical text drawing.

Enable the `debug` feature to draw cell or glyph boxes around rendered text.

## Example

```rust
use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
use glyph_cell::{font_data, Alignment, DrawableText, FontData, TextStyle};

const FONT: FontData<'static> = font_data! {
    size: 24,
    ascii_width: 12,
    bpp: 4,
    path: "assets/ascii.ttf",
    index: "Hello Rust!",
};

let style = TextStyle::new(Rgb565::WHITE)
    .monospace_with_spacing(1, 2)
    .align(Alignment::CENTER);

DrawableText::new(&FONT, "Hello", style)
    .at(Point::new(8, 8))
    .draw(&mut display)?;

DrawableText::new(&FONT, "Hello", style)
    .vertical()
    .at(Point::new(72, 8))
    .draw(&mut display)?;

let proportional = TextStyle::new(Rgb565::WHITE)
    .proportional_with_line_spacing(1, 2)
    .align(Alignment::TOP_LEFT);

DrawableText::new(&FONT, "Hello", proportional)
    .at(Point::new(8, 56))
    .draw(&mut display)?;
```

`draw()` writes binary pixels. If the target can blend colors, use
`for_each_coverage_pixel` with `bpp: 4` or `bpp: 8` to keep FreeType's
anti-aliased coverage instead of dithering it down to on/off pixels.

With `features = ["debug"]`:

```rust
let text = DrawableText::new(&FONT, "A", style).at(Point::new(0, 0));

text.draw_cell_boxes(&mut display)?;
text.draw_glyph_boxes(&mut display)?;
```
