#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use core::convert::Infallible;
use std::{
    fs,
    path::{Path, PathBuf},
    ptr::null_mut,
    slice,
    sync::Mutex,
};

use embedded_graphics_core::{
    Drawable, Pixel,
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Point, Size},
    pixelcolor::{Rgb888, RgbColor},
};
use freetype::freetype as ft;
use glyph_cell::{
    Alignment, DebugBoxKind, DrawableText, FontData as GlyphCellFontData, Glyph, TextStyle,
};
use serde::{Deserialize, Serialize};
use tauri::State;

fn main() {
    tauri::Builder::default()
        .manage(SimulatorState::new())
        .invoke_handler(tauri::generate_handler![
            get_initial_state,
            render_preview,
            refresh_system_fonts,
            choose_font_file,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run glyph-cell simulator");
}

struct SimulatorState {
    system_fonts: Mutex<Vec<SystemFont>>,
    font_cache: Mutex<FontCache>,
}

impl SimulatorState {
    fn new() -> Self {
        Self {
            system_fonts: Mutex::new(discover_system_fonts()),
            font_cache: Mutex::new(FontCache::default()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InitialState {
    settings: SimulatorSettings,
    system_fonts: Vec<SystemFontDto>,
    render: RenderResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SimulatorSettings {
    text: String,
    layout_mode: LayoutMode,
    flow: FlowMode,
    alignment: AlignmentChoice,
    debug_overlays: DebugOverlays,
    font_source: FontSource,
    selected_system_font: usize,
    custom_font_path: String,
    collection_index: u32,
    font_size: u16,
    ascii_width: u32,
    spacing: i32,
    line_spacing: i32,
    glyph_y_offsets: String,
    origin_x: i32,
    origin_y: i32,
    canvas_width: u32,
    canvas_height: u32,
    zoom: f32,
    glyph_color: Color,
}

impl Default for SimulatorSettings {
    fn default() -> Self {
        Self {
            text: "AWi\nmg\u{4f60}\u{597d}".to_owned(),
            layout_mode: LayoutMode::Proportional,
            flow: FlowMode::Horizontal,
            alignment: AlignmentChoice::TopLeft,
            debug_overlays: DebugOverlays::default(),
            font_source: FontSource::System,
            selected_system_font: 0,
            custom_font_path: String::new(),
            collection_index: 0,
            font_size: 18,
            ascii_width: 10,
            spacing: 1,
            line_spacing: 0,
            glyph_y_offsets: String::new(),
            origin_x: 4,
            origin_y: 22,
            canvas_width: 180,
            canvas_height: 96,
            zoom: 4.0,
            glyph_color: Color::rgb(54, 187, 128),
        }
    }
}

impl SimulatorSettings {
    fn sanitized(mut self) -> Self {
        self.collection_index = self.collection_index.min(8);
        self.font_size = self.font_size.clamp(4, 96);
        self.ascii_width = self.ascii_width.clamp(1, 128);
        self.spacing = self.spacing.clamp(-16, 48);
        self.line_spacing = self.line_spacing.clamp(-16, 64);
        self.origin_x = self.origin_x.clamp(-80, 240);
        self.origin_y = self.origin_y.clamp(-80, 180);
        self.canvas_width = self.canvas_width.clamp(32, 360);
        self.canvas_height = self.canvas_height.clamp(24, 240);
        self.zoom = finite_f32(self.zoom).clamp(1.0, 16.0);
        self
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SystemFontDto {
    name: String,
    label: String,
    path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RenderResponse {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    measurement: Measurement,
    font: FontReport,
    example_code: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Measurement {
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FontReport {
    path: Option<String>,
    error: Option<String>,
    loaded_glyphs: usize,
    index: String,
    missing_chars: String,
    clipped_chars: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Color {
    r: u8,
    g: u8,
    b: u8,
}

impl Color {
    const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

#[tauri::command]
fn get_initial_state(state: State<'_, SimulatorState>) -> Result<InitialState, String> {
    let settings = SimulatorSettings::default();
    let system_fonts = clone_system_fonts(&state)?;
    let mut font_cache = lock_font_cache(&state)?;
    let render = render_response(&settings, &system_fonts, &mut font_cache);

    Ok(InitialState {
        settings,
        system_fonts: system_font_dtos(&system_fonts),
        render,
    })
}

#[tauri::command]
fn render_preview(
    settings: SimulatorSettings,
    state: State<'_, SimulatorState>,
) -> Result<RenderResponse, String> {
    let settings = settings.sanitized();
    let system_fonts = clone_system_fonts(&state)?;
    let mut font_cache = lock_font_cache(&state)?;
    Ok(render_response(&settings, &system_fonts, &mut font_cache))
}

#[tauri::command]
fn refresh_system_fonts(state: State<'_, SimulatorState>) -> Result<Vec<SystemFontDto>, String> {
    let fonts = discover_system_fonts();
    {
        let mut system_fonts = state
            .system_fonts
            .lock()
            .map_err(|_| "System font list lock is poisoned".to_owned())?;
        *system_fonts = fonts.clone();
    }
    lock_font_cache(&state)?.invalidate();
    Ok(system_font_dtos(&fonts))
}

#[tauri::command]
fn choose_font_file(current: Option<String>) -> Option<String> {
    let current = current
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(Path::new);

    pick_font_file(current).map(|path| path.to_string_lossy().into_owned())
}

fn clone_system_fonts(state: &State<'_, SimulatorState>) -> Result<Vec<SystemFont>, String> {
    state
        .system_fonts
        .lock()
        .map(|fonts| fonts.clone())
        .map_err(|_| "System font list lock is poisoned".to_owned())
}

fn lock_font_cache<'a>(
    state: &'a State<'_, SimulatorState>,
) -> Result<std::sync::MutexGuard<'a, FontCache>, String> {
    state
        .font_cache
        .lock()
        .map_err(|_| "Font cache lock is poisoned".to_owned())
}

fn render_response(
    settings: &SimulatorSettings,
    system_fonts: &[SystemFont],
    font_cache: &mut FontCache,
) -> RenderResponse {
    ensure_font(settings, system_fonts, font_cache);

    let frame = render_frame(settings, &font_cache.data);
    let measurement = measure_text(settings, &font_cache.data);
    let path = current_font_path(settings, system_fonts);
    let example_code = example_code(settings, &font_cache.data, path.as_deref());

    RenderResponse {
        width: frame.width,
        height: frame.height,
        rgba: frame.into_rgba(),
        measurement: Measurement {
            width: measurement.width,
            height: measurement.height,
        },
        font: FontReport {
            path: path.map(|path| path.to_string_lossy().into_owned()),
            error: font_cache.error.clone(),
            loaded_glyphs: font_cache.data.glyphs.len(),
            index: font_cache.data.index.clone(),
            missing_chars: font_cache.missing_chars.clone(),
            clipped_chars: font_cache.clipped_chars.clone(),
        },
        example_code,
    }
}

fn render_frame(settings: &SimulatorSettings, font_data: &OwnedFontData) -> FrameBuffer {
    let mut frame = FrameBuffer::new(settings.canvas_width, settings.canvas_height);
    let font_data = font_data.as_font_data();
    let text = drawable_text(settings, &font_data, settings.glyph_color);
    let _ = text.draw(&mut frame);

    if settings.debug_overlays.has_any() {
        for kind in settings.debug_overlays.kinds() {
            let overlay = drawable_text(settings, &font_data, debug_box_color(kind));
            let _ = overlay.draw_debug_boxes(&mut frame, kind);
        }
    }

    frame
}

fn measure_text(settings: &SimulatorSettings, font_data: &OwnedFontData) -> Size {
    let font_data = font_data.as_font_data();
    drawable_text(settings, &font_data, settings.glyph_color).measure()
}

fn drawable_text<'a>(
    settings: &'a SimulatorSettings,
    font_data: &'a GlyphCellFontData<'a>,
    color: Color,
) -> DrawableText<'a, Rgb888> {
    let style = text_style(settings, color).align(settings.alignment.to_glyph_cell_alignment());
    let text = DrawableText::new(font_data, &settings.text, style)
        .at(Point::new(settings.origin_x, settings.origin_y));

    match settings.flow {
        FlowMode::Horizontal => text.horizontal(),
        FlowMode::Vertical => text.vertical(),
    }
}

fn text_style(settings: &SimulatorSettings, color: Color) -> TextStyle<Rgb888> {
    let style = TextStyle::new(color_to_rgb(color));
    match settings.layout_mode {
        LayoutMode::Monospace => {
            style.monospace_with_spacing(settings.spacing, settings.line_spacing)
        }
        LayoutMode::Proportional => {
            style.proportional_with_line_spacing(settings.spacing, settings.line_spacing)
        }
    }
}

fn ensure_font(
    settings: &SimulatorSettings,
    system_fonts: &[SystemFont],
    font_cache: &mut FontCache,
) {
    let Some(path) = current_font_path(settings, system_fonts) else {
        font_cache.error = Some("No font path selected".to_owned());
        font_cache.missing_chars.clear();
        font_cache.clipped_chars.clear();
        return;
    };

    let index = glyph_index_from_text(&settings.text);
    let key = FontBuildKey {
        path: path.to_string_lossy().into_owned(),
        collection_index: settings.collection_index,
        size: settings.font_size,
        ascii_width: settings.ascii_width,
        index,
        y_offsets: settings.glyph_y_offsets.clone(),
    };

    if font_cache.key.as_ref() == Some(&key) {
        return;
    }

    match build_font_data(
        &path,
        settings.collection_index,
        settings.font_size,
        settings.ascii_width as u16,
        &key.index,
        &settings.glyph_y_offsets,
    ) {
        Ok(build) => {
            font_cache.key = Some(key);
            font_cache.data = build.data;
            font_cache.missing_chars = build.missing_chars;
            font_cache.clipped_chars = build.clipped_chars;
            font_cache.error = None;
        }
        Err(err) => {
            font_cache.key = Some(key);
            font_cache.error = Some(err);
            font_cache.missing_chars.clear();
            font_cache.clipped_chars.clear();
        }
    }
}

fn current_font_path(settings: &SimulatorSettings, system_fonts: &[SystemFont]) -> Option<PathBuf> {
    match settings.font_source {
        FontSource::System => system_fonts
            .get(settings.selected_system_font)
            .map(|font| font.path.clone()),
        FontSource::Custom => current_custom_font_path(settings),
    }
}

fn current_custom_font_path(settings: &SimulatorSettings) -> Option<PathBuf> {
    let trimmed = settings.custom_font_path.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

fn example_code(
    settings: &SimulatorSettings,
    font_data: &OwnedFontData,
    current_font_path: Option<&Path>,
) -> String {
    let alignment = settings.alignment.code_name();
    let text = rust_string_literal(&settings.text);
    let index = rust_string_literal(&font_data.index);
    let path = current_font_path
        .map(|path| rust_string_literal(&path.to_string_lossy()))
        .unwrap_or_else(|| "\"path/to/font.ttf\"".to_owned());
    let layout = match settings.layout_mode {
        LayoutMode::Monospace if settings.spacing == 0 && settings.line_spacing == 0 => {
            "    .monospace()".to_owned()
        }
        LayoutMode::Monospace => format!(
            "    .monospace_with_spacing({}, {})",
            settings.spacing, settings.line_spacing
        ),
        LayoutMode::Proportional => format!(
            "    .proportional_with_line_spacing({}, {})",
            settings.spacing, settings.line_spacing
        ),
    };
    let flow = match settings.flow {
        FlowMode::Horizontal => String::new(),
        FlowMode::Vertical => "    .vertical()\n".to_owned(),
    };
    let y_offsets = example_y_offsets(settings, &font_data.index);

    format!(
        "use embedded_graphics_core::geometry::Point;\nuse embedded_graphics_core::pixelcolor::Rgb888;\nuse glyph_cell::{{font_data, Alignment, DrawableText, FontData, TextStyle}};\n\nconst FONT: FontData<'static> = font_data! {{\n    size: {},\n    ascii_width: {},\n    path: {},\n    index: {},\n{}}};\n\nlet style = TextStyle::new(Rgb888::new({}, {}, {}))\n{}\n    .align(Alignment::{});\n\nDrawableText::new(&FONT, {}, style)\n{}    .at(Point::new({}, {}))\n    .draw(&mut display)?;\n",
        settings.font_size,
        settings.ascii_width,
        path,
        index,
        y_offsets,
        settings.glyph_color.r,
        settings.glyph_color.g,
        settings.glyph_color.b,
        layout,
        alignment,
        text,
        flow,
        settings.origin_x,
        settings.origin_y
    )
}

fn example_y_offsets(settings: &SimulatorSettings, index: &str) -> String {
    let Ok(offsets) = parse_y_offset_tweaks(&settings.glyph_y_offsets, index) else {
        return String::new();
    };

    if offsets.is_empty() {
        return String::new();
    }

    let mut out = String::from("    y_offset: {\n");
    for (codepoint, delta) in offsets {
        out.push_str("        ");
        out.push_str(&rust_char_literal(codepoint));
        out.push_str(": ");
        out.push_str(&delta.to_string());
        out.push_str(",\n");
    }
    out.push_str("    },\n");
    out
}

fn system_font_dtos(system_fonts: &[SystemFont]) -> Vec<SystemFontDto> {
    system_fonts
        .iter()
        .map(|font| SystemFontDto {
            name: font.name.clone(),
            label: font.label().to_owned(),
            path: font.path.to_string_lossy().into_owned(),
        })
        .collect()
}

fn debug_box_color(kind: DebugBoxKind) -> Color {
    match kind {
        DebugBoxKind::Cell => Color::rgb(72, 166, 255),
        DebugBoxKind::Glyph => Color::rgb(233, 96, 154),
    }
}

fn finite_f32(value: f32) -> f32 {
    if value.is_finite() { value } else { 1.0 }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum FontSource {
    System,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum LayoutMode {
    Monospace,
    Proportional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum FlowMode {
    Horizontal,
    Vertical,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DebugOverlays {
    cell: bool,
    glyph: bool,
}

impl DebugOverlays {
    fn has_any(self) -> bool {
        self.cell || self.glyph
    }

    fn kinds(self) -> impl Iterator<Item = DebugBoxKind> {
        [
            self.cell.then_some(DebugBoxKind::Cell),
            self.glyph.then_some(DebugBoxKind::Glyph),
        ]
        .into_iter()
        .flatten()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum AlignmentChoice {
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    Center,
    MiddleRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl AlignmentChoice {
    fn code_name(self) -> &'static str {
        match self {
            Self::TopLeft => "TOP_LEFT",
            Self::TopCenter => "TOP_CENTER",
            Self::TopRight => "TOP_RIGHT",
            Self::MiddleLeft => "MIDDLE_LEFT",
            Self::Center => "CENTER",
            Self::MiddleRight => "MIDDLE_RIGHT",
            Self::BottomLeft => "BOTTOM_LEFT",
            Self::BottomCenter => "BOTTOM_CENTER",
            Self::BottomRight => "BOTTOM_RIGHT",
        }
    }

    fn to_glyph_cell_alignment(self) -> Alignment {
        match self {
            Self::TopLeft => Alignment::TOP_LEFT,
            Self::TopCenter => Alignment::TOP_CENTER,
            Self::TopRight => Alignment::TOP_RIGHT,
            Self::MiddleLeft => Alignment::MIDDLE_LEFT,
            Self::Center => Alignment::CENTER,
            Self::MiddleRight => Alignment::MIDDLE_RIGHT,
            Self::BottomLeft => Alignment::BOTTOM_LEFT,
            Self::BottomCenter => Alignment::BOTTOM_CENTER,
            Self::BottomRight => Alignment::BOTTOM_RIGHT,
        }
    }
}

#[derive(Clone)]
struct SystemFont {
    name: String,
    path: PathBuf,
}

impl SystemFont {
    fn label(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FontBuildKey {
    path: String,
    collection_index: u32,
    size: u16,
    ascii_width: u32,
    index: String,
    y_offsets: String,
}

struct FontCache {
    key: Option<FontBuildKey>,
    data: OwnedFontData,
    missing_chars: String,
    clipped_chars: String,
    error: Option<String>,
}

impl FontCache {
    fn invalidate(&mut self) {
        self.key = None;
    }
}

impl Default for FontCache {
    fn default() -> Self {
        Self {
            key: None,
            data: OwnedFontData::fallback(),
            missing_chars: String::new(),
            clipped_chars: String::new(),
            error: None,
        }
    }
}

struct OwnedFontData {
    index: String,
    size: u16,
    ascii_width: u16,
    bitmap: Vec<u8>,
    glyphs: Vec<Glyph>,
}

impl OwnedFontData {
    fn as_font_data(&self) -> GlyphCellFontData<'_> {
        GlyphCellFontData {
            index: &self.index,
            size: self.size,
            ascii_width: self.ascii_width,
            bitmap: &self.bitmap,
            glyphs: &self.glyphs,
        }
    }

    fn fallback() -> Self {
        Self {
            index: "A".to_owned(),
            size: 7,
            ascii_width: 5,
            bitmap: vec![0b01110100, 0b01100011, 0b11111000, 0b11000110, 0b00100000],
            glyphs: vec![Glyph {
                bitmap_offset: 0,
                width: 5,
                height: 7,
                cell_width: 5,
                x_offset: 0,
                y_offset: 7,
                x_min: 0,
                y_min: 0,
                advance_width: 6,
            }],
        }
    }
}

struct FontBuild {
    data: OwnedFontData,
    missing_chars: String,
    clipped_chars: String,
}

struct FrameBuffer {
    width: u32,
    height: u32,
    pixels: Vec<Option<Color>>,
}

impl FrameBuffer {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![None; width as usize * height as usize],
        }
    }

    fn set_pixel(&mut self, point: Point, color: Color) {
        let Some(index) = self.pixel_index(point) else {
            return;
        };
        self.pixels[index] = Some(color);
    }

    fn into_rgba(self) -> Vec<u8> {
        let mut rgba = Vec::with_capacity(self.pixels.len() * 4);
        for pixel in self.pixels {
            if let Some(color) = pixel {
                rgba.extend([color.r, color.g, color.b, 255]);
            } else {
                rgba.extend([0, 0, 0, 0]);
            }
        }
        rgba
    }

    fn pixel_index(&self, point: Point) -> Option<usize> {
        if point.x < 0 || point.y < 0 {
            return None;
        }

        let x = point.x as u32;
        let y = point.y as u32;
        if x >= self.width || y >= self.height {
            return None;
        }

        Some((y * self.width + x) as usize)
    }
}

impl OriginDimensions for FrameBuffer {
    fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }
}

impl DrawTarget for FrameBuffer {
    type Color = Rgb888;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            self.set_pixel(point, rgb_to_color(color));
        }

        Ok(())
    }
}

fn build_font_data(
    path: &Path,
    collection_index: u32,
    size: u16,
    ascii_width: u16,
    index: &str,
    y_offset_tweaks: &str,
) -> Result<FontBuild, String> {
    let bytes = fs::read(path).map_err(|err| format!("Failed to read font file: {err}"))?;
    let font = PreviewFreeTypeFont::new(bytes, collection_index as isize)?;
    let ascii_size = fitting_ascii_size(&font, size, index.chars())?;

    let mut glyphs = Vec::new();
    let mut bitmaps = Vec::new();
    let mut missing_chars = String::new();
    let mut clipped_chars = String::new();
    let mut active_size = None;

    for ch in index.chars() {
        if !font.has_glyph(ch) {
            missing_chars.push(ch);
        }

        let glyph_size = if ch.is_ascii() { ascii_size } else { size };
        set_active_preview_pixel_size(&font, &mut active_size, glyph_size)?;
        let rasterized = font.rasterize_glyph(ch)?;
        glyphs.push(glyph_from_rasterized(&rasterized));
        bitmaps.push(rasterized.bitmap);
    }
    apply_auto_y_offsets(size, index, &mut glyphs);
    apply_y_offset_tweaks(&mut glyphs, index, y_offset_tweaks)?;
    apply_cell_offsets(size, ascii_width, index, &mut glyphs);

    let mut bitmap = Vec::new();
    for ((ch, glyph), pixels) in index.chars().zip(glyphs.iter_mut()).zip(bitmaps.iter_mut()) {
        if fit_glyph_to_cell(size, glyph, pixels) {
            clipped_chars.push(ch);
        }
        glyph.bitmap_offset = bitmap.len() as u32;
        pack_pixels(pixels, &mut bitmap);
    }

    Ok(FontBuild {
        data: OwnedFontData {
            index: index.to_owned(),
            size,
            ascii_width,
            bitmap,
            glyphs,
        },
        missing_chars,
        clipped_chars,
    })
}

fn fitting_ascii_size(
    font: &PreviewFreeTypeFont,
    raster_height: u16,
    chars: impl IntoIterator<Item = char>,
) -> Result<u16, String> {
    let chars = chars
        .into_iter()
        .filter(|codepoint| codepoint.is_ascii())
        .collect::<Vec<_>>();
    if chars.is_empty() {
        return Ok(raster_height);
    }

    for glyph_size in (1..=raster_height).rev() {
        let mut active_size = None;
        set_active_preview_pixel_size(font, &mut active_size, glyph_size)?;
        let glyphs = chars
            .iter()
            .copied()
            .map(|codepoint| font.rasterize_glyph(codepoint))
            .map(|result| result.map(|rasterized| glyph_from_rasterized(&rasterized)))
            .collect::<Result<Vec<_>, _>>()?;

        if glyphs_fit_vertically(raster_height, &glyphs) {
            return Ok(glyph_size);
        }
    }

    Ok(1)
}

fn set_active_preview_pixel_size(
    font: &PreviewFreeTypeFont,
    active_size: &mut Option<u16>,
    size: u16,
) -> Result<(), String> {
    if *active_size == Some(size) {
        return Ok(());
    }

    font.set_pixel_size(size)?;
    *active_size = Some(size);
    Ok(())
}

fn glyph_from_rasterized(rasterized: &RasterizedGlyph) -> Glyph {
    Glyph {
        bitmap_offset: 0,
        width: rasterized.width,
        height: rasterized.height,
        cell_width: 0,
        x_offset: 0,
        y_offset: rasterized.y_offset,
        x_min: rasterized.x_min,
        y_min: rasterized.y_min,
        advance_width: rasterized.advance_width,
    }
}

struct PreviewFreeTypeFont {
    library: ft::FT_Library,
    face: ft::FT_Face,
    _bytes: Vec<u8>,
}

impl PreviewFreeTypeFont {
    fn new(bytes: Vec<u8>, face_index: isize) -> Result<Self, String> {
        let mut library = null_mut();
        ft_ok(
            unsafe { ft::FT_Init_FreeType(&mut library) },
            "initialize FreeType",
        )?;

        let mut face = null_mut();
        let face_result = ft_ok(
            unsafe {
                ft::FT_New_Memory_Face(
                    library,
                    bytes.as_ptr(),
                    bytes.len() as ft::FT_Long,
                    face_index as ft::FT_Long,
                    &mut face,
                )
            },
            "parse font",
        );

        if let Err(err) = face_result {
            unsafe {
                let _ = ft::FT_Done_FreeType(library);
            }
            return Err(err);
        }

        Ok(Self {
            library,
            face,
            _bytes: bytes,
        })
    }

    fn set_pixel_size(&self, height: u16) -> Result<(), String> {
        ft_ok(
            unsafe { ft::FT_Set_Char_Size(self.face, 0, height as ft::FT_F26Dot6 * 64, 300, 300) },
            "set character size",
        )?;
        ft_ok(
            unsafe { ft::FT_Set_Pixel_Sizes(self.face, 0, height as u32) },
            "set pixel size",
        )
    }

    fn has_glyph(&self, ch: char) -> bool {
        unsafe { ft::FT_Get_Char_Index(self.face, ch as ft::FT_ULong) != 0 }
    }

    fn rasterize_glyph(&self, ch: char) -> Result<RasterizedGlyph, String> {
        let glyph_index = unsafe { ft::FT_Get_Char_Index(self.face, ch as ft::FT_ULong) };
        ft_ok(
            unsafe { ft::FT_Load_Glyph(self.face, glyph_index, glyph_load_flags()) },
            "load glyph",
        )?;
        let slot = unsafe { (*self.face).glyph };

        let slot = unsafe { &*slot };
        let raw_width = slot.bitmap.width as usize;
        let raw_height = slot.bitmap.rows as usize;
        let width = raw_width.max(1).min(u16::MAX as usize) as u16;
        let height = raw_height.max(1).min(u16::MAX as usize) as u16;

        Ok(RasterizedGlyph {
            width,
            height,
            x_min: clamp_i32_to_i16(slot.bitmap_left),
            y_min: clamp_i32_to_i16(slot.bitmap_top - raw_height as i32),
            y_offset: clamp_i32_to_i16(slot.bitmap_top),
            advance_width: advance_width_pixels_16dot16(slot.linearHoriAdvance),
            bitmap: bitmap_pixels(&slot.bitmap, width, height),
        })
    }
}

impl Drop for PreviewFreeTypeFont {
    fn drop(&mut self) {
        unsafe {
            if !self.face.is_null() {
                let _ = ft::FT_Done_Face(self.face);
            }
            if !self.library.is_null() {
                let _ = ft::FT_Done_FreeType(self.library);
            }
        }
    }
}

struct RasterizedGlyph {
    width: u16,
    height: u16,
    x_min: i16,
    y_min: i16,
    y_offset: i16,
    advance_width: u16,
    bitmap: Vec<u8>,
}

fn bitmap_pixels(bitmap: &ft::FT_Bitmap, width: u16, height: u16) -> Vec<u8> {
    let width = width as usize;
    let height = height as usize;
    if bitmap.buffer.is_null() || bitmap.width == 0 || bitmap.rows == 0 {
        return vec![0; width * height];
    }

    let pitch = bitmap.pitch;
    let row_bytes = pitch.unsigned_abs() as usize;
    let buffer = unsafe { slice::from_raw_parts(bitmap.buffer, row_bytes * bitmap.rows as usize) };
    let mut pixels = vec![0; width * height];

    for y in 0..height.min(bitmap.rows as usize) {
        let source_y = if pitch >= 0 {
            y
        } else {
            bitmap.rows as usize - 1 - y
        };
        let row = source_y * row_bytes;
        for x in 0..width.min(bitmap.width as usize) {
            pixels[y * width + x] = match bitmap.pixel_mode as u32 {
                value if value == ft::FT_Pixel_Mode::FT_PIXEL_MODE_MONO as u32 => {
                    let byte = buffer[row + x / 8];
                    if byte & (0x80 >> (x % 8)) != 0 {
                        255
                    } else {
                        0
                    }
                }
                value if value == ft::FT_Pixel_Mode::FT_PIXEL_MODE_GRAY as u32 => buffer[row + x],
                _ => 0,
            };
        }
    }

    pixels
}

fn ft_ok(error: ft::FT_Error, action: &str) -> Result<(), String> {
    if error == ft::FT_Err_Ok as ft::FT_Error {
        Ok(())
    } else {
        Err(format!("Failed to {action}: FreeType error {error}"))
    }
}

fn advance_width_pixels_16dot16(advance_width: ft::FT_Fixed) -> u16 {
    if advance_width <= 0 {
        0
    } else {
        ((advance_width as i64 + 65535) / 65536).min(u16::MAX as i64) as u16
    }
}

fn ft_load_target_mono() -> i32 {
    2 << 16
}

fn glyph_load_flags() -> i32 {
    ft::FT_LOAD_RENDER as i32
        | ft::FT_LOAD_FORCE_AUTOHINT as i32
        | ft_load_target_mono()
        | ft::FT_LOAD_MONOCHROME as i32
}

fn apply_cell_offsets(raster_height: u16, ascii_width: u16, index: &str, glyphs: &mut [Glyph]) {
    for (ch, glyph) in index.chars().zip(glyphs.iter_mut()) {
        glyph.cell_width = if ch.is_ascii() {
            ascii_width
        } else {
            raster_height
        };
        glyph.x_offset = centered_offset(glyph.cell_width, glyph.width);
    }
}

fn fit_glyph_to_cell(raster_height: u16, glyph: &mut Glyph, pixels: &mut Vec<u8>) -> bool {
    let cell_width = glyph.cell_width as i32;
    let cell_height = raster_height as i32;
    let left = glyph.x_offset as i32;
    let top = cell_height - glyph.y_offset as i32;
    let right = left + glyph.width as i32;
    let bottom = top + glyph.height as i32;

    let visible_left = left.clamp(0, cell_width);
    let visible_top = top.clamp(0, cell_height);
    let visible_right = right.clamp(visible_left, cell_width);
    let visible_bottom = bottom.clamp(visible_top, cell_height);

    let new_width = (visible_right - visible_left) as u16;
    let new_height = (visible_bottom - visible_top) as u16;
    let source_x = (visible_left - left).max(0) as usize;
    let source_y = (visible_top - top).max(0) as usize;
    let old_width = glyph.width as usize;
    let old_height = glyph.height as usize;
    let vertically_clipped = source_y != 0 || new_height as usize != old_height;

    if source_x == 0
        && source_y == 0
        && new_width as usize == old_width
        && new_height as usize == old_height
    {
        return false;
    }

    let mut clipped = vec![0; new_width as usize * new_height as usize];
    for y in 0..new_height as usize {
        for x in 0..new_width as usize {
            clipped[y * new_width as usize + x] = pixels[(source_y + y) * old_width + source_x + x];
        }
    }

    let cropped_bottom = (bottom - visible_bottom).max(0);
    glyph.width = new_width;
    glyph.height = new_height;
    glyph.x_offset = clamp_i32_to_i16(visible_left);
    glyph.y_offset = clamp_i32_to_i16(cell_height - visible_top);
    glyph.x_min = offset_i16_i32(glyph.x_min, source_x as i32);
    glyph.y_min = offset_i16_i32(glyph.y_min, cropped_bottom);
    *pixels = clipped;
    vertically_clipped
}

fn centered_offset(outer: u16, inner: u16) -> i16 {
    clamp_i32_to_i16((outer as i32 - inner as i32) / 2)
}

fn apply_auto_y_offsets(raster_height: u16, index: &str, glyphs: &mut [Glyph]) {
    for ascii in [true, false] {
        let delta = y_offset_delta_for(raster_height, index, glyphs, ascii);
        for (ch, glyph) in index.chars().zip(glyphs.iter_mut()) {
            if ch.is_ascii() == ascii {
                glyph.y_offset = offset_i16_i32(glyph.y_offset, delta);
            }
        }
    }
}

fn y_offset_delta_for(raster_height: u16, index: &str, glyphs: &[Glyph], ascii: bool) -> i32 {
    y_offset_delta_from_iter(
        raster_height,
        index
            .chars()
            .zip(glyphs.iter())
            .filter_map(|(ch, glyph)| (ch.is_ascii() == ascii).then_some(glyph)),
    )
}

fn y_offset_delta_from_iter<'a>(
    raster_height: u16,
    mut glyphs: impl Iterator<Item = &'a Glyph>,
) -> i32 {
    let Some(first) = glyphs.next() else {
        return 0;
    };

    let raster_height = raster_height as i32;
    let mut min_top = glyph_top(raster_height, first);
    let mut max_bottom = glyph_bottom(raster_height, first);

    for glyph in glyphs {
        min_top = min_top.min(glyph_top(raster_height, glyph));
        max_bottom = max_bottom.max(glyph_bottom(raster_height, glyph));
    }

    let min_delta = max_bottom - raster_height;
    let max_delta = min_top;

    if min_delta <= max_delta {
        0.clamp(min_delta, max_delta)
    } else {
        (min_top + max_bottom - raster_height) / 2
    }
}

fn glyph_top(raster_height: i32, glyph: &Glyph) -> i32 {
    raster_height - glyph.y_offset as i32
}

fn glyph_bottom(raster_height: i32, glyph: &Glyph) -> i32 {
    glyph_top(raster_height, glyph) + glyph.height as i32
}

fn glyphs_fit_vertically(raster_height: u16, glyphs: &[Glyph]) -> bool {
    let Some(first) = glyphs.first() else {
        return true;
    };

    let raster_height = raster_height as i32;
    let mut min_top = glyph_top(raster_height, first);
    let mut max_bottom = glyph_bottom(raster_height, first);

    for glyph in &glyphs[1..] {
        min_top = min_top.min(glyph_top(raster_height, glyph));
        max_bottom = max_bottom.max(glyph_bottom(raster_height, glyph));
    }

    max_bottom - min_top <= raster_height
}

fn apply_y_offset_tweaks(
    glyphs: &mut [Glyph],
    index: &str,
    y_offset_tweaks: &str,
) -> Result<(), String> {
    for (codepoint, delta) in parse_y_offset_tweaks(y_offset_tweaks, index)? {
        let Some(glyph_index) = index.chars().position(|candidate| candidate == codepoint) else {
            return Err(format!(
                "y_offset character {codepoint:?} is not in the preview index"
            ));
        };
        if let Some(glyph) = glyphs.get_mut(glyph_index) {
            glyph.y_offset = offset_i16_i32(glyph.y_offset, delta as i32);
        }
    }

    Ok(())
}

fn parse_y_offset_tweaks(input: &str, index: &str) -> Result<Vec<(char, i16)>, String> {
    let mut offsets = Vec::new();

    for (line_index, line) in input.lines().enumerate() {
        let line = line.trim().trim_end_matches(',');
        if line.is_empty() {
            continue;
        }

        let Some((codepoint, delta)) = line.split_once(':') else {
            return Err(format!(
                "Invalid y_offset line {}: expected `char: pixels`",
                line_index + 1
            ));
        };
        let codepoint = parse_y_offset_char(codepoint.trim(), line_index + 1)?;
        if !index.contains(codepoint) {
            return Err(format!(
                "y_offset character {codepoint:?} is not in the preview index"
            ));
        }
        if offsets.iter().any(|(existing, _)| *existing == codepoint) {
            return Err(format!("Duplicate y_offset character {codepoint:?}"));
        }
        let delta = delta
            .trim()
            .trim_end_matches(',')
            .parse::<i16>()
            .map_err(|err| format!("Invalid y_offset value on line {}: {err}", line_index + 1))?;
        offsets.push((codepoint, delta));
    }

    Ok(offsets)
}

fn parse_y_offset_char(input: &str, line: usize) -> Result<char, String> {
    let text = input.trim();
    let text = text
        .strip_prefix('\'')
        .and_then(|text| text.strip_suffix('\''))
        .unwrap_or(text);
    let mut chars = text.chars();
    let Some(codepoint) = chars.next() else {
        return Err(format!("Invalid y_offset character on line {line}"));
    };
    if chars.next().is_some() {
        return Err(format!(
            "Invalid y_offset character on line {line}: expected exactly one character"
        ));
    }
    Ok(codepoint)
}

fn pack_pixels(pixels: &[u8], out: &mut Vec<u8>) {
    let mut byte = 0u8;

    for (index, pixel) in pixels.iter().enumerate() {
        if *pixel != 0 {
            byte |= 1 << (7 - index % 8);
        }
        if index % 8 == 7 {
            out.push(byte);
            byte = 0;
        }
    }

    if !pixels.len().is_multiple_of(8) {
        out.push(byte);
    }
}

fn glyph_index_from_text(text: &str) -> String {
    let mut index = String::new();

    for ch in text.chars() {
        if ch.is_control() || index.contains(ch) {
            continue;
        }
        index.push(ch);
    }

    if index.is_empty() {
        index.push('A');
    }

    index
}

fn discover_system_fonts() -> Vec<SystemFont> {
    let mut fonts = Vec::new();

    for dir in system_font_dirs() {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !is_font_file(&path) {
                continue;
            }

            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };

            fonts.push(SystemFont {
                name: file_name.to_owned(),
                path,
            });
        }
    }

    fonts.sort_by(|a, b| {
        font_priority(&a.name)
            .cmp(&font_priority(&b.name))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    fonts.dedup_by(|a, b| a.path == b.path);
    fonts
}

fn system_font_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    #[cfg(target_os = "windows")]
    {
        dirs.push(PathBuf::from(r"C:\Windows\Fonts"));
    }

    #[cfg(target_os = "macos")]
    {
        dirs.push(PathBuf::from("/System/Library/Fonts"));
        dirs.push(PathBuf::from("/Library/Fonts"));
    }

    #[cfg(target_os = "linux")]
    {
        dirs.push(PathBuf::from("/usr/share/fonts"));
        dirs.push(PathBuf::from("/usr/local/share/fonts"));
    }

    dirs
}

#[cfg(target_os = "windows")]
fn pick_font_file(current: Option<&Path>) -> Option<PathBuf> {
    use std::{
        ffi::OsString,
        os::windows::ffi::{OsStrExt, OsStringExt},
        ptr::{null, null_mut},
    };

    use windows_sys::Win32::UI::Controls::Dialogs::{
        GetOpenFileNameW, OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST,
        OPENFILENAMEW,
    };

    let mut file_buffer = [0u16; 32768];
    if let Some(path) = current {
        copy_path_to_wide_buffer(path, &mut file_buffer);
    }

    let filter: Vec<u16> = "Font files\0*.ttf;*.otf;*.ttc\0All files\0*.*\0\0"
        .encode_utf16()
        .collect();
    let title: Vec<u16> = "Choose font file\0".encode_utf16().collect();
    let initial_dir = current.and_then(Path::parent).map(|path| {
        path.as_os_str()
            .encode_wide()
            .chain([0])
            .collect::<Vec<_>>()
    });

    let mut dialog = OPENFILENAMEW {
        lStructSize: size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: null_mut(),
        hInstance: null_mut(),
        lpstrFilter: filter.as_ptr(),
        lpstrCustomFilter: null_mut(),
        nMaxCustFilter: 0,
        nFilterIndex: 1,
        lpstrFile: file_buffer.as_mut_ptr(),
        nMaxFile: file_buffer.len() as u32,
        lpstrFileTitle: null_mut(),
        nMaxFileTitle: 0,
        lpstrInitialDir: initial_dir.as_ref().map_or(null(), |path| path.as_ptr()),
        lpstrTitle: title.as_ptr(),
        Flags: OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
        nFileOffset: 0,
        nFileExtension: 0,
        lpstrDefExt: null(),
        lCustData: 0,
        lpfnHook: None,
        lpTemplateName: null(),
        pvReserved: null_mut(),
        dwReserved: 0,
        FlagsEx: 0,
    };

    let selected = unsafe { GetOpenFileNameW(&mut dialog) != 0 };
    if !selected {
        return None;
    }

    let len = file_buffer
        .iter()
        .position(|code_unit| *code_unit == 0)
        .unwrap_or(file_buffer.len());
    (len > 0).then(|| PathBuf::from(OsString::from_wide(&file_buffer[..len])))
}

#[cfg(target_os = "windows")]
fn copy_path_to_wide_buffer(path: &Path, buffer: &mut [u16]) {
    use std::os::windows::ffi::OsStrExt;

    let encoded = path.as_os_str().encode_wide().collect::<Vec<_>>();
    let len = encoded.len().min(buffer.len().saturating_sub(1));
    buffer[..len].copy_from_slice(&encoded[..len]);
    buffer[len] = 0;
}

#[cfg(not(target_os = "windows"))]
fn pick_font_file(_current: Option<&Path>) -> Option<PathBuf> {
    None
}

fn is_font_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            let extension = extension.to_ascii_lowercase();
            matches!(extension.as_str(), "ttf" | "otf" | "ttc")
        })
        .unwrap_or(false)
}

fn font_priority(name: &str) -> u8 {
    let name = name.to_ascii_lowercase();

    if name.contains("notosanssc") || name.contains("noto sans sc") {
        0
    } else if name.contains("msyh") || name.contains("simsun") || name.contains("simhei") {
        1
    } else if name.contains("deng") || name.contains("simkai") || name.contains("simfang") {
        2
    } else if name.contains("noto") || name.contains("mingliu") {
        3
    } else {
        10
    }
}

fn clamp_i32_to_i16(value: i32) -> i16 {
    value.clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

fn offset_i16_i32(value: i16, delta: i32) -> i16 {
    clamp_i32_to_i16(value as i32 + delta)
}

fn color_to_rgb(color: Color) -> Rgb888 {
    Rgb888::new(color.r, color.g, color.b)
}

fn rgb_to_color(color: Rgb888) -> Color {
    Color::rgb(color.r(), color.g(), color.b())
}

fn rust_string_literal(text: &str) -> String {
    format!("{text:?}")
}

fn rust_char_literal(ch: char) -> String {
    format!("{ch:?}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_response_uses_fallback_when_no_font_path_is_selected() {
        let settings = SimulatorSettings {
            font_source: FontSource::Custom,
            custom_font_path: String::new(),
            ..SimulatorSettings::default()
        };
        let mut cache = FontCache::default();

        let render = render_response(&settings, &[], &mut cache);

        assert_eq!(
            render.rgba.len(),
            (settings.canvas_width * settings.canvas_height * 4) as usize
        );
        assert_eq!(render.font.error.as_deref(), Some("No font path selected"));
        assert_eq!(render.font.index, "A");
    }
}
