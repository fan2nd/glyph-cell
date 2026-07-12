use core::convert::Infallible;
use std::{
    fs,
    ops::RangeInclusive,
    path::{Path, PathBuf},
};

use eframe::egui;
use embedded_graphics_core::{
    Drawable, Pixel,
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Point, Size},
    pixelcolor::{Rgb888, RgbColor},
};
use fontdue::{Font, FontSettings};
use glyph_cell::{
    Alignment, DebugBoxKind, DrawableText, FontData as GlyphCellFontData, Glyph, TextStyle,
};

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1180.0, 760.0]),
        ..Default::default()
    };

    eframe::run_native(
        "glyph-cell simulator",
        options,
        Box::new(|cc| Ok(Box::new(SimulatorApp::new(cc)))),
    )
}

struct SimulatorApp {
    text: String,
    layout_mode: LayoutMode,
    flow: FlowMode,
    alignment: AlignmentChoice,
    debug_overlays: DebugOverlays,
    font_source: FontSource,
    system_fonts: Vec<SystemFont>,
    selected_system_font: usize,
    custom_font_path: String,
    collection_index: u32,
    font_size: u16,
    font_cache: FontCache,
    ascii_width: u32,
    spacing: i32,
    line_spacing: i32,
    glyph_y_offsets: String,
    origin_x: i32,
    origin_y: i32,
    canvas_width: u32,
    canvas_height: u32,
    zoom: f32,
    glyph_color: egui::Color32,
    example_panel_open: bool,
}

impl SimulatorApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let system_fonts = discover_system_fonts();
        install_ui_font(&cc.egui_ctx, &system_fonts);

        let mut app = Self {
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
            font_cache: FontCache::default(),
            system_fonts,
            ascii_width: 10,
            spacing: 1,
            line_spacing: 0,
            glyph_y_offsets: String::new(),
            origin_x: 4,
            origin_y: 22,
            canvas_width: 180,
            canvas_height: 96,
            zoom: 4.0,
            glyph_color: egui::Color32::from_rgb(54, 187, 128),
            example_panel_open: true,
        };
        app.ensure_font();
        app
    }
}

impl eframe::App for SimulatorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::left("config")
            .resizable(true)
            .default_width(320.0)
            .show(ctx, |ui| {
                ui.heading("Parameters");
                ui.separator();
                self.font_controls(ui);
                ui.separator();
                self.text_controls(ui);
                ui.separator();
                self.layout_controls(ui);
                ui.separator();
                self.canvas_controls(ui);
            });

        self.ensure_font();

        self.example_panel(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Preview");
            ui.separator();
            self.preview(ui);
        });
    }
}

impl SimulatorApp {
    fn example_panel(&mut self, ctx: &egui::Context) {
        if self.example_panel_open {
            egui::SidePanel::right("example")
                .resizable(true)
                .default_width(410.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading("Example Code");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(">").on_hover_text("Hide example code").clicked() {
                                self.example_panel_open = false;
                            }
                        });
                    });
                    ui.separator();
                    let mut code = self.example_code();
                    ui.add(
                        egui::TextEdit::multiline(&mut code)
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(36)
                            .lock_focus(true),
                    );
                });
        } else {
            egui::SidePanel::right("example_collapsed")
                .resizable(false)
                .exact_width(36.0)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        if ui.button("<").on_hover_text("Show example code").clicked() {
                            self.example_panel_open = true;
                        }
                    });
                });
        }
    }

    fn font_controls(&mut self, ui: &mut egui::Ui) {
        let previous_font_source = self.font_source;

        egui::ComboBox::from_label("Font source")
            .selected_text(self.font_source.label())
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.font_source, FontSource::System, "System font");
                ui.selectable_value(
                    &mut self.font_source,
                    FontSource::Custom,
                    "Custom font file",
                );
            });

        if previous_font_source != FontSource::Custom && self.font_source == FontSource::Custom {
            self.choose_custom_font_file();
        }

        match self.font_source {
            FontSource::System => {
                if self.system_fonts.is_empty() {
                    ui.label("No system TTF/OTF/TTC fonts found.");
                } else {
                    let selected = self
                        .system_fonts
                        .get(self.selected_system_font)
                        .map(SystemFont::label)
                        .unwrap_or("No font");
                    egui::ComboBox::from_label("System font")
                        .selected_text(selected)
                        .show_ui(ui, |ui| {
                            for (index, font) in self.system_fonts.iter().enumerate() {
                                ui.selectable_value(
                                    &mut self.selected_system_font,
                                    index,
                                    font.label(),
                                );
                            }
                        });
                }

                if ui.button("Refresh system fonts").clicked() {
                    self.system_fonts = discover_system_fonts();
                    self.selected_system_font = self
                        .selected_system_font
                        .min(self.system_fonts.len().saturating_sub(1));
                    self.font_cache.invalidate();
                }
            }
            FontSource::Custom => {
                if ui.button("Choose font file...").clicked() {
                    self.choose_custom_font_file();
                }

                if self.current_custom_font_path().is_none() {
                    ui.label("No custom font selected.");
                }
            }
        }

        stepped_slider_u32(ui, "Collection index", &mut self.collection_index, 0..=8, 1);
        stepped_slider_u16(ui, "Raster size", &mut self.font_size, 4..=96, 1);
        stepped_slider_u32(ui, "ASCII cell width", &mut self.ascii_width, 1..=128, 1);
        ui.label("Glyph y_offset tweaks");
        ui.add(
            egui::TextEdit::multiline(&mut self.glyph_y_offsets)
                .desired_rows(3)
                .hint_text("g: -1\nA: 1"),
        );

        if let Some(path) = self.current_font_path() {
            ui.label(format!("Path: {}", path.display()));
        }

        if let Some(error) = self.font_cache.error.as_deref() {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        } else {
            ui.label(format!(
                "Loaded glyphs: {} | Index: {}",
                self.font_cache.data.glyphs.len(),
                self.font_cache.data.index
            ));
        }

        if !self.font_cache.missing_chars.is_empty() {
            ui.colored_label(
                egui::Color32::YELLOW,
                format!(
                    "Missing glyphs in selected font: {}",
                    self.font_cache.missing_chars
                ),
            );
        }
    }

    fn text_controls(&mut self, ui: &mut egui::Ui) {
        ui.label("Text");
        ui.add(
            egui::TextEdit::multiline(&mut self.text)
                .desired_rows(5)
                .hint_text("Text to render"),
        );

        egui::ComboBox::from_label("Flow")
            .selected_text(self.flow.label())
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.flow, FlowMode::Horizontal, "Horizontal");
                ui.selectable_value(&mut self.flow, FlowMode::Vertical, "Vertical");
            });

        egui::ComboBox::from_label("Alignment")
            .selected_text(self.alignment.label())
            .show_ui(ui, |ui| {
                for alignment in AlignmentChoice::ALL {
                    ui.selectable_value(&mut self.alignment, alignment, alignment.label());
                }
            });

        ui.horizontal(|ui| {
            ui.label("Glyph color");
            ui.color_edit_button_srgba(&mut self.glyph_color);
        });
    }

    fn layout_controls(&mut self, ui: &mut egui::Ui) {
        egui::ComboBox::from_label("Layout")
            .selected_text(self.layout_mode.label())
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.layout_mode, LayoutMode::Monospace, "Monospace");
                ui.selectable_value(
                    &mut self.layout_mode,
                    LayoutMode::Proportional,
                    "Proportional",
                );
            });

        stepped_slider_i32(ui, "Spacing", &mut self.spacing, -16..=48, 1);
        stepped_slider_i32(ui, "Line spacing", &mut self.line_spacing, -16..=64, 1);

        ui.horizontal(|ui| {
            ui.label("Debug boxes");
            toggle_debug_box(
                ui,
                "Cell",
                DebugBoxKind::Cell,
                &mut self.debug_overlays.cell,
            );
            toggle_debug_box(
                ui,
                "Glyph",
                DebugBoxKind::Glyph,
                &mut self.debug_overlays.glyph,
            );
        });
    }

    fn canvas_controls(&mut self, ui: &mut egui::Ui) {
        stepped_slider_i32(ui, "Origin X", &mut self.origin_x, -80..=240, 1);
        stepped_slider_i32(ui, "Origin Y", &mut self.origin_y, -80..=180, 1);
        stepped_slider_u32(ui, "Canvas width", &mut self.canvas_width, 32..=360, 1);
        stepped_slider_u32(ui, "Canvas height", &mut self.canvas_height, 24..=240, 1);
        stepped_slider_f32(ui, "Zoom", &mut self.zoom, 1.0..=16.0, 0.25);
    }

    fn preview(&self, ui: &mut egui::Ui) {
        let frame = self.render();
        let desired_size = egui::vec2(
            frame.width as f32 * self.zoom,
            frame.height as f32 * self.zoom,
        );
        let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
        let painter = ui.painter_at(rect);
        let background = ui.visuals().extreme_bg_color;

        painter.rect_filled(rect, 0.0, background);

        for y in 0..frame.height {
            for x in 0..frame.width {
                if let Some(color) = frame.pixel(x, y) {
                    let min = rect.min + egui::vec2(x as f32 * self.zoom, y as f32 * self.zoom);
                    let pixel_rect =
                        egui::Rect::from_min_size(min, egui::Vec2::splat(self.zoom.ceil()));
                    painter.rect_filled(pixel_rect, 0.0, rgb_to_egui(color));
                }
            }
        }

        if self.zoom >= 6.0 {
            let stroke =
                egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color);
            for x in 0..=frame.width {
                let px = rect.left() + x as f32 * self.zoom;
                painter.line_segment(
                    [egui::pos2(px, rect.top()), egui::pos2(px, rect.bottom())],
                    stroke,
                );
            }
            for y in 0..=frame.height {
                let py = rect.top() + y as f32 * self.zoom;
                painter.line_segment(
                    [egui::pos2(rect.left(), py), egui::pos2(rect.right(), py)],
                    stroke,
                );
            }
        }

        ui.add_space(8.0);
        let measurement = self.measurement();
        ui.label(format!(
            "Measured text: {} x {} px | Canvas: {} x {} px",
            measurement.width, measurement.height, frame.width, frame.height
        ));
    }

    fn render(&self) -> FrameBuffer {
        let mut frame = FrameBuffer::new(self.canvas_width, self.canvas_height);
        let font_data = self.font_cache.data.as_font_data();
        let text = self.drawable_text(&font_data, self.glyph_color);
        let _ = text.draw(&mut frame);

        if self.debug_overlays.has_any() {
            for kind in self.debug_overlays.kinds() {
                let overlay = self.drawable_text(&font_data, debug_box_color(kind));
                let _ = overlay.draw_debug_boxes(&mut frame, kind);
            }
        }

        frame
    }

    fn measurement(&self) -> Size {
        let font_data = self.font_cache.data.as_font_data();
        self.drawable_text(&font_data, self.glyph_color).measure()
    }

    fn drawable_text<'a>(
        &'a self,
        font_data: &'a GlyphCellFontData<'a>,
        color: egui::Color32,
    ) -> DrawableText<'a, Rgb888> {
        let style = self
            .style(color)
            .align(self.alignment.to_glyph_cell_alignment());
        let text = DrawableText::new(font_data, &self.text, style)
            .at(Point::new(self.origin_x, self.origin_y));

        match self.flow {
            FlowMode::Horizontal => text.horizontal(),
            FlowMode::Vertical => text.vertical(),
        }
    }

    fn style(&self, color: egui::Color32) -> TextStyle<Rgb888> {
        let style = TextStyle::new(egui_to_rgb(color));
        match self.layout_mode {
            LayoutMode::Monospace => style.monospace_with_spacing(self.spacing, self.line_spacing),
            LayoutMode::Proportional => {
                style.proportional_with_line_spacing(self.spacing, self.line_spacing)
            }
        }
    }

    fn ensure_font(&mut self) {
        let Some(path) = self.current_font_path() else {
            self.font_cache.error = Some("No font path selected".to_owned());
            return;
        };

        let index = glyph_index_from_text(&self.text);
        let key = FontBuildKey {
            path: path.to_string_lossy().into_owned(),
            collection_index: self.collection_index,
            size: self.font_size,
            ascii_width: self.ascii_width,
            index,
            y_offsets: self.glyph_y_offsets.clone(),
        };

        if self.font_cache.key.as_ref() == Some(&key) {
            return;
        }

        match build_font_data(
            &path,
            self.collection_index,
            self.font_size,
            self.ascii_width as u16,
            &key.index,
            &self.glyph_y_offsets,
        ) {
            Ok(build) => {
                self.font_cache.key = Some(key);
                self.font_cache.data = build.data;
                self.font_cache.missing_chars = build.missing_chars;
                self.font_cache.error = None;
            }
            Err(err) => {
                self.font_cache.key = Some(key);
                self.font_cache.error = Some(err);
            }
        }
    }

    fn current_font_path(&self) -> Option<PathBuf> {
        match self.font_source {
            FontSource::System => self
                .system_fonts
                .get(self.selected_system_font)
                .map(|font| font.path.clone()),
            FontSource::Custom => self.current_custom_font_path(),
        }
    }

    fn current_custom_font_path(&self) -> Option<PathBuf> {
        let trimmed = self.custom_font_path.trim();
        (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
    }

    fn choose_custom_font_file(&mut self) {
        if let Some(path) = pick_font_file(self.current_custom_font_path().as_deref()) {
            self.custom_font_path = path.to_string_lossy().into_owned();
            self.font_cache.invalidate();
        }
    }

    fn example_code(&self) -> String {
        let alignment = self.alignment.code_name();
        let text = rust_string_literal(&self.text);
        let index = rust_string_literal(&self.font_cache.data.index);
        let path = self
            .current_font_path()
            .map(|path| rust_string_literal(&path.to_string_lossy()))
            .unwrap_or_else(|| "\"path/to/font.ttf\"".to_owned());
        let layout = match self.layout_mode {
            LayoutMode::Monospace if self.spacing == 0 && self.line_spacing == 0 => {
                "    .monospace()".to_owned()
            }
            LayoutMode::Monospace => format!(
                "    .monospace_with_spacing({}, {})",
                self.spacing, self.line_spacing
            ),
            LayoutMode::Proportional => format!(
                "    .proportional_with_line_spacing({}, {})",
                self.spacing, self.line_spacing
            ),
        };
        let flow = match self.flow {
            FlowMode::Horizontal => String::new(),
            FlowMode::Vertical => "    .vertical()\n".to_owned(),
        };
        let y_offsets = self.example_y_offsets();

        format!(
            "use embedded_graphics_core::geometry::Point;\nuse embedded_graphics_core::pixelcolor::Rgb888;\nuse glyph_cell::{{font_data, Alignment, DrawableText, FontData, TextStyle}};\n\nconst FONT: FontData<'static> = font_data! {{\n    size: {},\n    ascii_width: {},\n    path: {},\n    index: {},\n{}}};\n\nlet style = TextStyle::new(Rgb888::new({}, {}, {}))\n{}\n    .align(Alignment::{});\n\nDrawableText::new(&FONT, {}, style)\n{}    .at(Point::new({}, {}))\n    .draw(&mut display)?;\n",
            self.font_size,
            self.ascii_width,
            path,
            index,
            y_offsets,
            self.glyph_color.r(),
            self.glyph_color.g(),
            self.glyph_color.b(),
            layout,
            alignment,
            text,
            flow,
            self.origin_x,
            self.origin_y
        )
    }

    fn example_y_offsets(&self) -> String {
        let Ok(offsets) = parse_y_offset_tweaks(&self.glyph_y_offsets, &self.font_cache.data.index)
        else {
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
}

fn stepped_slider_u32(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut u32,
    range: RangeInclusive<u32>,
    step: u32,
) {
    ui.horizontal(|ui| {
        ui.add(
            egui::Slider::new(value, range.clone())
                .text(label)
                .show_value(false),
        );
        number_stepper_u32(ui, value, range, step);
    });
}

fn stepped_slider_u16(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut u16,
    range: RangeInclusive<u16>,
    step: u16,
) {
    ui.horizontal(|ui| {
        ui.add(
            egui::Slider::new(value, range.clone())
                .text(label)
                .show_value(false),
        );
        number_stepper_u16(ui, value, range, step);
    });
}

fn stepped_slider_i32(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut i32,
    range: RangeInclusive<i32>,
    step: i32,
) {
    ui.horizontal(|ui| {
        ui.add(
            egui::Slider::new(value, range.clone())
                .text(label)
                .show_value(false),
        );
        number_stepper_i32(ui, value, range, step);
    });
}

fn stepped_slider_f32(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: RangeInclusive<f32>,
    step: f32,
) {
    ui.horizontal(|ui| {
        ui.add(
            egui::Slider::new(value, range.clone())
                .text(label)
                .show_value(false),
        );
        number_stepper_f32(ui, value, range, step);
    });
}

fn number_stepper_u32(ui: &mut egui::Ui, value: &mut u32, range: RangeInclusive<u32>, step: u32) {
    ui.add(egui::DragValue::new(value).speed(step as f64).range(range));
}

fn number_stepper_u16(ui: &mut egui::Ui, value: &mut u16, range: RangeInclusive<u16>, step: u16) {
    ui.add(egui::DragValue::new(value).speed(step as f64).range(range));
}

fn number_stepper_i32(ui: &mut egui::Ui, value: &mut i32, range: RangeInclusive<i32>, step: i32) {
    ui.add(egui::DragValue::new(value).speed(step as f64).range(range));
}

fn number_stepper_f32(ui: &mut egui::Ui, value: &mut f32, range: RangeInclusive<f32>, step: f32) {
    ui.add(
        egui::DragValue::new(value)
            .speed(step as f64)
            .range(range)
            .max_decimals(2),
    );
}

fn toggle_debug_box(ui: &mut egui::Ui, label: &str, kind: DebugBoxKind, enabled: &mut bool) {
    let text = egui::RichText::new(label)
        .color(debug_box_color(kind))
        .strong();
    if ui.add(egui::Button::new(text).selected(*enabled)).clicked() {
        *enabled = !*enabled;
    }
}

fn debug_box_color(kind: DebugBoxKind) -> egui::Color32 {
    match kind {
        DebugBoxKind::Cell => egui::Color32::from_rgb(72, 166, 255),
        DebugBoxKind::Glyph => egui::Color32::from_rgb(233, 96, 154),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FontSource {
    System,
    Custom,
}

impl FontSource {
    fn label(self) -> &'static str {
        match self {
            Self::System => "System font",
            Self::Custom => "Custom font file",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayoutMode {
    Monospace,
    Proportional,
}

impl LayoutMode {
    fn label(self) -> &'static str {
        match self {
            Self::Monospace => "Monospace",
            Self::Proportional => "Proportional",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlowMode {
    Horizontal,
    Vertical,
}

impl FlowMode {
    fn label(self) -> &'static str {
        match self {
            Self::Horizontal => "Horizontal",
            Self::Vertical => "Vertical",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    const ALL: [Self; 9] = [
        Self::TopLeft,
        Self::TopCenter,
        Self::TopRight,
        Self::MiddleLeft,
        Self::Center,
        Self::MiddleRight,
        Self::BottomLeft,
        Self::BottomCenter,
        Self::BottomRight,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::TopLeft => "Top left",
            Self::TopCenter => "Top center",
            Self::TopRight => "Top right",
            Self::MiddleLeft => "Middle left",
            Self::Center => "Center",
            Self::MiddleRight => "Middle right",
            Self::BottomLeft => "Bottom left",
            Self::BottomCenter => "Bottom center",
            Self::BottomRight => "Bottom right",
        }
    }

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
}

struct FrameBuffer {
    width: u32,
    height: u32,
    pixels: Vec<Option<Rgb888>>,
}

impl FrameBuffer {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![None; width as usize * height as usize],
        }
    }

    fn pixel(&self, x: u32, y: u32) -> Option<Rgb888> {
        self.pixels[(y * self.width + x) as usize]
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
            if point.x < 0 || point.y < 0 {
                continue;
            }

            let x = point.x as u32;
            let y = point.y as u32;
            if x >= self.width || y >= self.height {
                continue;
            }

            self.pixels[(y * self.width + x) as usize] = Some(color);
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
    let font = Font::from_bytes(
        bytes,
        FontSettings {
            collection_index,
            ..FontSettings::default()
        },
    )
    .map_err(|err| format!("Failed to parse font file: {err}"))?;

    let mut glyphs = Vec::new();
    let mut bitmap = Vec::new();
    let mut missing_chars = String::new();

    for ch in index.chars() {
        if !font.has_glyph(ch) {
            missing_chars.push(ch);
        }

        let (metrics, coverage) = font.rasterize(ch, size as f32);
        let width = metrics.width.max(1).min(u16::MAX as usize) as u16;
        let height = metrics.height.max(1).min(u16::MAX as usize) as u16;
        let pixels = if metrics.width == 0 || metrics.height == 0 {
            vec![false; width as usize * height as usize]
        } else {
            coverage.into_iter().map(|alpha| alpha >= 96).collect()
        };
        let bitmap_offset = bitmap.len() as u32;
        pack_bpp1(&pixels, &mut bitmap);

        glyphs.push(Glyph {
            bitmap_offset,
            width,
            height,
            cell_width: 0,
            x_offset: 0,
            y_offset: glyph_y_offset(metrics.height, metrics.ymin),
            x_min: clamp_i32_to_i16(metrics.xmin),
            y_min: clamp_i32_to_i16(metrics.ymin),
            advance_width: advance_width_pixels(metrics.advance_width),
        });
    }
    apply_auto_y_offsets(size, index, &mut glyphs);
    apply_y_offset_tweaks(&mut glyphs, index, y_offset_tweaks)?;
    apply_cell_offsets(size, ascii_width, index, &mut glyphs);

    Ok(FontBuild {
        data: OwnedFontData {
            index: index.to_owned(),
            size,
            ascii_width,
            bitmap,
            glyphs,
        },
        missing_chars,
    })
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

fn glyph_y_offset(height: usize, y_min: i32) -> i16 {
    clamp_i32_to_i16(height as i32 + y_min)
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

fn pack_bpp1(pixels: &[bool], out: &mut Vec<u8>) {
    let mut byte = 0u8;
    for (index, pixel) in pixels.iter().enumerate() {
        if *pixel {
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

fn install_ui_font(ctx: &egui::Context, system_fonts: &[SystemFont]) {
    let Some(font) = system_fonts
        .iter()
        .find(|font| font_priority(&font.name) == 0)
        .or_else(|| system_fonts.first())
    else {
        return;
    };

    let Ok(bytes) = fs::read(&font.path) else {
        return;
    };

    let mut definitions = egui::FontDefinitions::default();
    definitions.font_data.insert(
        "glyph-cell-ui-cjk".to_owned(),
        egui::FontData::from_owned(bytes),
    );

    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        if let Some(fonts) = definitions.families.get_mut(&family) {
            fonts.insert(0, "glyph-cell-ui-cjk".to_owned());
        }
    }

    ctx.set_fonts(definitions);
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

fn advance_width_pixels(advance_width: f32) -> u16 {
    if !advance_width.is_finite() || advance_width <= 0.0 {
        0
    } else if advance_width >= u16::MAX as f32 {
        u16::MAX
    } else {
        advance_width.ceil() as u16
    }
}

fn clamp_i32_to_i16(value: i32) -> i16 {
    value.clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

fn offset_i16_i32(value: i16, delta: i32) -> i16 {
    clamp_i32_to_i16(value as i32 + delta)
}

fn egui_to_rgb(color: egui::Color32) -> Rgb888 {
    Rgb888::new(color.r(), color.g(), color.b())
}

fn rgb_to_egui(color: Rgb888) -> egui::Color32 {
    egui::Color32::from_rgb(color.r(), color.g(), color.b())
}

fn rust_string_literal(text: &str) -> String {
    format!("{text:?}")
}

fn rust_char_literal(ch: char) -> String {
    format!("{ch:?}")
}
