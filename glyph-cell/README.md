# glyph-cell

`glyph-cell` is the public runtime crate. It is `no_std`, targets
`embedded-graphics` draw traits, and re-exports `font_data!` from
`glyph-cell-macros`.

## API

- `font_data!`: rasterize one or more font files into `FontData`.
- `FontData`: static glyph index, bitmap bytes, raster height, ASCII cell width,
  raw glyph metrics, and generated cell placement data.
- `Glyph`: one bitmap glyph's bounds, advance, generated cell placement, and
  bitmap offset.
- `TextStyle`: color, layout mode, character spacing/line spacing, and 3x3
  alignment.
- `DrawableText`: horizontal or vertical text drawing.
- `GlyphCellTextStyle`: character style for `embedded_graphics::text::Text`.

Enable the `debug` feature to draw cell or glyph boxes around rendered text.

## Example

```rust
use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
use embedded_graphics::text::{Baseline, Text};
use glyph_cell::{font_data, Alignment, DrawableText, FontData, GlyphCellTextStyle, TextStyle};

const FONT: FontData<'static> = font_data! {
    size: 24,
    ascii_width: 12,
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

let embedded_text_style = GlyphCellTextStyle::new(&FONT, Rgb565::WHITE)
    .monospace_with_spacing(1, 2);

Text::with_baseline(
    "Hello",
    Point::new(8, 88),
    embedded_text_style,
    Baseline::Top,
)
.draw(&mut display)?;
```

`draw()` writes binary pixels from the generated 1bpp bitmap.

With `features = ["debug"]`:

```rust
let text = DrawableText::new(&FONT, "A", style).at(Point::new(0, 0));

text.draw_cell_boxes(&mut display)?;
text.draw_glyph_boxes(&mut display)?;
```
