// Preview handler trait and context for extensible file preview system

use crate::entry::FileEntry;
use crate::style::Theme;
use eframe::egui;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

/// Context passed to preview handlers containing shared resources
pub struct PreviewContext<'a> {
    pub syntax_set: &'a SyntaxSet,
    pub theme_set: &'a ThemeSet,
    pub theme: Theme,
    pub show_hidden: bool,
    pub last_selection_change: Instant,
    pub directory_selections: &'a HashMap<PathBuf, usize>,
    pub next_navigation: &'a std::cell::RefCell<Option<PathBuf>>,
    pub pending_selection: &'a std::cell::RefCell<Option<PathBuf>>,
}

/// Trait for file preview handlers
///
/// Allows modular, extensible file preview system where different handlers
/// can be enabled/disabled via configuration, and new handlers can be added
/// without modifying the core preview system.
pub trait PreviewHandler: Send + Sync {
    /// Name of this handler (for configuration and debugging)
    fn name(&self) -> &str;

    /// Check if this handler can preview the given file
    ///
    /// Returns true if this handler should be used for the file.
    /// Handlers are checked in priority order.
    fn can_preview(&self, entry: &FileEntry) -> bool;

    /// Render the preview for the given file
    ///
    /// Should render the preview into the provided UI context.
    /// Returns Ok(()) on success, or Err(message) on failure.
    fn render(
        &self,
        ui: &mut egui::Ui,
        entry: &FileEntry,
        context: &PreviewContext,
    ) -> Result<(), String>;

    /// Priority of this handler (lower = higher priority)
    ///
    /// Used to determine the order in which handlers are checked.
    /// Default is 100. Specific handlers (e.g., markdown) should have
    /// lower priority than generic handlers (e.g., text).
    fn priority(&self) -> i32 {
        100
    }

    /// Whether this handler is enabled by default
    fn enabled_by_default(&self) -> bool {
        true
    }
}
