use eframe::egui;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Theme {
    Light,
    Dark,
}

// --- Sizing ---
pub const DIVIDER_WIDTH: f32 = 4.0;

// --- Panel constraints ---
pub const PARENT_MIN: f32 = 100.0;
pub const PARENT_MAX: f32 = 400.0;
pub const PREVIEW_MIN: f32 = 150.0;
pub const PREVIEW_MAX: f32 = 800.0;

// --- Modals ---
pub const MODAL_MIN_WIDTH: f32 = 300.0;
pub const MODAL_MAX_WIDTH: f32 = 500.0;
pub const MODAL_WIDTH_RATIO: f32 = 0.6;
pub const MODAL_HEIGHT_RATIO: f32 = 0.8;

// --- Timing ---
pub const DOUBLE_PRESS_MS: u64 = 500;
pub const KEY_SEQUENCE_DELAY_MS: u64 = 10; // Delay between keys in sequences like 'g' + key
pub const MESSAGE_TIMEOUT_SECS: u64 = 5;

// --- Preview limits ---
pub const MAX_PREVIEW_SIZE: u64 = 10 * 1024 * 1024;

// --- Helper functions ---

pub fn modal_width(ctx: &egui::Context) -> f32 {
    let width = ctx.input(|i| i.viewport().inner_rect.map(|r| r.width()).unwrap_or(800.0));
    (width * MODAL_WIDTH_RATIO).clamp(MODAL_MIN_WIDTH, MODAL_MAX_WIDTH)
}

pub fn modal_max_height(ctx: &egui::Context) -> f32 {
    let height = ctx.input(|i| i.viewport().inner_rect.map(|r| r.height()).unwrap_or(600.0));
    height * MODAL_HEIGHT_RATIO
}

pub fn truncated_label(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) -> egui::Response {
    ui.add(egui::Label::new(text).truncate())
}

pub fn truncated_label_with_sense(
    ui: &mut egui::Ui,
    text: impl Into<egui::WidgetText>,
    sense: egui::Sense,
) -> egui::Response {
    ui.add(egui::Label::new(text).truncate().sense(sense))
}
