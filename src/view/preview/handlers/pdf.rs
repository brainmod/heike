// PDF preview handler

use crate::entry::FileEntry;
use crate::style;
use crate::view::preview::handler::{PreviewContext, PreviewHandler};
use eframe::egui;
use lopdf::Document as PdfDocument;

pub struct PdfPreviewHandler;

impl PdfPreviewHandler {
    pub fn new() -> Self {
        Self
    }

    /// Extract PDF metadata as a cacheable string
    /// Format: "pages:<N>\ntitle:<title>\nauthor:<author>"
    fn extract_metadata(entry: &FileEntry) -> Result<String, String> {
        match PdfDocument::load(&entry.path) {
            Ok(doc) => {
                let mut lines = Vec::new();
                lines.push(format!("pages:{}", doc.get_pages().len()));

                if let Ok(info_ref) = doc.trailer.get(b"Info") {
                    if let Ok(info_id) = info_ref.as_reference() {
                        if let Ok(info_obj) = doc.get_object(info_id) {
                            if let Ok(info_dict) = info_obj.as_dict() {
                                if let Ok(title_obj) = info_dict.get(b"Title") {
                                    if let Ok(title_bytes) = title_obj.as_str() {
                                        if let Ok(title_str) =
                                            String::from_utf8(title_bytes.to_vec())
                                        {
                                            if !title_str.is_empty() {
                                                lines.push(format!("title:{}", title_str));
                                            }
                                        }
                                    }
                                }
                                if let Ok(author_obj) = info_dict.get(b"Author") {
                                    if let Ok(author_bytes) = author_obj.as_str() {
                                        if let Ok(author_str) =
                                            String::from_utf8(author_bytes.to_vec())
                                        {
                                            if !author_str.is_empty() {
                                                lines.push(format!("author:{}", author_str));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                Ok(lines.join("\n"))
            }
            Err(e) => Err(format!("Failed to load PDF: {}", e)),
        }
    }
}

impl PreviewHandler for PdfPreviewHandler {
    fn name(&self) -> &str {
        "pdf"
    }

    fn can_preview(&self, entry: &FileEntry) -> bool {
        entry.extension == "pdf"
    }

    fn render(
        &self,
        ui: &mut egui::Ui,
        entry: &FileEntry,
        context: &PreviewContext,
    ) -> Result<(), String> {
        // File size check to prevent blocking UI on large PDFs
        if entry.size > style::MAX_PREVIEW_SIZE {
            ui.centered_and_justified(|ui| {
                ui.label(format!(
                    "PDF too large for preview ({} > {})",
                    bytesize::ByteSize(entry.size),
                    bytesize::ByteSize(style::MAX_PREVIEW_SIZE)
                ));
            });
            return Ok(());
        }

        // Try to get cached metadata
        let cached_content = {
            let cache = context.preview_cache.borrow();
            cache.get(&entry.path, entry.modified)
        };

        let metadata = if let Some(cached) = cached_content {
            Ok(cached)
        } else {
            let result = Self::extract_metadata(entry);
            if let Ok(ref content) = result {
                context
                    .preview_cache
                    .borrow_mut()
                    .insert(entry.path.clone(), content.clone(), entry.modified);
            }
            result
        };

        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label(egui::RichText::new("📕 PDF Document").size(18.0));
            ui.add_space(10.0);

            match metadata {
                Ok(content) => {
                    let mut has_metadata = false;
                    for line in content.lines() {
                        if let Some(pages) = line.strip_prefix("pages:") {
                            ui.label(format!("Pages: {}", pages));
                            ui.add_space(5.0);
                        } else if let Some(title) = line.strip_prefix("title:") {
                            ui.label(format!("Title: {}", title));
                            has_metadata = true;
                        } else if let Some(author) = line.strip_prefix("author:") {
                            ui.label(format!("Author: {}", author));
                            has_metadata = true;
                        }
                    }

                    if !has_metadata {
                        ui.label(
                            egui::RichText::new("No metadata available")
                                .italics()
                                .weak(),
                        );
                    }

                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new("Text content extraction disabled for performance")
                            .italics()
                            .weak(),
                    );
                }
                Err(e) => {
                    ui.colored_label(egui::Color32::RED, &e);
                }
            }
        });
        Ok(())
    }

    fn priority(&self) -> i32 {
        40 // Medium priority
    }
}
