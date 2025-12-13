// Binary file fallback handler

use crate::entry::FileEntry;
use crate::view::preview::handler::{PreviewContext, PreviewHandler};
use eframe::egui;

pub struct BinaryPreviewHandler;

impl BinaryPreviewHandler {
    pub fn new() -> Self {
        Self
    }
}

impl PreviewHandler for BinaryPreviewHandler {
    fn name(&self) -> &str {
        "binary"
    }

    fn can_preview(&self, entry: &FileEntry) -> bool {
        // Binary handler is the fallback - it can "preview" any file
        !entry.is_dir
    }

    fn render(
        &self,
        ui: &mut egui::Ui,
        entry: &FileEntry,
        _context: &PreviewContext,
    ) -> Result<(), String> {
        ui.centered_and_justified(|ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.label(egui::RichText::new("📦 Binary File").size(18.0));
                ui.add_space(10.0);
                ui.label("Preview not available for this file type");
                ui.add_space(5.0);
                ui.label(format!("Extension: .{}", entry.extension));
            });
        });
        Ok(())
    }

    fn priority(&self) -> i32 {
        1000 // Lowest priority - fallback handler
    }
}
