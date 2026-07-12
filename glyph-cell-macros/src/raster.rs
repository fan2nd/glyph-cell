use std::slice;

use freetype::freetype as ft;

use crate::source::{self, FreeTypeFont};

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
    pub bitmap: Vec<u8>,
}

pub(crate) fn rasterize_block(
    font: &FreeTypeFont,
    size: u16,
    chars: impl IntoIterator<Item = char>,
) -> syn::Result<Vec<BitmapGlyph>> {
    let chars = chars.into_iter().collect::<Vec<_>>();
    let ascii_size = fitting_ascii_size(font, size, chars.iter().copied())?;
    let mut glyphs: Vec<_> = chars
        .into_iter()
        .scan(None, |active_size, codepoint| {
            let glyph_size = if codepoint.is_ascii() {
                ascii_size
            } else {
                size
            };
            let result = set_active_pixel_size(font, active_size, glyph_size)
                .and_then(|()| rasterize_glyph(font, codepoint, size));
            Some(result)
        })
        .collect::<Result<_, _>>()?;
    apply_auto_y_offsets(size, &mut glyphs);
    Ok(glyphs)
}

fn fitting_ascii_size(
    font: &FreeTypeFont,
    raster_height: u16,
    chars: impl IntoIterator<Item = char>,
) -> syn::Result<u16> {
    let chars = chars
        .into_iter()
        .filter(|codepoint| codepoint.is_ascii())
        .collect::<Vec<_>>();
    if chars.is_empty() {
        return Ok(raster_height);
    }

    for glyph_size in (1..=raster_height).rev() {
        let mut active_size = None;
        set_active_pixel_size(font, &mut active_size, glyph_size)?;
        let glyphs = chars
            .iter()
            .copied()
            .map(|codepoint| rasterize_glyph(font, codepoint, raster_height))
            .collect::<syn::Result<Vec<_>>>()?;

        if glyphs_fit_vertically(raster_height, &glyphs) {
            return Ok(glyph_size);
        }
    }

    Ok(1)
}

fn set_active_pixel_size(
    font: &FreeTypeFont,
    active_size: &mut Option<u16>,
    size: u16,
) -> syn::Result<()> {
    if *active_size == Some(size) {
        return Ok(());
    }

    font.set_pixel_size(size)
        .map_err(|err| syn::Error::new(proc_macro2::Span::call_site(), err))?;
    *active_size = Some(size);
    Ok(())
}

fn rasterize_glyph(font: &FreeTypeFont, codepoint: char, size: u16) -> syn::Result<BitmapGlyph> {
    let face = font.face();
    let glyph_index = unsafe { ft::FT_Get_Char_Index(face, codepoint as ft::FT_ULong) };
    source::ft_ok(
        unsafe { ft::FT_Load_Glyph(face, glyph_index, glyph_load_flags()) },
        "load glyph",
    )
    .map_err(|err| syn::Error::new(proc_macro2::Span::call_site(), err))?;
    let slot = unsafe { (*face).glyph };
    let slot = unsafe { &*slot };
    let bitmap = &slot.bitmap;
    let raw_width = bitmap.width as usize;
    let raw_height = bitmap.rows as usize;
    let width = raw_width.max(1).min(u16::MAX as usize) as u16;
    let height = raw_height.max(1).min(u16::MAX as usize) as u16;
    let pixels = bitmap_pixels(bitmap, width, height);
    let y_offset = clamp_i32_to_i16(slot.bitmap_top);
    let y_min = clamp_i32_to_i16(slot.bitmap_top - raw_height as i32);

    Ok(BitmapGlyph {
        codepoint,
        width,
        height,
        cell_width: size,
        x_offset: 0,
        y_offset,
        x_min: clamp_i32_to_i16(slot.bitmap_left),
        y_min,
        advance_width: advance_width_pixels_16dot16(slot.linearHoriAdvance),
        bitmap: pixels,
    })
}

pub(crate) fn apply_cell_offsets(raster_height: u16, ascii_width: u16, glyphs: &mut [BitmapGlyph]) {
    for glyph in glyphs {
        glyph.cell_width = if glyph.codepoint.is_ascii() {
            ascii_width
        } else {
            raster_height
        };
        glyph.x_offset = centered_offset(glyph.cell_width, glyph.width);
        fit_glyph_to_cell(raster_height, glyph);
    }
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

fn glyphs_fit_vertically(raster_height: u16, glyphs: &[BitmapGlyph]) -> bool {
    let Some(first) = glyphs.first() else {
        return true;
    };

    let raster_height = raster_height as i32;
    let mut min_top = glyph_top(raster_height, first);
    let mut max_bottom = glyph_bottom(raster_height, first);

    for glyph in &glyphs[1..] {
        min_top = min_top.min(glyph_top(raster_height, glyph));
        max_bottom = max_bottom.max(glyph_bottom(raster_height, glyph));
    }

    max_bottom - min_top <= raster_height
}

fn bitmap_pixels(bitmap: &ft::FT_Bitmap, width: u16, height: u16) -> Vec<u8> {
    let width = width as usize;
    let height = height as usize;
    if bitmap.buffer.is_null() || bitmap.width == 0 || bitmap.rows == 0 {
        return vec![0; width * height];
    }

    let pitch = bitmap.pitch;
    let row_bytes = pitch.unsigned_abs() as usize;
    let buffer = unsafe { slice::from_raw_parts(bitmap.buffer, row_bytes * bitmap.rows as usize) };
    let mut pixels = vec![0; width * height];

    for y in 0..height.min(bitmap.rows as usize) {
        let source_y = if pitch >= 0 {
            y
        } else {
            bitmap.rows as usize - 1 - y
        };
        let row = source_y * row_bytes;
        for x in 0..width.min(bitmap.width as usize) {
            pixels[y * width + x] = match bitmap.pixel_mode as u32 {
                value if value == ft::FT_Pixel_Mode::FT_PIXEL_MODE_MONO as u32 => {
                    let byte = buffer[row + x / 8];
                    if byte & (0x80 >> (x % 8)) != 0 {
                        255
                    } else {
                        0
                    }
                }
                value if value == ft::FT_Pixel_Mode::FT_PIXEL_MODE_GRAY as u32 => buffer[row + x],
                _ => 0,
            };
        }
    }

    pixels
}

fn fit_glyph_to_cell(raster_height: u16, glyph: &mut BitmapGlyph) {
    let cell_width = glyph.cell_width as i32;
    let cell_height = raster_height as i32;
    let left = glyph.x_offset as i32;
    let top = cell_height - glyph.y_offset as i32;
    let right = left + glyph.width as i32;
    let bottom = top + glyph.height as i32;

    let visible_left = left.clamp(0, cell_width);
    let visible_top = top.clamp(0, cell_height);
    let visible_right = right.clamp(visible_left, cell_width);
    let visible_bottom = bottom.clamp(visible_top, cell_height);

    let new_width = (visible_right - visible_left) as u16;
    let new_height = (visible_bottom - visible_top) as u16;
    let source_x = (visible_left - left).max(0) as usize;
    let source_y = (visible_top - top).max(0) as usize;
    let old_width = glyph.width as usize;
    let old_height = glyph.height as usize;

    if source_x == 0
        && source_y == 0
        && new_width as usize == old_width
        && new_height as usize == old_height
    {
        return;
    }

    let mut clipped = vec![0; new_width as usize * new_height as usize];
    for y in 0..new_height as usize {
        for x in 0..new_width as usize {
            clipped[y * new_width as usize + x] =
                glyph.bitmap[(source_y + y) * old_width + source_x + x];
        }
    }

    let cropped_bottom = (bottom - visible_bottom).max(0);
    glyph.width = new_width;
    glyph.height = new_height;
    glyph.x_offset = clamp_i32_to_i16(visible_left);
    glyph.y_offset = clamp_i32_to_i16(cell_height - visible_top);
    glyph.x_min = offset_i16_i32(glyph.x_min, source_x as i32);
    glyph.y_min = offset_i16_i32(glyph.y_min, cropped_bottom);
    glyph.bitmap = clipped;
}

fn advance_width_pixels_16dot16(advance_width: ft::FT_Fixed) -> u16 {
    if advance_width <= 0 {
        0
    } else {
        ((advance_width as i64 + 65535) / 65536).min(u16::MAX as i64) as u16
    }
}

fn ft_load_target_mono() -> i32 {
    2 << 16
}

fn glyph_load_flags() -> i32 {
    ft::FT_LOAD_RENDER as i32
        | ft::FT_LOAD_FORCE_AUTOHINT as i32
        | ft_load_target_mono()
        | ft::FT_LOAD_MONOCHROME as i32
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
        BitmapGlyph, advance_width_pixels_16dot16, apply_auto_y_offsets, apply_cell_offsets,
        centered_offset, glyphs_fit_vertically, y_offset_delta,
    };

    #[test]
    fn rounds_16dot16_advance_up_to_pixels() {
        assert_eq!(advance_width_pixels_16dot16(0), 0);
        assert_eq!(advance_width_pixels_16dot16(4 * 65536), 4);
        assert_eq!(advance_width_pixels_16dot16(4 * 65536 + 1), 5);
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

    #[test]
    fn clips_glyph_bitmap_to_generated_cell() {
        let mut glyphs = [glyph_for_codepoint('A', 8, 8, 7)];
        glyphs[0].x_offset = -2;
        glyphs[0].bitmap = (0..64)
            .map(|index| if index % 2 == 0 { 255 } else { 0 })
            .collect();

        apply_cell_offsets(6, 4, &mut glyphs);

        assert_eq!(glyphs[0].width, 4);
        assert_eq!(glyphs[0].height, 6);
        assert_eq!(glyphs[0].x_offset, 0);
        assert_eq!(glyphs[0].y_offset, 6);
        assert_eq!(glyphs[0].bitmap.len(), 24);
    }

    #[test]
    fn vertical_fit_detects_top_and_bottom_overflow() {
        assert!(!glyphs_fit_vertically(
            6,
            &[glyph_for_codepoint('A', 3, 8, 7)]
        ));
        assert!(!glyphs_fit_vertically(
            6,
            &[glyph_for_codepoint('A', 3, 8, 1)]
        ));
    }

    #[test]
    fn vertical_fit_ignores_horizontal_overflow() {
        assert!(glyphs_fit_vertically(
            6,
            &[glyph_for_codepoint('A', 99, 4, 5)]
        ));
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
            bitmap: vec![255; width as usize * height as usize],
        }
    }
}
