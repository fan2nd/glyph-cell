use embedded_graphics_core::{
    Pixel,
    draw_target::DrawTarget,
    geometry::{Point, Size},
    pixelcolor::PixelColor,
};

use crate::{
    DrawableText,
    layout::{PositionedGlyph, TextRun, glyph_box_bounds},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugBoxKind {
    Cell,
    Glyph,
}

impl<'a, C: PixelColor> DrawableText<'a, C> {
    pub fn draw_debug_boxes<D>(&self, target: &mut D, kind: DebugBoxKind) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = C>,
    {
        draw_text_boxes(target, self.run(), self.style.color, kind)
    }

    pub fn draw_cell_boxes<D>(&self, target: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = C>,
    {
        self.draw_debug_boxes(target, DebugBoxKind::Cell)
    }

    pub fn draw_glyph_boxes<D>(&self, target: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = C>,
    {
        self.draw_debug_boxes(target, DebugBoxKind::Glyph)
    }
}

fn draw_text_boxes<D, C>(
    target: &mut D,
    run: TextRun<'_, '_>,
    color: C,
    kind: DebugBoxKind,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = C>,
    C: PixelColor,
{
    run.for_each_positioned_glyph(|positioned| {
        if let Some((origin, size)) = box_bounds(positioned, kind) {
            draw_outline(target, origin, size, color)?;
        }
        Ok(())
    })
}

fn box_bounds(positioned: PositionedGlyph, kind: DebugBoxKind) -> Option<(Point, Size)> {
    match kind {
        DebugBoxKind::Cell => Some((positioned.cell_origin, positioned.cell_size)),
        DebugBoxKind::Glyph => glyph_box_bounds(positioned),
    }
}

fn draw_outline<D, C>(target: &mut D, origin: Point, size: Size, color: C) -> Result<(), D::Error>
where
    D: DrawTarget<Color = C>,
    C: PixelColor,
{
    if size.width == 0 || size.height == 0 {
        return Ok(());
    }

    let right = origin.x + size.width as i32 - 1;
    let bottom = origin.y + size.height as i32 - 1;

    for x in origin.x..=right {
        target.draw_iter(core::iter::once(Pixel(Point::new(x, origin.y), color)))?;
        if bottom != origin.y {
            target.draw_iter(core::iter::once(Pixel(Point::new(x, bottom), color)))?;
        }
    }

    for y in (origin.y + 1)..bottom {
        target.draw_iter(core::iter::once(Pixel(Point::new(origin.x, y), color)))?;
        if right != origin.x {
            target.draw_iter(core::iter::once(Pixel(Point::new(right, y), color)))?;
        }
    }

    Ok(())
}
