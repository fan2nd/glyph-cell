use std::fmt::Write;

use crate::raster::BitmapGlyph;

pub(crate) fn font_expression(
    size: u16,
    ascii_width: u16,
    mut glyphs: Vec<BitmapGlyph>,
) -> syn::Result<proc_macro2::TokenStream> {
    glyphs.sort_by_key(|glyph| glyph.codepoint);
    let source = source(size, ascii_width, &glyphs).map_err(|err| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("failed to generate font data: {err}"),
        )
    })?;
    source.parse().map_err(|err| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("generated invalid Rust source: {err}"),
        )
    })
}

fn source(
    size: u16,
    ascii_width: u16,
    glyphs: &[BitmapGlyph],
) -> Result<String, Box<dyn std::error::Error>> {
    let mut bitmap = Vec::new();
    let metrics = glyphs
        .iter()
        .map(|glyph| {
            let offset = bitmap.len() as u32;
            pack_bpp1(&glyph.bitmap, &mut bitmap);
            (glyph, offset)
        })
        .collect::<Vec<_>>();

    let mut out = String::new();
    write_glyphs(&mut out, &metrics)?;
    write_bitmap(&mut out, &bitmap)?;
    write_font(&mut out, size, ascii_width, glyphs)?;
    Ok(out)
}

fn write_glyphs(out: &mut String, metrics: &[(&BitmapGlyph, u32)]) -> std::fmt::Result {
    writeln!(
        out,
        "const GLYPHS: [glyph_cell::Glyph; {}] = [",
        metrics.len()
    )?;
    for (glyph, bitmap_offset) in metrics {
        writeln!(out, "    glyph_cell::Glyph {{")?;
        writeln!(out, "        bitmap_offset: {bitmap_offset},")?;
        writeln!(out, "        width: {},", glyph.width)?;
        writeln!(out, "        height: {},", glyph.height)?;
        writeln!(out, "        cell_width: {},", glyph.cell_width)?;
        writeln!(out, "        x_offset: {},", glyph.x_offset)?;
        writeln!(out, "        y_offset: {},", glyph.y_offset)?;
        writeln!(out, "        x_min: {},", glyph.x_min)?;
        writeln!(out, "        y_min: {},", glyph.y_min)?;
        writeln!(out, "        advance_width: {},", glyph.advance_width)?;
        writeln!(out, "    }},")?;
    }
    writeln!(out, "];\n")
}

fn write_bitmap(out: &mut String, bitmap: &[u8]) -> std::fmt::Result {
    writeln!(out, "const BITMAP: [u8; {}] = [", bitmap.len())?;
    for byte in bitmap {
        writeln!(out, "    0b{byte:08b},")?;
    }
    writeln!(out, "];\n")
}

fn write_font(
    out: &mut String,
    size: u16,
    ascii_width: u16,
    glyphs: &[BitmapGlyph],
) -> std::fmt::Result {
    let index: String = glyphs.iter().map(|glyph| glyph.codepoint).collect();
    writeln!(out, "glyph_cell::FontData {{")?;
    writeln!(out, "    index: {:?},", index)?;
    writeln!(out, "    size: {},", size)?;
    writeln!(out, "    ascii_width: {},", ascii_width)?;
    writeln!(out, "    bitmap: &BITMAP,")?;
    writeln!(out, "    glyphs: &GLYPHS,")?;
    writeln!(out, "}}")
}

fn pack_bpp1(pixels: &[bool], out: &mut Vec<u8>) {
    let mut byte = 0u8;
    for (index, pixel) in pixels.iter().enumerate() {
        if *pixel {
            byte |= 1 << (7 - index % 8);
        }
        if index % 8 == 7 {
            out.push(byte);
            byte = 0;
        }
    }

    if !pixels.len().is_multiple_of(8) {
        out.push(byte);
    }
}
