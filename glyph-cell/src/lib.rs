#![no_std]

mod font;
mod layout;
mod style;
mod text;

#[cfg(feature = "debug")]
mod debug;

#[cfg(feature = "debug")]
pub use debug::DebugBoxKind;
pub use font::{FontData, Glyph};
pub use glyph_cell_macros::font_data;
pub use style::{Alignment, HorizontalAlignment, TextLayout, TextStyle, VerticalAlignment};
pub use text::DrawableText;
