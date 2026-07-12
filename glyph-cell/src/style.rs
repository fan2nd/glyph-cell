use embedded_graphics_core::{geometry::Size, pixelcolor::PixelColor};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorizontalAlignment {
    Left,
    Center,
    Right,
}

impl HorizontalAlignment {
    pub(crate) fn offset(self, outer_width: u32, inner_width: u32) -> i32 {
        match self {
            Self::Left => 0,
            Self::Center => (outer_width as i32 - inner_width as i32) / 2,
            Self::Right => outer_width as i32 - inner_width as i32,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalAlignment {
    Top,
    Middle,
    Bottom,
}

impl VerticalAlignment {
    pub(crate) fn offset(self, outer_height: u32, inner_height: u32) -> i32 {
        match self {
            Self::Top => 0,
            Self::Middle => (outer_height as i32 - inner_height as i32) / 2,
            Self::Bottom => outer_height as i32 - inner_height as i32,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Alignment {
    pub horizontal: HorizontalAlignment,
    pub vertical: VerticalAlignment,
}

impl Alignment {
    pub const TOP_LEFT: Self = Self::new(HorizontalAlignment::Left, VerticalAlignment::Top);
    pub const TOP_CENTER: Self = Self::new(HorizontalAlignment::Center, VerticalAlignment::Top);
    pub const TOP_RIGHT: Self = Self::new(HorizontalAlignment::Right, VerticalAlignment::Top);
    pub const MIDDLE_LEFT: Self = Self::new(HorizontalAlignment::Left, VerticalAlignment::Middle);
    pub const CENTER: Self = Self::new(HorizontalAlignment::Center, VerticalAlignment::Middle);
    pub const MIDDLE_RIGHT: Self = Self::new(HorizontalAlignment::Right, VerticalAlignment::Middle);
    pub const BOTTOM_LEFT: Self = Self::new(HorizontalAlignment::Left, VerticalAlignment::Bottom);
    pub const BOTTOM_CENTER: Self =
        Self::new(HorizontalAlignment::Center, VerticalAlignment::Bottom);
    pub const BOTTOM_RIGHT: Self = Self::new(HorizontalAlignment::Right, VerticalAlignment::Bottom);

    pub const fn new(horizontal: HorizontalAlignment, vertical: VerticalAlignment) -> Self {
        Self {
            horizontal,
            vertical,
        }
    }

    pub(crate) fn offset(self, outer: Size, inner: Size) -> (i32, i32) {
        (
            self.horizontal.offset(outer.width, inner.width),
            self.vertical.offset(outer.height, inner.height),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextLayout {
    Monospace { spacing: i32, line_spacing: i32 },
    Proportional { spacing: i32, line_spacing: i32 },
}

impl TextLayout {
    pub const fn monospace() -> Self {
        Self::Monospace {
            spacing: 0,
            line_spacing: 0,
        }
    }

    pub const fn monospace_with_spacing(spacing: i32, line_spacing: i32) -> Self {
        Self::Monospace {
            spacing,
            line_spacing,
        }
    }

    pub const fn proportional(spacing: i32) -> Self {
        Self::Proportional {
            spacing,
            line_spacing: 0,
        }
    }

    pub const fn proportional_with_line_spacing(spacing: i32, line_spacing: i32) -> Self {
        Self::Proportional {
            spacing,
            line_spacing,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextStyle<C: PixelColor> {
    pub color: C,
    pub layout: TextLayout,
    pub alignment: Alignment,
}

impl<C: PixelColor> TextStyle<C> {
    pub const fn new(color: C) -> Self {
        Self {
            color,
            layout: TextLayout::Proportional {
                spacing: 0,
                line_spacing: 0,
            },
            alignment: Alignment::CENTER,
        }
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

    pub const fn align(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }
}
