// Layout constants for Heike file manager

use eframe::egui;

// --- Sizing ---
pub const ICON_SIZE: f32 = 14.0;
pub const ICON_COL_WIDTH: f32 = 30.0;
pub const ROW_HEIGHT: f32 = 24.0;
pub const HEADER_HEIGHT: f32 = 20.0;
pub const DIVIDER_WIDTH: f32 = 4.0;

// --- Panel constraints ---
pub const PARENT_MIN: f32 = 100.0;
pub const PARENT_MAX: f32 = 400.0;
pub const PARENT_DEFAULT: f32 = 200.0;
pub const PREVIEW_MIN: f32 = 150.0;
pub const PREVIEW_MAX: f32 = 800.0;
pub const PREVIEW_DEFAULT: f32 = 350.0;

// --- Modals ---
pub const MODAL_MIN_WIDTH: f32 = 300.0;
pub const MODAL_MAX_WIDTH: f32 = 500.0;
pub const MODAL_WIDTH_RATIO: f32 = 0.6;
pub const MODAL_HEIGHT_RATIO: f32 = 0.8;

// --- Timing ---
pub const PREVIEW_DEBOUNCE_MS: u64 = 200;
pub const DOUBLE_PRESS_MS: u64 = 500; // for gg
pub const MESSAGE_TIMEOUT_SECS: u64 = 5;

// --- Preview limits ---
pub const HEX_PREVIEW_BYTES: usize = 512;
pub const TEXT_PREVIEW_LIMIT: usize = 100_000; // chars
pub const ARCHIVE_PREVIEW_ITEMS: usize = 100;
pub const MAX_PREVIEW_SIZE: u64 = 10 * 1024 * 1024; // 10MB

// --- Helper functions ---

/// Calculate responsive modal width based on screen size
pub fn modal_width(ctx: &egui::Context) -> f32 {
    (ctx.input(|i| i.screen_rect().width()) * MODAL_WIDTH_RATIO).clamp(MODAL_MIN_WIDTH, MODAL_MAX_WIDTH)
}

/// Calculate maximum modal height based on screen size
pub fn modal_max_height(ctx: &egui::Context) -> f32 {
    ctx.input(|i| i.screen_rect().height()) * MODAL_HEIGHT_RATIO
}

/// Render a label that truncates overflowing text with an ellipsis.
pub fn truncated_label(
    ui: &mut egui::Ui,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    ui.add(egui::Label::new(text).truncate())
}

/// Render a label that truncates overflowing text with an ellipsis and uses the provided sense.
pub fn truncated_label_with_sense(
    ui: &mut egui::Ui,
    text: impl Into<egui::WidgetText>,
    sense: egui::Sense,
) -> egui::Response {
    ui.add(egui::Label::new(text).truncate().sense(sense))
}
