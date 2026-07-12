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
    pub bpp: u8,
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
        let sample = self.glyph_sample(glyph, x, y);
        if sample == 0 {
            return false;
        }

        let bpp = self.normalized_bpp();
        let max = (1u16 << bpp) - 1;
        if sample as u16 >= max {
            return true;
        }

        const BAYER_4X4: [u8; 16] = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];
        let threshold = BAYER_4X4[(y as usize % 4) * 4 + x as usize % 4] as u16;
        sample as u16 * 16 > threshold * max
    }

    pub fn glyph_coverage(&self, glyph: &Glyph, x: u16, y: u16) -> u8 {
        let sample = self.glyph_sample(glyph, x, y) as u16;
        if sample == 0 {
            return 0;
        }

        let max = (1u16 << self.normalized_bpp()) - 1;
        ((sample * 255 + max / 2) / max) as u8
    }

    fn glyph_sample(&self, glyph: &Glyph, x: u16, y: u16) -> u8 {
        if x >= glyph.width || y >= glyph.height {
            return 0;
        }

        let bpp = self.normalized_bpp() as usize;
        let pixel_index = y as usize * glyph.width as usize + x as usize;
        let start_bit = glyph.bitmap_offset as usize * 8 + pixel_index * bpp;
        let mut sample = 0u8;

        for offset in 0..bpp {
            let bit_index = start_bit + offset;
            let byte = self.bitmap.get(bit_index / 8).copied().unwrap_or(0);
            let bit = 7 - (bit_index % 8);
            sample = (sample << 1) | ((byte >> bit) & 1);
        }

        sample
    }

    const fn normalized_bpp(&self) -> u8 {
        match self.bpp {
            1 | 2 | 3 | 4 | 8 => self.bpp,
            _ => 1,
        }
    }
}
