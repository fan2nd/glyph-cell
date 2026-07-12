use fontdue::Font;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BitmapGlyph {
    pub codepoint: char,
    pub width: u16,
    pub height: u16,
    pub cell_width: u16,
    pub x_offset: i16,
    pub y_offset: i16,
    pub x_min: i16,
    pub y_min: i16,
    pub advance_width: u16,
    pub bitmap: Vec<bool>,
}

pub(crate) fn rasterize_block(
    font: &Font,
    size: u16,
    chars: impl IntoIterator<Item = char>,
) -> Vec<BitmapGlyph> {
    let mut glyphs: Vec<_> = chars
        .into_iter()
        .map(|codepoint| rasterize_glyph(font, codepoint, size))
        .collect();
    apply_auto_y_offsets(size, &mut glyphs);
    glyphs
}

fn rasterize_glyph(font: &Font, codepoint: char, size: u16) -> BitmapGlyph {
    let (metrics, bitmap) = font.rasterize(codepoint, size as f32);
    let width = metrics.width.max(1) as u16;
    let height = metrics.height.max(1) as u16;
    let pixels = if metrics.width == 0 || metrics.height == 0 {
        vec![false; width as usize * height as usize]
    } else {
        bitmap.into_iter().map(|alpha| alpha >= 96).collect()
    };

    BitmapGlyph {
        codepoint,
        width,
        height,
        cell_width: size,
        x_offset: 0,
        y_offset: glyph_y_offset(metrics.height, metrics.ymin),
        x_min: clamp_i32_to_i16(metrics.xmin),
        y_min: clamp_i32_to_i16(metrics.ymin),
        advance_width: advance_width_pixels(metrics.advance_width),
        bitmap: pixels,
    }
}

pub(crate) fn apply_cell_offsets(raster_height: u16, ascii_width: u16, glyphs: &mut [BitmapGlyph]) {
    for glyph in glyphs {
        glyph.cell_width = if glyph.codepoint.is_ascii() {
            ascii_width
        } else {
            raster_height
        };
        glyph.x_offset = centered_offset(glyph.cell_width, glyph.width);
    }
}

fn glyph_y_offset(height: usize, y_min: i32) -> i16 {
    clamp_i32_to_i16(height as i32 + y_min)
}

fn centered_offset(outer: u16, inner: u16) -> i16 {
    clamp_i32_to_i16((outer as i32 - inner as i32) / 2)
}

fn apply_auto_y_offsets(raster_height: u16, glyphs: &mut [BitmapGlyph]) {
    for ascii in [true, false] {
        let delta = y_offset_delta_for(raster_height, glyphs, ascii);
        for glyph in glyphs
            .iter_mut()
            .filter(|glyph| glyph.codepoint.is_ascii() == ascii)
        {
            glyph.y_offset = offset_i16_i32(glyph.y_offset, delta);
        }
    }
}

#[cfg(test)]
fn y_offset_delta(raster_height: u16, glyphs: &[BitmapGlyph]) -> i32 {
    y_offset_delta_from_iter(raster_height, glyphs.iter())
}

fn y_offset_delta_for(raster_height: u16, glyphs: &[BitmapGlyph], ascii: bool) -> i32 {
    y_offset_delta_from_iter(
        raster_height,
        glyphs
            .iter()
            .filter(|glyph| glyph.codepoint.is_ascii() == ascii),
    )
}

fn y_offset_delta_from_iter<'a>(
    raster_height: u16,
    mut glyphs: impl Iterator<Item = &'a BitmapGlyph>,
) -> i32 {
    let Some(first) = glyphs.next() else {
        return 0;
    };

    let raster_height = raster_height as i32;
    let mut min_top = glyph_top(raster_height, first);
    let mut max_bottom = glyph_bottom(raster_height, first);

    for glyph in glyphs {
        min_top = min_top.min(glyph_top(raster_height, glyph));
        max_bottom = max_bottom.max(glyph_bottom(raster_height, glyph));
    }

    let min_delta = max_bottom - raster_height;
    let max_delta = min_top;

    if min_delta <= max_delta {
        0.clamp(min_delta, max_delta)
    } else {
        (min_top + max_bottom - raster_height) / 2
    }
}

fn glyph_top(raster_height: i32, glyph: &BitmapGlyph) -> i32 {
    raster_height - glyph.y_offset as i32
}

fn glyph_bottom(raster_height: i32, glyph: &BitmapGlyph) -> i32 {
    glyph_top(raster_height, glyph) + glyph.height as i32
}

fn advance_width_pixels(advance_width: f32) -> u16 {
    if !advance_width.is_finite() || advance_width <= 0.0 {
        0
    } else if advance_width >= u16::MAX as f32 {
        u16::MAX
    } else {
        advance_width.ceil() as u16
    }
}

pub(crate) fn offset_i16(value: i16, delta: i16) -> i16 {
    offset_i16_i32(value, delta as i32)
}

fn offset_i16_i32(value: i16, delta: i32) -> i16 {
    clamp_i32_to_i16(value as i32 + delta)
}

fn clamp_i32_to_i16(value: i32) -> i16 {
    value.clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

#[cfg(test)]
mod tests {
    use super::{
        BitmapGlyph, advance_width_pixels, apply_auto_y_offsets, apply_cell_offsets,
        centered_offset, y_offset_delta,
    };

    #[test]
    fn rounds_fractional_advance_up_to_pixels() {
        assert_eq!(advance_width_pixels(0.0), 0);
        assert_eq!(advance_width_pixels(4.0), 4);
        assert_eq!(advance_width_pixels(4.1), 5);
    }

    #[test]
    fn centers_glyph_bitmap_in_cell() {
        assert_eq!(centered_offset(24, 10), 7);
        assert_eq!(centered_offset(24, 24), 0);
        assert_eq!(centered_offset(24, 28), -2);
    }

    #[test]
    fn keeps_glyphs_that_already_fit_in_raster_height() {
        assert_eq!(y_offset_delta(12, &[glyph(5, 10), glyph(4, 8)]), 0);
    }

    #[test]
    fn moves_font_block_down_until_top_fits() {
        assert_eq!(y_offset_delta(12, &[glyph(5, 14), glyph(4, 12)]), -2);
    }

    #[test]
    fn moves_font_block_up_until_bottom_fits() {
        assert_eq!(y_offset_delta(12, &[glyph(5, 3), glyph(4, 4)]), 2);
    }

    #[test]
    fn auto_adjusts_generated_y_offset_without_changing_raw_y_min() {
        let mut glyphs = [glyph(5, 14), glyph(4, 12)];
        let y_min = glyphs[0].y_min;

        apply_auto_y_offsets(12, &mut glyphs);

        assert_eq!(glyphs[0].y_offset, 12);
        assert_eq!(glyphs[0].y_min, y_min);
    }

    #[test]
    fn auto_adjusts_ascii_and_other_y_offsets_independently() {
        let mut glyphs = [
            glyph_for_codepoint('A', 1, 5, 14),
            glyph_for_codepoint('你', 1, 5, 3),
        ];

        apply_auto_y_offsets(12, &mut glyphs);

        assert_eq!(glyphs[0].y_offset, 12);
        assert_eq!(glyphs[1].y_offset, 5);
    }

    #[test]
    fn applies_ascii_and_other_cell_offsets_after_y_min_adjustment() {
        let mut glyphs = [
            glyph_for_codepoint('A', 3, 3, 3),
            glyph_for_codepoint('你', 6, 5, 5),
        ];

        apply_cell_offsets(12, 6, &mut glyphs);

        assert_eq!(glyphs[0].cell_width, 6);
        assert_eq!(glyphs[0].x_offset, 1);
        assert_eq!(glyphs[0].y_offset, 3);
        assert_eq!(glyphs[1].cell_width, 12);
        assert_eq!(glyphs[1].x_offset, 3);
        assert_eq!(glyphs[1].y_offset, 5);
    }

    fn glyph(height: u16, y_offset: i16) -> BitmapGlyph {
        glyph_for_codepoint('A', 1, height, y_offset)
    }

    fn glyph_for_codepoint(codepoint: char, width: u16, height: u16, y_offset: i16) -> BitmapGlyph {
        BitmapGlyph {
            codepoint,
            width,
            height,
            cell_width: 1,
            x_offset: 0,
            y_offset,
            x_min: 0,
            y_min: y_offset - height as i16,
            advance_width: 1,
            bitmap: vec![true; height as usize],
        }
    }
}
