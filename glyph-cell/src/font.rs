#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glyph {
    pub bitmap_offset: u32,
    pub width: u16,
    pub height: u16,
    pub cell_width: u16,
    pub x_offset: i16,
    pub y_offset: i16,
    pub x_min: i16,
    pub y_min: i16,
    pub advance_width: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontData<'a> {
    pub index: &'a str,
    pub size: u16,
    pub ascii_width: u16,
    pub bitmap: &'a [u8],
    pub glyphs: &'a [Glyph],
}

impl<'a> FontData<'a> {
    pub fn glyph(&self, ch: char) -> Option<&Glyph> {
        self.index
            .chars()
            .position(|candidate| candidate == ch)
            .and_then(|index| self.glyphs.get(index))
    }

    pub fn cell_width(&self, ch: char) -> u16 {
        self.glyph(ch)
            .map(|glyph| glyph.cell_width)
            .unwrap_or_else(|| {
                if ch.is_ascii() {
                    self.ascii_width
                } else {
                    self.size
                }
            })
    }

    pub fn glyph_pixel(&self, glyph: &Glyph, x: u16, y: u16) -> bool {
        if x >= glyph.width || y >= glyph.height {
            return false;
        }

        let pixel_index = y as usize * glyph.width as usize + x as usize;
        let bit_index = glyph.bitmap_offset as usize * 8 + pixel_index;
        let byte = self.bitmap.get(bit_index / 8).copied().unwrap_or(0);
        let bit = 7 - (bit_index % 8);
        byte & (1 << bit) != 0
    }
}
