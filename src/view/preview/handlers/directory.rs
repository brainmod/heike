// Directory preview handler

use crate::entry::FileEntry;
use crate::io::directory::read_directory;
use crate::style;
use crate::view::preview::handler::{PreviewContext, PreviewHandler};
use eframe::egui;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

type DirCacheEntry = (PathBuf, SystemTime, bool, Arc<Vec<FileEntry>>);

pub struct DirectoryPreviewHandler {
    // Last previewed directory, keyed by (path, mtime, show_hidden), so the
    // directory is read once per selection instead of every frame
    cache: Mutex<Option<DirCacheEntry>>,
}

impl DirectoryPreviewHandler {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(None),
        }
    }

    fn entries_for(
        &self,
        entry: &FileEntry,
        show_hidden: bool,
    ) -> Result<Arc<Vec<FileEntry>>, String> {
        // Stat the directory itself: its mtime changes when children are added/removed,
        // which the non-recursive watcher on the current directory does not report
        let mtime = std::fs::metadata(&entry.path)
            .and_then(|m| m.modified())
            .unwrap_or(entry.modified);
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((path, modified, hidden, entries)) = cache.as_ref() {
            if *path == entry.path && *modified == mtime && *hidden == show_hidden {
                return Ok(entries.clone());
            }
        }
        let entries = Arc::new(
            read_directory(&entry.path, show_hidden)
                .map_err(|e| format!("Cannot read directory: {}", e))?,
        );
        *cache = Some((entry.path.clone(), mtime, show_hidden, entries.clone()));
        Ok(entries)
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

        let entries = self.entries_for(entry, context.show_hidden)?;

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
