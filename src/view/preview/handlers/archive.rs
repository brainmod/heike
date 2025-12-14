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
    const ARCHIVE_SIZE_LIMIT: u64 = 100 * 1024 * 1024; // 100MB

    fn is_archive_extension(ext: &str) -> bool {
        matches!(ext, "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz")
    }

    /// Extract archive contents as a cacheable string
    /// Format: "total:<N>|+" on first line, then "D|F\tname\tsize" per item
    fn extract_contents(entry: &FileEntry) -> Result<String, String> {
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
                    (items, Some(total))
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
                        .take(Self::MAX_PREVIEW_ITEMS + 1)
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

                    let has_more = items.len() > Self::MAX_PREVIEW_ITEMS;
                    let shown_count = items.len().min(Self::MAX_PREVIEW_ITEMS);
                    let items_to_show: Vec<_> = if has_more {
                        items.into_iter().take(Self::MAX_PREVIEW_ITEMS).collect()
                    } else {
                        items
                    };

                    (items_to_show, if has_more { None } else { Some(shown_count) })
                })
            })
        } else {
            None
        };

        match result {
            Some((items, total)) => {
                let mut lines = Vec::with_capacity(items.len() + 1);
                // First line: total count
                match total {
                    Some(t) => lines.push(format!("total:{}", t)),
                    None => lines.push(format!("total:{}+", items.len())),
                }
                // Subsequent lines: type\tname\tsize
                for (name, size, is_dir) in items {
                    let type_char = if is_dir { 'D' } else { 'F' };
                    lines.push(format!("{}\t{}\t{}", type_char, name, size));
                }
                Ok(lines.join("\n"))
            }
            None => Err("Failed to read archive".to_string()),
        }
    }

    /// Parse cached content back into items and total
    fn parse_cached(content: &str) -> Option<(Vec<(String, u64, bool)>, Option<usize>)> {
        let mut lines = content.lines();
        let first_line = lines.next()?;

        let total = if let Some(rest) = first_line.strip_prefix("total:") {
            if rest.ends_with('+') {
                None // Unknown total
            } else {
                rest.parse().ok()
            }
        } else {
            return None;
        };

        let items: Vec<_> = lines
            .filter_map(|line| {
                let mut parts = line.splitn(3, '\t');
                let type_char = parts.next()?;
                let name = parts.next()?.to_string();
                let size: u64 = parts.next()?.parse().ok()?;
                let is_dir = type_char == "D";
                Some((name, size, is_dir))
            })
            .collect();

        Some((items, total))
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
        context: &PreviewContext,
    ) -> Result<(), String> {
        // File size check
        if entry.size > Self::ARCHIVE_SIZE_LIMIT {
            ui.centered_and_justified(|ui| {
                ui.label(format!(
                    "Archive too large for preview ({} > {})",
                    bytesize::ByteSize(entry.size),
                    bytesize::ByteSize(Self::ARCHIVE_SIZE_LIMIT)
                ));
            });
            return Ok(());
        }

        // Try to get cached content
        let cached_content = {
            let cache = context.preview_cache.borrow();
            cache.get(&entry.path, entry.modified)
        };

        let parsed = if let Some(cached) = cached_content {
            Self::parse_cached(&cached)
        } else {
            let result = Self::extract_contents(entry);
            match result {
                Ok(ref content) => {
                    context
                        .preview_cache
                        .borrow_mut()
                        .insert(entry.path.clone(), content.clone(), entry.modified);
                    Self::parse_cached(content)
                }
                Err(e) => return Err(e),
            }
        };

        match parsed {
            Some((items, total)) => {
                if items.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.label("Empty archive");
                    });
                    return Ok(());
                }

                let count_msg = match total {
                    Some(t) => {
                        if t > Self::MAX_PREVIEW_ITEMS {
                            format!("Archive contains {} items (showing first {})", t, Self::MAX_PREVIEW_ITEMS)
                        } else {
                            format!("Archive contains {} items", t)
                        }
                    }
                    None => {
                        format!("Archive contains {}+ items (lazy preview)", items.len())
                    }
                };

                ui.label(format!("{}:", count_msg));
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
            None => Err("Failed to parse archive data".to_string()),
        }
    }

    fn priority(&self) -> i32 {
        30 // Medium-high priority
    }
}
