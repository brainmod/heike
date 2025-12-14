// Image preview handler

use crate::entry::FileEntry;
use crate::view::preview::handler::{PreviewContext, PreviewHandler};
use eframe::egui;

pub struct ImagePreviewHandler;

impl ImagePreviewHandler {
    pub fn new() -> Self {
        Self
    }

    fn is_image_extension(ext: &str) -> bool {
        matches!(
            ext,
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" | "ico"
        )
    }

    /// Encode a file path as a proper file:// URI with percent-encoding
    fn path_to_file_uri(path: &std::path::Path) -> String {
        let path_str = path.to_string_lossy();
        let mut encoded = String::with_capacity(path_str.len() + 10);
        encoded.push_str("file://");

        for ch in path_str.chars() {
            match ch {
                // RFC 3986 unreserved characters (safe in URIs)
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' | '/' => {
                    encoded.push(ch);
                }
                // Everything else needs percent-encoding
                _ => {
                    for byte in ch.to_string().as_bytes() {
                        encoded.push_str(&format!("%{:02X}", byte));
                    }
                }
            }
        }
        encoded
    }
}

impl PreviewHandler for ImagePreviewHandler {
    fn name(&self) -> &str {
        "image"
    }

    fn can_preview(&self, entry: &FileEntry) -> bool {
        Self::is_image_extension(&entry.extension)
    }

    fn render(
        &self,
        ui: &mut egui::Ui,
        entry: &FileEntry,
        _context: &PreviewContext,
    ) -> Result<(), String> {
        let uri = Self::path_to_file_uri(&entry.path);
        egui::ScrollArea::vertical()
            .id_salt("preview_img")
            .auto_shrink([false, false])
            .max_height(ui.available_height())
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width());
                let available = ui.available_size();
                ui.add(
                    egui::Image::new(uri)
                        .max_width(available.x)
                        .max_height(available.y - 100.0)
                        .maintain_aspect_ratio(true)
                        .shrink_to_fit(),
                );
            });
        Ok(())
    }

    fn priority(&self) -> i32 {
        10 // High priority - specific handler
    }
}
