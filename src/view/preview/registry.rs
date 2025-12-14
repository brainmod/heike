// Preview handler registry for managing and dispatching preview handlers

use super::handler::{PreviewContext, PreviewHandler};
use crate::entry::FileEntry;
use eframe::egui;
use std::collections::HashSet;
use std::sync::Arc;

/// Registry for managing preview handlers
pub struct PreviewRegistry {
    handlers: Vec<Arc<dyn PreviewHandler>>,
    enabled_handlers: HashSet<String>,
}

impl PreviewRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            enabled_handlers: HashSet::new(),
        }
    }

    /// Register a preview handler
    ///
    /// Handlers are automatically sorted by priority after registration.
    pub fn register(&mut self, handler: Arc<dyn PreviewHandler>) {
        // Add to enabled set if enabled by default
        if handler.enabled_by_default() {
            self.enabled_handlers.insert(handler.name().to_string());
        }
        self.handlers.push(handler);
        // Sort handlers by priority (lower priority value = checked first)
        self.handlers.sort_by_key(|h| h.priority());
    }

    /// Check if a handler is enabled
    pub fn is_enabled(&self, name: &str) -> bool {
        self.enabled_handlers.contains(name)
    }

    /// Set enabled handlers from configuration
    pub fn set_enabled_handlers(&mut self, enabled: Vec<String>) {
        self.enabled_handlers = enabled.into_iter().collect();
    }

    /// Render preview using the first matching enabled handler
    ///
    /// Returns true if a handler was found and rendered successfully.
    pub fn render_preview(
        &self,
        ui: &mut egui::Ui,
        entry: &FileEntry,
        context: &PreviewContext,
    ) -> bool {
        for handler in &self.handlers {
            // Skip disabled handlers
            if !self.is_enabled(handler.name()) {
                continue;
            }

            // Check if handler can preview this file
            if handler.can_preview(entry) {
                match handler.render(ui, entry, context) {
                    Ok(()) => return true,
                    Err(e) => {
                        ui.colored_label(
                            egui::Color32::RED,
                            format!("Preview error ({}): {}", handler.name(), e),
                        );
                        return true; // Still handled, even if error
                    }
                }
            }
        }
        false
    }

    /// Get list of enabled handler names
    pub fn enabled_handler_names(&self) -> Vec<String> {
        self.enabled_handlers.iter().cloned().collect()
    }
}

impl Default for PreviewRegistry {
    fn default() -> Self {
        Self::new()
    }
}
