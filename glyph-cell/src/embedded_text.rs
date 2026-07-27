use embedded_graphics::text::{
    Baseline,
    renderer::{CharacterStyle, TextMetrics, TextRenderer},
};
use embedded_graphics_core::{
    draw_target::DrawTarget,
    geometry::{Point, Size},
    pixelcolor::PixelColor,
    primitives::Rectangle,
};

use crate::{FontData, Glyph, TextLayout, text::draw_glyph};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphCellTextStyle<'a, C: PixelColor> {
    pub font_data: &'a FontData<'a>,
    pub text_color: Option<C>,
    pub layout: TextLayout,
}

impl<'a, C: PixelColor> GlyphCellTextStyle<'a, C> {
    pub const fn new(font_data: &'a FontData<'a>, text_color: C) -> Self {
        Self {
            font_data,
            text_color: Some(text_color),
            layout: TextLayout::Proportional {
                spacing: 0,
                line_spacing: 0,
            },
        }
    }

    pub const fn transparent(font_data: &'a FontData<'a>) -> Self {
        Self {
            font_data,
            text_color: None,
            layout: TextLayout::Proportional {
                spacing: 0,
                line_spacing: 0,
            },
        }
    }

    pub const fn layout(mut self, layout: TextLayout) -> Self {
        self.layout = layout;
        self
    }

    pub const fn monospace(mut self) -> Self {
        self.layout = TextLayout::monospace();
        self
    }

    pub const fn monospace_with_spacing(mut self, spacing: i32, line_spacing: i32) -> Self {
        self.layout = TextLayout::monospace_with_spacing(spacing, line_spacing);
        self
    }

    pub const fn proportional(mut self, spacing: i32) -> Self {
        self.layout = TextLayout::proportional(spacing);
        self
    }

    pub const fn proportional_with_line_spacing(mut self, spacing: i32, line_spacing: i32) -> Self {
        self.layout = TextLayout::proportional_with_line_spacing(spacing, line_spacing);
        self
    }

    pub const fn text_color(mut self, text_color: C) -> Self {
        self.text_color = Some(text_color);
        self
    }

    pub const fn reset_text_color(mut self) -> Self {
        self.text_color = None;
        self
    }

    fn top_left_for_baseline(&self, position: Point, baseline: Baseline) -> Point {
        position - Point::new(0, self.baseline_offset(baseline))
    }

    fn baseline_offset(&self, baseline: Baseline) -> i32 {
        let em_bottom = self.font_data.size.saturating_sub(1) as i32;

        match baseline {
            Baseline::Top => 0,
            Baseline::Bottom | Baseline::Alphabetic => em_bottom,
            Baseline::Middle => em_bottom / 2,
        }
    }

    fn line_spacing(&self) -> i32 {
        match self.layout {
            TextLayout::Monospace { line_spacing, .. }
            | TextLayout::Proportional { line_spacing, .. } => line_spacing,
        }
    }

    fn text_metrics(&self, text: &str, position: Point, baseline: Baseline) -> TextMetrics {
        let top_left = self.top_left_for_baseline(position, baseline);
        let size = self.line_metrics(text).size();

        TextMetrics {
            bounding_box: Rectangle::new(top_left, size),
            next_position: position + Size::new(size.width, 0),
        }
    }

    fn line_metrics(&self, text: &str) -> LineMetrics {
        let mut metrics = LineMetrics::new(self.font_data.size as u32);
        let (spacing, _) = self.horizontal_layout();
        let mut pen_x = 0i32;
        let mut chars = text.chars().peekable();

        while let Some(ch) = chars.next() {
            let cell_width = self.cell_width(ch) as u32;
            metrics.include(pen_x, cell_width);
            pen_x = advance_pen(pen_x, cell_width as i32, spacing, chars.peek().is_some());
        }

        metrics
    }

    fn draw_line<D>(
        &self,
        text: &str,
        top_left: Point,
        target: &mut D,
        color: C,
    ) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = C>,
    {
        let metrics = self.line_metrics(text);
        let (spacing, use_generated_x_offset) = self.horizontal_layout();
        let line_left = top_left.x - metrics.min_x();
        let mut pen_x = 0i32;
        let mut chars = text.chars().peekable();

        while let Some(ch) = chars.next() {
            let cell_width = self.cell_width(ch);
            let cell_origin = Point::new(line_left + pen_x, top_left.y);

            if let Some(glyph) = self.font_data.glyph(ch) {
                let glyph_origin = self.glyph_origin(glyph, cell_origin, use_generated_x_offset);
                draw_glyph(target, self.font_data, glyph, glyph_origin, color)?;
            }

            pen_x = advance_pen(pen_x, cell_width as i32, spacing, chars.peek().is_some());
        }

        Ok(())
    }

    fn horizontal_layout(&self) -> (i32, bool) {
        match self.layout {
            TextLayout::Monospace { spacing, .. } => (spacing, true),
            TextLayout::Proportional { spacing, .. } => (spacing, false),
        }
    }

    fn cell_width(&self, ch: char) -> u16 {
        match self.layout {
            TextLayout::Monospace { .. } => self.font_data.cell_width(ch),
            TextLayout::Proportional { .. } => self
                .font_data
                .glyph(ch)
                .map(|glyph| glyph.width)
                .unwrap_or_else(|| self.font_data.cell_width(ch)),
        }
    }

    fn glyph_origin(
        &self,
        glyph: &Glyph,
        cell_origin: Point,
        use_generated_x_offset: bool,
    ) -> Point {
        let x_offset = if use_generated_x_offset {
            glyph.x_offset as i32
        } else {
            0
        };

        Point::new(
            cell_origin.x + x_offset,
            cell_origin.y + self.font_data.size as i32 - glyph.y_offset as i32,
        )
    }
}

impl<C> TextRenderer for GlyphCellTextStyle<'_, C>
where
    C: PixelColor,
{
    type Color = C;

    fn draw_string<D>(
        &self,
        text: &str,
        position: Point,
        baseline: Baseline,
        target: &mut D,
    ) -> Result<Point, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        let metrics = self.text_metrics(text, position, baseline);

        if let Some(text_color) = self.text_color {
            self.draw_line(text, metrics.bounding_box.top_left, target, text_color)?;
        }

        Ok(metrics.next_position)
    }

    fn draw_whitespace<D>(
        &self,
        width: u32,
        position: Point,
        _baseline: Baseline,
        _target: &mut D,
    ) -> Result<Point, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        Ok(position + Size::new(width, 0))
    }

    fn measure_string(&self, text: &str, position: Point, baseline: Baseline) -> TextMetrics {
        self.text_metrics(text, position, baseline)
    }

    fn line_height(&self) -> u32 {
        non_negative_u32(self.font_data.size as i32 + self.line_spacing())
    }
}

impl<C> CharacterStyle for GlyphCellTextStyle<'_, C>
where
    C: PixelColor,
{
    type Color = C;

    fn set_text_color(&mut self, text_color: Option<Self::Color>) {
        self.text_color = text_color;
    }
}

fn non_negative_u32(value: i32) -> u32 {
    if value <= 0 { 0 } else { value as u32 }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct LineMetrics {
    min_x: i32,
    max_x: i32,
    height: u32,
    has_chars: bool,
}

impl LineMetrics {
    const fn new(height: u32) -> Self {
        Self {
            min_x: 0,
            max_x: 0,
            height,
            has_chars: false,
        }
    }

    fn include(&mut self, pen_x: i32, width: u32) {
        let right = pen_x.saturating_add(width as i32);

        if !self.has_chars {
            self.min_x = pen_x;
            self.max_x = right;
            self.has_chars = true;
        } else {
            self.min_x = self.min_x.min(pen_x);
            self.max_x = self.max_x.max(right);
        }
    }

    fn size(self) -> Size {
        if self.has_chars {
            Size::new(non_negative_u32(self.max_x - self.min_x), self.height)
        } else {
            Size::zero()
        }
    }

    fn min_x(self) -> i32 {
        if self.has_chars { self.min_x } else { 0 }
    }
}

fn advance_pen(pen: i32, advance: i32, spacing: i32, has_next: bool) -> i32 {
    let step = if has_next {
        advance.saturating_add(spacing)
    } else {
        advance
    };
    pen.saturating_add(step)
}
