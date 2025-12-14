// Directory preview handler

use crate::entry::FileEntry;
use crate::io::directory::read_directory;
use crate::style;
use crate::view::preview::handler::{PreviewContext, PreviewHandler};
use eframe::egui;
use std::time::Duration;

pub struct DirectoryPreviewHandler;

impl DirectoryPreviewHandler {
    pub fn new() -> Self {
        Self
    }
}

impl PreviewHandler for DirectoryPreviewHandler {
    fn name(&self) -> &str {
        "directory"
    }

    fn can_preview(&self, entry: &FileEntry) -> bool {
        entry.is_dir
    }

    fn render(
        &self,
        ui: &mut egui::Ui,
        entry: &FileEntry,
        context: &PreviewContext,
    ) -> Result<(), String> {
        // Debounce directory loading
        if context.last_selection_change.elapsed() <= Duration::from_millis(200) {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return Ok(());
        }

        let entries = read_directory(&entry.path, context.show_hidden)
            .map_err(|e| format!("Cannot read directory: {}", e))?;

        let accent = egui::Color32::from_rgb(120, 180, 255);
        let highlighted_index = context.directory_selections.get(&entry.path).copied();

        egui::ScrollArea::vertical()
            .id_salt("preview_dir")
            .auto_shrink([false, false])
            .max_height(ui.available_height())
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width());
                let default_color = ui.visuals().text_color();
                use egui_extras::{Column, TableBuilder};
                TableBuilder::new(ui)
                    .striped(true)
                    .resizable(false)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::auto().at_least(30.0))
                    .column(Column::remainder().clip(true))
                    .body(|body| {
                        body.rows(24.0, entries.len(), |mut row| {
                            let row_index = row.index();
                            let preview_entry = &entries[row_index];
                            let is_highlighted = highlighted_index == Some(row_index);
                            let text_color = if is_highlighted || preview_entry.is_dir {
                                accent
                            } else {
                                default_color
                            };
                            row.col(|ui| {
                                ui.label(
                                    egui::RichText::new(preview_entry.get_icon())
                                        .size(14.0)
                                        .color(text_color),
                                );
                            });
                            row.col(|ui| {
                                let response = style::truncated_label_with_sense(
                                    ui,
                                    egui::RichText::new(preview_entry.display_name())
                                        .color(text_color),
                                    egui::Sense::click(),
                                );
                                if response.clicked() {
                                    *context.next_navigation.borrow_mut() =
                                        Some(entry.path.clone());
                                    *context.pending_selection.borrow_mut() =
                                        Some(preview_entry.path.clone());
                                }
                            });
                        });
                    });
            });

        Ok(())
    }

    fn priority(&self) -> i32 {
        5 // Very high priority - directories are common
    }
}
