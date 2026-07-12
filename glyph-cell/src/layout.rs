use embedded_graphics_core::geometry::{Point, Size};

use crate::{Alignment, FontData, Glyph, TextLayout};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextFlow {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PositionedGlyph {
    pub ch: char,
    pub glyph: Option<Glyph>,
    pub cell_origin: Point,
    pub cell_size: Size,
    pub glyph_origin: Option<Point>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TextRun<'a> {
    font: &'a FontData<'a>,
    text: &'a str,
    start: Point,
    layout: TextLayout,
    alignment: Alignment,
    flow: TextFlow,
}

impl<'a> TextRun<'a> {
    pub(crate) const fn new(
        font: &'a FontData<'a>,
        text: &'a str,
        start: Point,
        layout: TextLayout,
        alignment: Alignment,
        flow: TextFlow,
    ) -> Self {
        Self {
            font,
            text,
            start,
            layout,
            alignment,
            flow,
        }
    }

    pub(crate) fn measure(&self) -> Size {
        match (self.flow, self.layout) {
            (
                TextFlow::Horizontal,
                TextLayout::Monospace {
                    spacing,
                    line_spacing,
                },
            ) => self.measure_monospace_horizontal(spacing, line_spacing),
            (
                TextFlow::Vertical,
                TextLayout::Monospace {
                    spacing,
                    line_spacing,
                },
            ) => self.measure_monospace_vertical(spacing, line_spacing),
            (
                TextFlow::Horizontal,
                TextLayout::Proportional {
                    spacing,
                    line_spacing,
                },
            ) => self.measure_proportional_horizontal(spacing, line_spacing),
            (
                TextFlow::Vertical,
                TextLayout::Proportional {
                    spacing,
                    line_spacing,
                },
            ) => self.measure_proportional_vertical(spacing, line_spacing),
        }
    }

    pub(crate) fn top_left(&self) -> Point {
        let measure = self.measure();
        let (x, y) = self.alignment.offset(measure, Size::zero());
        self.start - Point::new(x, y)
    }

    pub(crate) fn for_each_positioned_glyph<E>(
        &self,
        mut visit: impl FnMut(PositionedGlyph) -> Result<(), E>,
    ) -> Result<(), E> {
        match (self.flow, self.layout) {
            (
                TextFlow::Horizontal,
                TextLayout::Monospace {
                    spacing,
                    line_spacing,
                },
            ) => self.for_each_monospace_horizontal(spacing, line_spacing, &mut visit),
            (
                TextFlow::Vertical,
                TextLayout::Monospace {
                    spacing,
                    line_spacing,
                },
            ) => self.for_each_monospace_vertical(spacing, line_spacing, &mut visit),
            (
                TextFlow::Horizontal,
                TextLayout::Proportional {
                    spacing,
                    line_spacing,
                },
            ) => self.for_each_proportional_horizontal(spacing, line_spacing, &mut visit),
            (
                TextFlow::Vertical,
                TextLayout::Proportional {
                    spacing,
                    line_spacing,
                },
            ) => self.for_each_proportional_vertical(spacing, line_spacing, &mut visit),
        }
    }

    fn measure_monospace_horizontal(&self, spacing: i32, line_spacing: i32) -> Size {
        self.measure_horizontal(spacing, line_spacing, Self::monospace_cell)
    }

    fn measure_monospace_vertical(&self, spacing: i32, line_spacing: i32) -> Size {
        self.measure_vertical(spacing, line_spacing, Self::monospace_cell)
    }

    fn measure_proportional_horizontal(&self, spacing: i32, line_spacing: i32) -> Size {
        self.measure_horizontal(spacing, line_spacing, Self::proportional_cell)
    }

    fn measure_proportional_vertical(&self, spacing: i32, line_spacing: i32) -> Size {
        self.measure_vertical(spacing, line_spacing, Self::proportional_cell)
    }

    fn measure_horizontal(
        &self,
        spacing: i32,
        line_spacing: i32,
        cell_for_char: fn(&Self, char) -> Size,
    ) -> Size {
        if self.text.is_empty() {
            return Size::zero();
        }

        let mut max_width = 0u32;
        let mut total_height = 0i32;
        let mut lines = self.text.split('\n').peekable();

        while let Some(line) = lines.next() {
            let line = self.measure_line(line, spacing, cell_for_char);
            max_width = max_width.max(line.width());
            total_height = advance_pen(
                total_height,
                line.height as i32,
                line_spacing,
                lines.peek().is_some(),
            );
        }

        Size::new(max_width, non_negative_u32(total_height))
    }

    fn measure_vertical(
        &self,
        spacing: i32,
        line_spacing: i32,
        cell_for_char: fn(&Self, char) -> Size,
    ) -> Size {
        if self.text.is_empty() {
            return Size::zero();
        }

        let mut total_width = 0i32;
        let mut max_height = 0u32;
        let mut columns = self.text.split('\n').peekable();

        while let Some(column) = columns.next() {
            let column = self.measure_column(column, spacing, cell_for_char);
            total_width = advance_pen(
                total_width,
                column.width as i32,
                line_spacing,
                columns.peek().is_some(),
            );
            max_height = max_height.max(column.height());
        }

        Size::new(non_negative_u32(total_width), max_height)
    }

    fn for_each_monospace_horizontal<E>(
        &self,
        spacing: i32,
        line_spacing: i32,
        visit: &mut impl FnMut(PositionedGlyph) -> Result<(), E>,
    ) -> Result<(), E> {
        self.for_each_horizontal(spacing, line_spacing, Self::monospace_cell, true, visit)
    }

    fn for_each_monospace_vertical<E>(
        &self,
        spacing: i32,
        line_spacing: i32,
        visit: &mut impl FnMut(PositionedGlyph) -> Result<(), E>,
    ) -> Result<(), E> {
        self.for_each_vertical(spacing, line_spacing, Self::monospace_cell, true, visit)
    }

    fn for_each_proportional_horizontal<E>(
        &self,
        spacing: i32,
        line_spacing: i32,
        visit: &mut impl FnMut(PositionedGlyph) -> Result<(), E>,
    ) -> Result<(), E> {
        self.for_each_horizontal(spacing, line_spacing, Self::proportional_cell, false, visit)
    }

    fn for_each_proportional_vertical<E>(
        &self,
        spacing: i32,
        line_spacing: i32,
        visit: &mut impl FnMut(PositionedGlyph) -> Result<(), E>,
    ) -> Result<(), E> {
        self.for_each_vertical(spacing, line_spacing, Self::proportional_cell, false, visit)
    }

    fn for_each_horizontal<E>(
        &self,
        spacing: i32,
        line_spacing: i32,
        cell_for_char: fn(&Self, char) -> Size,
        use_generated_x_offset: bool,
        visit: &mut impl FnMut(PositionedGlyph) -> Result<(), E>,
    ) -> Result<(), E> {
        let measure = self.measure_horizontal(spacing, line_spacing, cell_for_char);
        let top_left = self.top_left();
        let mut pen_y = top_left.y;
        let mut lines = self.text.split('\n').peekable();

        while let Some(line) = lines.next() {
            let line_metrics = self.measure_line(line, spacing, cell_for_char);
            let line_left = top_left.x
                + self
                    .alignment
                    .horizontal
                    .offset(measure.width, line_metrics.width())
                - line_metrics.min_x();
            let mut pen_x = 0i32;
            let mut chars = line.chars().peekable();

            while let Some(ch) = chars.next() {
                let cell = cell_for_char(self, ch);
                let cell_origin = Point::new(line_left + pen_x, pen_y);
                visit(self.positioned_in_cell(ch, cell_origin, cell, use_generated_x_offset))?;
                pen_x = advance_pen(pen_x, cell.width as i32, spacing, chars.peek().is_some());
            }

            pen_y = advance_pen(
                pen_y,
                line_metrics.height as i32,
                line_spacing,
                lines.peek().is_some(),
            );
        }

        Ok(())
    }

    fn for_each_vertical<E>(
        &self,
        spacing: i32,
        line_spacing: i32,
        cell_for_char: fn(&Self, char) -> Size,
        use_generated_x_offset: bool,
        visit: &mut impl FnMut(PositionedGlyph) -> Result<(), E>,
    ) -> Result<(), E> {
        let measure = self.measure_vertical(spacing, line_spacing, cell_for_char);
        let top_left = self.top_left();
        let mut pen_x = top_left.x;
        let mut columns = self.text.split('\n').peekable();

        while let Some(column) = columns.next() {
            let column_metrics = self.measure_column(column, spacing, cell_for_char);
            let column_top = top_left.y
                + self
                    .alignment
                    .vertical
                    .offset(measure.height, column_metrics.height())
                - column_metrics.min_y();
            let mut pen_y = 0i32;
            let mut chars = column.chars().peekable();

            while let Some(ch) = chars.next() {
                let cell = cell_for_char(self, ch);
                let cell_x = pen_x
                    + self
                        .alignment
                        .horizontal
                        .offset(column_metrics.width, cell.width);
                let cell_origin = Point::new(cell_x, column_top + pen_y);
                visit(self.positioned_in_cell(ch, cell_origin, cell, use_generated_x_offset))?;
                pen_y = advance_pen(pen_y, cell.height as i32, spacing, chars.peek().is_some());
            }

            pen_x = advance_pen(
                pen_x,
                column_metrics.width as i32,
                line_spacing,
                columns.peek().is_some(),
            );
        }

        Ok(())
    }

    fn measure_line(
        &self,
        line: &str,
        spacing: i32,
        cell_for_char: fn(&Self, char) -> Size,
    ) -> LineMetrics {
        let mut metrics = LineMetrics::new(self.font.size as u32);
        let mut pen_x = 0i32;
        let mut chars = line.chars().peekable();

        while let Some(ch) = chars.next() {
            let cell = cell_for_char(self, ch);
            metrics.include(pen_x, cell.width);
            pen_x = advance_pen(pen_x, cell.width as i32, spacing, chars.peek().is_some());
        }

        metrics
    }

    fn measure_column(
        &self,
        column: &str,
        spacing: i32,
        cell_for_char: fn(&Self, char) -> Size,
    ) -> ColumnMetrics {
        let mut metrics = ColumnMetrics::default();
        let mut pen_y = 0i32;
        let mut chars = column.chars().peekable();

        while let Some(ch) = chars.next() {
            let cell = cell_for_char(self, ch);
            metrics.include(pen_y, cell);
            pen_y = advance_pen(pen_y, cell.height as i32, spacing, chars.peek().is_some());
        }

        metrics
    }

    fn monospace_cell(&self, ch: char) -> Size {
        Size::new(self.font.cell_width(ch) as u32, self.font.size as u32)
    }

    fn proportional_cell(&self, ch: char) -> Size {
        let width = self
            .font
            .glyph(ch)
            .map(|glyph| glyph.width)
            .unwrap_or_else(|| self.font.cell_width(ch));
        Size::new(width as u32, self.font.size as u32)
    }

    fn positioned_in_cell(
        &self,
        ch: char,
        cell_origin: Point,
        cell: Size,
        use_generated_x_offset: bool,
    ) -> PositionedGlyph {
        let glyph = self.font.glyph(ch).copied();
        let glyph_origin = glyph.map(|glyph| {
            let x_offset = if use_generated_x_offset {
                glyph.x_offset as i32
            } else {
                0
            };
            Point::new(
                cell_origin.x + x_offset,
                cell_origin.y + self.font.size as i32 - glyph.y_offset as i32,
            )
        });

        PositionedGlyph {
            ch,
            glyph,
            cell_origin,
            cell_size: cell,
            glyph_origin,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LineMetrics {
    min_x: i32,
    max_x: i32,
    height: u32,
    has_chars: bool,
}

impl LineMetrics {
    fn new(height: u32) -> Self {
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

    fn width(self) -> u32 {
        if self.has_chars {
            non_negative_u32(self.max_x - self.min_x)
        } else {
            0
        }
    }

    fn min_x(self) -> i32 {
        if self.has_chars { self.min_x } else { 0 }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct ColumnMetrics {
    width: u32,
    min_y: i32,
    max_y: i32,
    has_chars: bool,
}

impl ColumnMetrics {
    fn include(&mut self, pen_y: i32, cell: Size) {
        let bottom = pen_y.saturating_add(cell.height as i32);

        if !self.has_chars {
            self.min_y = pen_y;
            self.max_y = bottom;
            self.has_chars = true;
        } else {
            self.min_y = self.min_y.min(pen_y);
            self.max_y = self.max_y.max(bottom);
        }

        self.width = self.width.max(cell.width);
    }

    fn height(self) -> u32 {
        if self.has_chars {
            non_negative_u32(self.max_y - self.min_y)
        } else {
            0
        }
    }

    fn min_y(self) -> i32 {
        if self.has_chars { self.min_y } else { 0 }
    }
}

#[cfg(feature = "debug")]
pub(crate) fn glyph_box_bounds(positioned: PositionedGlyph) -> Option<(Point, Size)> {
    positioned.glyph.map(|glyph| {
        (
            positioned.glyph_origin.unwrap_or(positioned.cell_origin),
            Size::new(glyph.width as u32, glyph.height as u32),
        )
    })
}

fn advance_pen(pen: i32, advance: i32, spacing: i32, has_next: bool) -> i32 {
    let step = if has_next {
        advance.saturating_add(spacing)
    } else {
        advance
    };
    pen.saturating_add(step)
}

fn non_negative_u32(value: i32) -> u32 {
    if value <= 0 { 0 } else { value as u32 }
}
