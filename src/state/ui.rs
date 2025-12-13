// UI state - presentation and layout settings
use crate::style::Theme;
use crate::state::{SortOptions, SearchOptions};
use std::time::Instant;
use eframe::egui;

pub struct UIState {
    pub show_hidden: bool,
    pub theme: Theme,
    pub sort_options: SortOptions,
    pub error_message: Option<(String, Instant)>,
    pub info_message: Option<(String, Instant)>,
    pub panel_widths: [f32; 2],
    pub dragging_divider: Option<usize>,
    pub last_screen_size: egui::Vec2,
    pub is_loading: bool,
    pub search_query: String,
    pub search_options: SearchOptions,
    pub search_in_progress: bool,
    pub search_file_count: usize,
}

impl UIState {
    pub fn new(theme: Theme, sort_options: SortOptions) -> Self {
        Self {
            show_hidden: false,
            theme,
            sort_options,
            error_message: None,
            info_message: None,
            panel_widths: [200.0, 350.0],
            dragging_divider: None,
            last_screen_size: egui::Vec2::ZERO,
            is_loading: false,
            search_query: String::new(),
            search_options: SearchOptions::default(),
            search_in_progress: false,
            search_file_count: 0,
        }
    }

    pub fn set_error(&mut self, message: String) {
        self.error_message = Some((message, Instant::now()));
    }

    pub fn set_info(&mut self, message: String) {
        self.info_message = Some((message, Instant::now()));
    }

    pub fn clear_expired_messages(&mut self, timeout_secs: u64) {
        if let Some((_, time)) = &self.error_message {
            if time.elapsed().as_secs() >= timeout_secs {
                self.error_message = None;
            }
        }
        if let Some((_, time)) = &self.info_message {
            if time.elapsed().as_secs() >= timeout_secs {
                self.info_message = None;
            }
        }
    }
}
