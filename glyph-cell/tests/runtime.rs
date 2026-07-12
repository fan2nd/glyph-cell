use embedded_graphics::{mock_display::MockDisplay, pixelcolor::BinaryColor, prelude::*};
use glyph_cell::*;

const FONT: FontData<'static> = FontData {
    index: "ABg\u{4f60}",
    size: 5,
    ascii_width: 4,
    bitmap: &BITMAP,
    glyphs: &GLYPHS,
};

const GLYPHS: [Glyph; 4] = [
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
    Glyph {
        bitmap_offset: 5,
        width: 1,
        height: 1,
        cell_width: 5,
        x_offset: 2,
        y_offset: 3,
        x_min: 0,
        y_min: 2,
        advance_width: 2,
    },
];

// A: .#./#.#/###/#.#/#.#
// B: ##./#.#/##./#.#/##.
// g: #
// U+4F60: #
const BITMAP: [u8; 6] = [
    0b01010111, 0b11011010, 0b11010111, 0b01011100, 0b10000000, 0b10000000,
];

const ONE_BPP_FONT: FontData<'static> = FontData {
    index: "x",
    size: 1,
    ascii_width: 4,
    bitmap: &[0b10100000],
    glyphs: &[Glyph {
        bitmap_offset: 0,
        width: 4,
        height: 1,
        cell_width: 4,
        x_offset: 0,
        y_offset: 1,
        x_min: 0,
        y_min: 0,
        advance_width: 4,
    }],
};

fn monospace() -> TextStyle<BinaryColor> {
    TextStyle::new(BinaryColor::On)
        .monospace()
        .align(Alignment::TOP_LEFT)
}

fn monospace_with_spacing(spacing: i32, line_spacing: i32) -> TextStyle<BinaryColor> {
    TextStyle::new(BinaryColor::On)
        .monospace_with_spacing(spacing, line_spacing)
        .align(Alignment::TOP_LEFT)
}

fn proportional(spacing: i32) -> TextStyle<BinaryColor> {
    TextStyle::new(BinaryColor::On)
        .proportional(spacing)
        .align(Alignment::TOP_LEFT)
}

fn proportional_with_line_spacing(spacing: i32, line_spacing: i32) -> TextStyle<BinaryColor> {
    TextStyle::new(BinaryColor::On)
        .proportional_with_line_spacing(spacing, line_spacing)
        .align(Alignment::TOP_LEFT)
}

#[test]
fn finds_glyph_by_index_string() {
    let glyph = FONT.glyph('A').unwrap();
    assert_eq!(glyph.bitmap_offset, 0);
    assert_eq!(glyph.advance_width, 4);
    assert_eq!(FONT.glyph('Z'), None);
}

#[test]
fn font_data_reports_generated_cell_widths() {
    assert_eq!(FONT.cell_width('A'), 4);
    assert_eq!(FONT.cell_width('\u{4f60}'), 5);
    assert_eq!(FONT.cell_width('Z'), 4);
    assert_eq!(FONT.cell_width('\u{597d}'), 5);
}

#[test]
fn monospace_layout_measures_generated_cells() {
    let horizontal = DrawableText::new(&FONT, "AB\u{4f60}", monospace());
    let vertical = DrawableText::new(&FONT, "AB\u{4f60}", monospace()).vertical();

    assert_eq!(horizontal.measure(), Size::new(13, 5));
    assert_eq!(vertical.measure(), Size::new(5, 15));
}

#[test]
fn monospace_layout_uses_fontdata_fallback_cell_widths_for_missing_glyphs() {
    let horizontal = DrawableText::new(&FONT, "A\u{597d}Z", monospace());
    let vertical = DrawableText::new(&FONT, "A\u{597d}Z", monospace()).vertical();

    assert_eq!(horizontal.measure(), Size::new(13, 5));
    assert_eq!(vertical.measure(), Size::new(5, 15));
}

#[test]
fn monospace_layout_measures_spacing_and_line_spacing() {
    let horizontal = DrawableText::new(&FONT, "A\nB", monospace_with_spacing(1, 2));
    let vertical = DrawableText::new(&FONT, "A\nB", monospace_with_spacing(1, 2)).vertical();

    assert_eq!(horizontal.measure(), Size::new(4, 12));
    assert_eq!(vertical.measure(), Size::new(10, 5));
}

#[test]
fn proportional_layout_measures_actual_widths_and_spacing() {
    let text = DrawableText::new(&FONT, "AB", proportional(1));

    assert_eq!(text.measure(), Size::new(7, 5));
}

#[test]
fn proportional_layout_measures_line_spacing() {
    let horizontal = DrawableText::new(&FONT, "A\nB", proportional_with_line_spacing(0, 2));
    let vertical =
        DrawableText::new(&FONT, "A\nB", proportional_with_line_spacing(0, 2)).vertical();

    assert_eq!(horizontal.measure(), Size::new(3, 12));
    assert_eq!(vertical.measure(), Size::new(8, 5));
}

#[test]
fn proportional_vertical_layout_stacks_upright_display_cells() {
    let text = DrawableText::new(&FONT, "Ag", proportional(1)).vertical();

    assert_eq!(text.measure(), Size::new(3, 11));
}

#[test]
fn missing_glyph_uses_fontdata_cell_width_in_proportional_layout() {
    let text = DrawableText::new(&FONT, "AZ", proportional(1));

    assert_eq!(text.measure(), Size::new(8, 5));
}

#[test]
fn draws_monospace_horizontal_text_from_generated_cells() {
    let mut display = MockDisplay::<BinaryColor>::new();
    let text = DrawableText::new(&FONT, "AB", monospace());

    text.draw(&mut display).unwrap();

    display.assert_pattern(&["  #  ## ", " # # # #", " ### ## ", " # # # #", " # # ## "]);
}

#[test]
fn draws_monospace_glyphs_at_generated_offsets() {
    let mut display = MockDisplay::<BinaryColor>::new();
    let text = DrawableText::new(&FONT, "A\u{4f60}", monospace());

    text.draw(&mut display).unwrap();

    display.assert_pattern(&["  #    ", " # #   ", " ###  #", " # #   ", " # #   "]);
}

#[test]
fn draws_monospace_vertical_text_upright_from_generated_offsets() {
    let mut display = MockDisplay::<BinaryColor>::new();
    let text = DrawableText::new(&FONT, "\u{4f60}A", monospace()).vertical();

    text.draw(&mut display).unwrap();

    display.assert_pattern(&[
        "     ", "     ", "  #  ", "     ", "     ", "  #  ", " # # ", " ### ", " # # ", " # # ",
    ]);
}

#[test]
fn draws_proportional_horizontal_text_using_actual_widths() {
    let mut display = MockDisplay::<BinaryColor>::new();
    let text = DrawableText::new(&FONT, "AB", proportional(1));

    text.draw(&mut display).unwrap();

    display.assert_pattern(&[" #  ## ", "# # # #", "### ## ", "# # # #", "# # ## "]);
}

#[test]
fn draw_path_keeps_1bpp_bitmap_semantics() {
    let mut display = MockDisplay::<BinaryColor>::new();
    let text = DrawableText::new(&ONE_BPP_FONT, "x", monospace());

    text.draw(&mut display).unwrap();

    display.assert_pattern(&["# # "]);
}

#[test]
fn draws_proportional_vertical_text_upright() {
    let mut display = MockDisplay::<BinaryColor>::new();
    let text = DrawableText::new(&FONT, "Ag", proportional(1)).vertical();

    text.draw(&mut display).unwrap();

    display.assert_pattern(&[
        " # ", "# #", "###", "# #", "# #", "   ", "   ", "   ", "   ", "   ", "#  ",
    ]);
}

#[test]
fn proportional_glyph_uses_generated_y_offset() {
    const DESCENDER_FONT: FontData<'static> = FontData {
        index: "Ag",
        size: 5,
        ascii_width: 4,
        bitmap: &BITMAP,
        glyphs: &[
            GLYPHS[0],
            Glyph {
                y_min: -1,
                ..GLYPHS[2]
            },
        ],
    };

    let mut display = MockDisplay::<BinaryColor>::new();
    let text = DrawableText::new(&DESCENDER_FONT, "Ag", proportional(0));
    text.draw(&mut display).unwrap();

    display.assert_pattern(&[" #  ", "# # ", "### ", "# # ", "# ##"]);
}

#[test]
fn alignment_anchors_measure_box_for_all_nine_positions() {
    let alignments = [
        (Alignment::TOP_LEFT, Point::new(20, 30)),
        (Alignment::TOP_CENTER, Point::new(17, 30)),
        (Alignment::TOP_RIGHT, Point::new(13, 30)),
        (Alignment::MIDDLE_LEFT, Point::new(20, 28)),
        (Alignment::CENTER, Point::new(17, 28)),
        (Alignment::MIDDLE_RIGHT, Point::new(13, 28)),
        (Alignment::BOTTOM_LEFT, Point::new(20, 25)),
        (Alignment::BOTTOM_CENTER, Point::new(17, 25)),
        (Alignment::BOTTOM_RIGHT, Point::new(13, 25)),
    ];

    for (alignment, expected_top_left) in alignments {
        let text =
            DrawableText::new(&FONT, "AB", proportional(1).align(alignment)).at(Point::new(20, 30));

        assert_eq!(text.measure(), Size::new(7, 5));
        assert_eq!(text.bounding_box().top_left, expected_top_left);
    }
}
