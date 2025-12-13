// Archive preview handler (zip, tar, gz, etc.)

use crate::entry::FileEntry;
use crate::view::preview::handler::{PreviewContext, PreviewHandler};
use eframe::egui;
use flate2::read::GzDecoder;
use std::fs;
use tar::Archive;
use zip::ZipArchive;

pub struct ArchivePreviewHandler;

impl ArchivePreviewHandler {
    pub fn new() -> Self {
        Self
    }

    const MAX_PREVIEW_ITEMS: usize = 100;

    fn is_archive_extension(ext: &str) -> bool {
        matches!(ext, "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz")
    }
}

impl PreviewHandler for ArchivePreviewHandler {
    fn name(&self) -> &str {
        "archive"
    }

    fn can_preview(&self, entry: &FileEntry) -> bool {
        Self::is_archive_extension(&entry.extension)
    }

    fn render(
        &self,
        ui: &mut egui::Ui,
        entry: &FileEntry,
        _context: &PreviewContext,
    ) -> Result<(), String> {
        let result = if entry.extension == "zip" {
            fs::File::open(&entry.path).ok().and_then(|file| {
                ZipArchive::new(file).ok().map(|mut archive| {
                    let total = archive.len();
                    let mut items = Vec::new();
                    for i in 0..total.min(Self::MAX_PREVIEW_ITEMS) {
                        if let Ok(file) = archive.by_index(i) {
                            items.push((file.name().to_string(), file.size(), file.is_dir()));
                        }
                    }
                    (items, total)
                })
            })
        } else if entry.extension == "tar" || entry.extension == "gz" || entry.extension == "tgz" {
            fs::File::open(&entry.path).ok().and_then(|file| {
                let reader: Box<dyn std::io::Read> =
                    if entry.extension == "gz" || entry.extension == "tgz" {
                        Box::new(GzDecoder::new(file))
                    } else {
                        Box::new(file)
                    };

                Archive::new(reader).entries().ok().map(|entries| {
                    let items: Vec<_> = entries
                        .filter_map(|e| e.ok())
                        .take(Self::MAX_PREVIEW_ITEMS)
                        .map(|e| {
                            let size = e.header().size().unwrap_or(0);
                            let path = e
                                .path()
                                .ok()
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_default();
                            let is_dir = e.header().entry_type().is_dir();
                            (path, size, is_dir)
                        })
                        .collect();
                    let total = items.len();
                    (items, total)
                })
            })
        } else {
            None
        };

        match result {
            Some((items, total)) => {
                if items.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.label("Empty archive");
                    });
                    return Ok(());
                }

                ui.label(format!(
                    "Archive contains {} items{}:",
                    total,
                    if total > Self::MAX_PREVIEW_ITEMS {
                        format!(" (showing first {})", Self::MAX_PREVIEW_ITEMS)
                    } else {
                        String::new()
                    }
                ));
                ui.separator();

                egui::ScrollArea::vertical()
                    .id_salt("preview_archive")
                    .auto_shrink([false, false])
                    .max_height(ui.available_height())
                    .show(ui, |ui| {
                        ui.set_max_width(ui.available_width());
                        use egui_extras::{Column, TableBuilder};
                        TableBuilder::new(ui)
                            .striped(true)
                            .resizable(false)
                            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                            .column(Column::auto().at_least(30.0))
                            .column(Column::remainder().clip(true))
                            .column(Column::auto().at_least(80.0))
                            .body(|body| {
                                body.rows(20.0, items.len(), |mut row| {
                                    let (name, size, is_dir) = &items[row.index()];
                                    row.col(|ui| {
                                        let icon = if *is_dir { "\u{f07c}" } else { "\u{f15b}" };
                                        ui.label(icon);
                                    });
                                    row.col(|ui| {
                                        ui.label(name);
                                    });
                                    row.col(|ui| {
                                        if !*is_dir {
                                            ui.label(bytesize::ByteSize(*size).to_string());
                                        }
                                    });
                                });
                            });
                    });
                Ok(())
            }
            None => Err("Failed to read archive".to_string()),
        }
    }

    fn priority(&self) -> i32 {
        30 // Medium-high priority
    }
}
