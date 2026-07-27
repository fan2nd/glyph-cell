use embedded_graphics_core::{
    Drawable, Pixel,
    draw_target::DrawTarget,
    geometry::{Point, Size},
    pixelcolor::PixelColor,
    primitives::Rectangle,
};

use crate::{
    FontData, Glyph, TextStyle,
    layout::{TextFlow, TextRun},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrawableText<'a, C: PixelColor> {
    pub font_data: &'a FontData<'a>,
    pub text: &'a str,
    pub start_point: Point,
    pub style: TextStyle<C>,
    flow: TextFlow,
}

impl<'a, C: PixelColor> DrawableText<'a, C> {
    pub const fn new(font_data: &'a FontData<'a>, text: &'a str, style: TextStyle<C>) -> Self {
        Self {
            font_data,
            text,
            start_point: Point::zero(),
            style,
            flow: TextFlow::Horizontal,
        }
    }

    pub const fn at(mut self, start_point: Point) -> Self {
        self.start_point = start_point;
        self
    }

    pub const fn horizontal(mut self) -> Self {
        self.flow = TextFlow::Horizontal;
        self
    }

    pub const fn vertical(mut self) -> Self {
        self.flow = TextFlow::Vertical;
        self
    }

    pub fn measure(&self) -> Size {
        self.run().measure()
    }

    pub fn bounding_box(&self) -> Rectangle {
        Rectangle::new(self.run().top_left(), self.measure())
    }

    pub(crate) fn run(&self) -> TextRun<'a, 'a> {
        TextRun::new(
            self.font_data,
            self.text,
            self.start_point,
            self.style.layout,
            self.style.alignment,
            self.flow,
        )
    }
}

impl<'a, C> Drawable for DrawableText<'a, C>
where
    C: PixelColor,
{
    type Color = C;
    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = C>,
    {
        draw_text_run(target, self.font_data, self.run(), self.style.color)
    }
}

pub(crate) fn draw_text_run<D, C>(
    target: &mut D,
    font: &FontData<'_>,
    run: TextRun<'_, '_>,
    color: C,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = C>,
    C: PixelColor,
{
    run.for_each_positioned_glyph(|positioned| {
        if let (Some(glyph), Some(origin)) = (positioned.glyph, positioned.glyph_origin) {
            draw_glyph(target, font, &glyph, origin, color)?;
        }
        Ok(())
    })
}

pub(crate) fn draw_glyph<D, C>(
    target: &mut D,
    font: &FontData<'_>,
    glyph: &Glyph,
    origin: Point,
    color: C,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = C>,
    C: PixelColor,
{
    for y in 0..glyph.height {
        for x in 0..glyph.width {
            if font.glyph_pixel(glyph, x, y) {
                target.draw_iter(core::iter::once(Pixel(
                    origin + Point::new(x as i32, y as i32),
                    color,
                )))?;
            }
        }
    }
    Ok(())
}
