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
        _context: &PreviewContext,
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

        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label(egui::RichText::new("📕 PDF Document").size(18.0));
            ui.add_space(10.0);

            match PdfDocument::load(&entry.path) {
                Ok(doc) => {
                    ui.label(format!("Pages: {}", doc.get_pages().len()));
                    ui.add_space(5.0);

                    let mut has_metadata = false;
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
                                                    ui.label(format!("Title: {}", title_str));
                                                    has_metadata = true;
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
                                                    ui.label(format!("Author: {}", author_str));
                                                    has_metadata = true;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
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
                    ui.colored_label(egui::Color32::RED, format!("Failed to load PDF: {}", e));
                }
            }
        });
        Ok(())
    }

    fn priority(&self) -> i32 {
        40 // Medium priority
    }
}
