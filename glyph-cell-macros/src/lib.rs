mod emit;
mod input;
mod raster;
mod source;

use std::collections::BTreeMap;

use input::{FontBlock, FontDataInput};
use proc_macro::TokenStream;
use quote::quote;
use syn::{LitStr, parse_macro_input};

#[proc_macro]
pub fn font_data(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as FontDataInput);
    expand(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

fn expand(input: FontDataInput) -> syn::Result<proc_macro2::TokenStream> {
    let size = input.size.base10_parse::<u16>()?;
    let ascii_width = input
        .ascii_width
        .as_ref()
        .map(|value| value.base10_parse::<u16>())
        .transpose()?
        .unwrap_or(size);
    let bpp = input
        .bpp
        .as_ref()
        .map(|value| value.base10_parse::<u8>())
        .transpose()?
        .unwrap_or(1);
    if !matches!(bpp, 1 | 2 | 3 | 4 | 8) {
        return Err(syn::Error::new(
            input
                .bpp
                .as_ref()
                .map_or_else(proc_macro2::Span::call_site, |value| value.span()),
            "bpp must be 1, 2, 3, 4, or 8",
        ));
    }
    let blocks = indexed_blocks(input.blocks)?;
    let mut glyphs = Vec::new();

    for (block, chars) in blocks {
        let font = source::load_font(&block.path)?;
        let mut block_glyphs = raster::rasterize_block(&font, size, bpp, chars)?;
        apply_y_offsets(&mut block_glyphs, &block)?;
        raster::apply_cell_offsets(size, ascii_width, &mut block_glyphs);
        glyphs.extend(block_glyphs);
    }

    let generated = emit::font_expression(size, ascii_width, bpp, glyphs)?;
    Ok(quote! {{ #generated }})
}

fn apply_y_offsets(glyphs: &mut [raster::BitmapGlyph], block: &FontBlock) -> syn::Result<()> {
    for adjustment in &block.y_offsets {
        let codepoint = adjustment.codepoint.value();
        let Some(glyph) = glyphs.iter_mut().find(|glyph| glyph.codepoint == codepoint) else {
            return Err(syn::Error::new(
                adjustment.codepoint.span(),
                format!("y_offset character {codepoint:?} is not in this font block index"),
            ));
        };

        glyph.y_offset = raster::offset_i16(glyph.y_offset, adjustment.delta);
    }

    Ok(())
}

fn indexed_blocks(blocks: Vec<FontBlock>) -> syn::Result<Vec<(FontBlock, Vec<char>)>> {
    let mut seen = BTreeMap::<char, LitStr>::new();
    blocks
        .into_iter()
        .map(|block| {
            let chars = unique_block_chars(&block.index, &mut seen)?;
            Ok((block, chars))
        })
        .collect()
}

fn unique_block_chars(index: &LitStr, seen: &mut BTreeMap<char, LitStr>) -> syn::Result<Vec<char>> {
    let mut chars = Vec::new();

    for ch in index.value().chars() {
        if chars.contains(&ch) {
            continue;
        }

        if let Some(first_index) = seen.get(&ch) {
            return Err(syn::Error::new(
                index.span(),
                format!(
                    "duplicate index character {ch:?}; first seen in index {:?}",
                    first_index.value()
                ),
            ));
        }

        seen.insert(ch, index.clone());
        chars.push(ch);
    }

    Ok(chars)
}
