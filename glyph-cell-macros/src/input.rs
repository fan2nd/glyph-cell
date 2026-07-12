use syn::{
    Ident, LitChar, LitInt, LitStr, Result, Token, braced,
    parse::{Parse, ParseStream},
};

pub(crate) struct FontDataInput {
    pub size: LitInt,
    pub ascii_width: Option<LitInt>,
    pub bpp: Option<LitInt>,
    pub blocks: Vec<FontBlock>,
}

pub(crate) struct FontBlock {
    pub path: LitStr,
    pub index: LitStr,
    pub y_offsets: Vec<GlyphYOffset>,
}

pub(crate) struct GlyphYOffset {
    pub codepoint: LitChar,
    pub delta: i16,
}

impl Parse for FontDataInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut size = None;
        let mut ascii_width = None;
        let mut bpp = None;
        let mut blocks = Vec::new();
        let mut current = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![:]>()?;

            match key.to_string().as_str() {
                "size" | "height" => parse_size(&mut size, key, input)?,
                "ascii_width" => ascii_width = Some(input.parse()?),
                "bpp" => bpp = Some(input.parse()?),
                "path" => {
                    finish_block(&mut blocks, current.take());
                    current = Some(FontBlock {
                        path: input.parse()?,
                        index: LitStr::new("", key.span()),
                        y_offsets: Vec::new(),
                    });
                }
                "index" => parse_index(&mut current, key, input)?,
                "y_offset" | "y_offsets" | "yoffset" => parse_y_offsets(&mut current, key, input)?,
                _ => {
                    return Err(syn::Error::new(
                        key.span(),
                        "expected size, height, ascii_width, bpp, path, index, or y_offset",
                    ));
                }
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        finish_block(&mut blocks, current.take());
        validate(size, ascii_width, bpp, blocks, input)
    }
}

fn parse_size(size: &mut Option<LitInt>, key: Ident, input: ParseStream<'_>) -> Result<()> {
    if size.is_some() {
        return Err(syn::Error::new(key.span(), "duplicate size/height"));
    }

    *size = Some(input.parse()?);
    Ok(())
}

fn parse_index(current: &mut Option<FontBlock>, key: Ident, input: ParseStream<'_>) -> Result<()> {
    let Some(block) = current.as_mut() else {
        return Err(syn::Error::new(key.span(), "index must follow path"));
    };

    if !block.index.value().is_empty() {
        return Err(syn::Error::new(
            key.span(),
            "duplicate index for font block",
        ));
    }

    block.index = input.parse()?;
    Ok(())
}

fn parse_y_offsets(
    current: &mut Option<FontBlock>,
    key: Ident,
    input: ParseStream<'_>,
) -> Result<()> {
    let Some(block) = current.as_mut() else {
        return Err(syn::Error::new(key.span(), "y_offset must follow path"));
    };

    if !block.y_offsets.is_empty() {
        return Err(syn::Error::new(
            key.span(),
            "duplicate y_offset for font block",
        ));
    }

    let content;
    braced!(content in input);

    while !content.is_empty() {
        let codepoint: LitChar = content.parse()?;
        content.parse::<Token![:]>()?;
        let delta = parse_i16(&content)?;

        if block
            .y_offsets
            .iter()
            .any(|adjustment| adjustment.codepoint.value() == codepoint.value())
        {
            return Err(syn::Error::new(
                codepoint.span(),
                format!("duplicate y_offset for character {:?}", codepoint.value()),
            ));
        }

        block.y_offsets.push(GlyphYOffset { codepoint, delta });

        if content.peek(Token![,]) {
            content.parse::<Token![,]>()?;
        }
    }

    Ok(())
}

fn parse_i16(input: ParseStream<'_>) -> Result<i16> {
    let negative = input.peek(Token![-]);
    if negative {
        input.parse::<Token![-]>()?;
    }
    let value: LitInt = input.parse()?;
    let magnitude = value.base10_parse::<i32>()?;
    let signed = if negative { -magnitude } else { magnitude };

    i16::try_from(signed)
        .map_err(|_| syn::Error::new(value.span(), "y_offset value must fit in i16"))
}

fn finish_block(blocks: &mut Vec<FontBlock>, block: Option<FontBlock>) {
    if let Some(block) = block {
        blocks.push(block);
    }
}

fn validate(
    size: Option<LitInt>,
    ascii_width: Option<LitInt>,
    bpp: Option<LitInt>,
    blocks: Vec<FontBlock>,
    input: ParseStream<'_>,
) -> Result<FontDataInput> {
    if blocks.is_empty() {
        return Err(input.error("missing font block"));
    }

    for block in &blocks {
        if block.index.value().is_empty() {
            return Err(syn::Error::new(
                block.path.span(),
                "missing index for font block",
            ));
        }

        for adjustment in &block.y_offsets {
            let codepoint = adjustment.codepoint.value();
            if !block.index.value().contains(codepoint) {
                return Err(syn::Error::new(
                    adjustment.codepoint.span(),
                    format!("y_offset character {codepoint:?} is not in this font block index"),
                ));
            }
        }
    }

    Ok(FontDataInput {
        size: size.ok_or_else(|| input.error("missing size"))?,
        ascii_width,
        bpp,
        blocks,
    })
}

#[cfg(test)]
mod tests {
    use super::FontDataInput;

    #[test]
    fn parses_y_offset_tweaks_inside_font_block() {
        let input: FontDataInput = syn::parse_str(
            r#"
            size: 18,
            ascii_width: 9,
            bpp: 4,
            path: "font.ttf",
            index: "Ag",
            y_offset: {
                'g': -1,
            },
            "#,
        )
        .unwrap();

        assert_eq!(input.ascii_width.unwrap().base10_parse::<u16>().unwrap(), 9);
        assert_eq!(input.bpp.unwrap().base10_parse::<u8>().unwrap(), 4);
        assert_eq!(input.blocks.len(), 1);
        assert_eq!(input.blocks[0].y_offsets.len(), 1);
        assert_eq!(input.blocks[0].y_offsets[0].codepoint.value(), 'g');
        assert_eq!(input.blocks[0].y_offsets[0].delta, -1);
    }

    #[test]
    fn parses_height_as_size_alias() {
        let input: FontDataInput = syn::parse_str(
            r#"
            height: 18,
            path: "font.ttf",
            index: "A",
            "#,
        )
        .unwrap();

        assert_eq!(input.size.base10_parse::<u16>().unwrap(), 18);
    }

    #[test]
    fn rejects_duplicate_size_and_height() {
        let result = syn::parse_str::<FontDataInput>(
            r#"
            size: 18,
            height: 18,
            path: "font.ttf",
            index: "A",
            "#,
        );
        let err = match result {
            Ok(_) => panic!("expected duplicate size/height error"),
            Err(err) => err,
        };

        assert!(err.to_string().contains("duplicate size/height"));
    }
}
