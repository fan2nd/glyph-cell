#![cfg(feature = "debug")]

use embedded_graphics::{mock_display::MockDisplay, pixelcolor::BinaryColor};
use glyph_cell::*;

const FONT: FontData<'static> = FontData {
    index: "ABg",
    size: 5,
    ascii_width: 4,
    bitmap: &BITMAP,
    glyphs: &GLYPHS,
};

const GLYPHS: [Glyph; 3] = [
    Glyph {
        bitmap_offset: 0,
        width: 3,
        height: 5,
        cell_width: 4,
        x_offset: 1,
        y_offset: 5,
        x_min: 0,
        y_min: 0,
        advance_width: 4,
    },
    Glyph {
        bitmap_offset: 2,
        width: 3,
        height: 5,
        cell_width: 4,
        x_offset: 1,
        y_offset: 5,
        x_min: 0,
        y_min: 0,
        advance_width: 5,
    },
    Glyph {
        bitmap_offset: 4,
        width: 1,
        height: 1,
        cell_width: 4,
        x_offset: 2,
        y_offset: 1,
        x_min: 0,
        y_min: 0,
        advance_width: 2,
    },
];

const BITMAP: [u8; 5] = [0b01010111, 0b11011010, 0b11010111, 0b01011100, 0b10000000];

fn monospace() -> TextStyle<BinaryColor> {
    TextStyle::new(BinaryColor::On)
        .monospace()
        .align(Alignment::TOP_LEFT)
}

fn proportional(spacing: i32) -> TextStyle<BinaryColor> {
    TextStyle::new(BinaryColor::On)
        .proportional(spacing)
        .align(Alignment::TOP_LEFT)
}

#[test]
fn draws_monospace_cell_debug_box_from_generated_cell() {
    let mut display = MockDisplay::<BinaryColor>::new();
    display.set_allow_overdraw(true);
    let text = DrawableText::new(&FONT, "A", monospace());

    text.draw_cell_boxes(&mut display).unwrap();

    display.assert_pattern(&["####", "#  #", "#  #", "#  #", "####"]);
}

#[test]
fn draws_proportional_cell_debug_boxes_from_actual_widths() {
    let mut display = MockDisplay::<BinaryColor>::new();
    display.set_allow_overdraw(true);
    let text = DrawableText::new(&FONT, "AB", proportional(1));

    text.draw_cell_boxes(&mut display).unwrap();

    display.assert_pattern(&["### ###", "# # # #", "# # # #", "# # # #", "### ###"]);
}

#[test]
fn draws_proportional_glyph_debug_box() {
    let mut display = MockDisplay::<BinaryColor>::new();
    display.set_allow_overdraw(true);
    let text = DrawableText::new(&FONT, "A", proportional(0));

    text.draw_glyph_boxes(&mut display).unwrap();

    display.assert_pattern(&["###", "# #", "# #", "# #", "###"]);
}

#[test]
fn draws_vertical_proportional_glyph_boxes_upright() {
    let mut display = MockDisplay::<BinaryColor>::new();
    display.set_allow_overdraw(true);
    let text = DrawableText::new(&FONT, "Ag", proportional(1)).vertical();

    text.draw_glyph_boxes(&mut display).unwrap();

    display.assert_pattern(&[
        "###", "# #", "# #", "# #", "###", "   ", "   ", "   ", "   ", "   ", "#  ",
    ]);
}
