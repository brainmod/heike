// Audio metadata preview handler

use crate::entry::FileEntry;
use crate::view::preview::handler::{show_loading, PreviewContext, PreviewHandler};
use eframe::egui;
use id3::TagLike;

pub struct AudioPreviewHandler;

impl AudioPreviewHandler {
    pub fn new() -> Self {
        Self
    }

    fn is_audio_extension(ext: &str) -> bool {
        matches!(ext, "mp3" | "flac" | "ogg" | "m4a" | "wav")
    }

    /// Extract metadata as a cacheable string
    fn extract_metadata(entry: &FileEntry) -> Result<String, String> {
        if entry.extension != "mp3" {
            return Err("non-mp3".to_string());
        }

        match id3::Tag::read_from_path(&entry.path) {
            Ok(tag) => {
                let mut lines = Vec::new();

                if let Some(title) = tag.title() {
                    lines.push(format!("Title: {}", title));
                }
                if let Some(artist) = tag.artist() {
                    lines.push(format!("Artist: {}", artist));
                }
                if let Some(album) = tag.album() {
                    lines.push(format!("Album: {}", album));
                }
                if let Some(year) = tag.year() {
                    lines.push(format!("Year: {}", year));
                }
                if let Some(genre) = tag.genre() {
                    lines.push(format!("Genre: {}", genre));
                }

                if let Some(picture) = tag.pictures().next() {
                    lines.push(format!(
                        "Album art: {} ({})",
                        picture.mime_type,
                        bytesize::ByteSize(picture.data.len() as u64)
                    ));
                }

                Ok(lines.join("\n"))
            }
            Err(e) => Err(format!("No ID3 tags: {}", e)),
        }
    }
}

impl PreviewHandler for AudioPreviewHandler {
    fn name(&self) -> &str {
        "audio"
    }

    fn can_preview(&self, entry: &FileEntry) -> bool {
        Self::is_audio_extension(&entry.extension)
    }

    fn render(
        &self,
        ui: &mut egui::Ui,
        entry: &FileEntry,
        context: &PreviewContext,
    ) -> Result<(), String> {
        if entry.extension != "mp3" {
            ui.label("Audio metadata preview only available for MP3 files");
            return Ok(());
        }

        let Some(metadata) =
            context
                .preview_cache
                .borrow_mut()
                .load(ui.ctx(), entry, Self::extract_metadata)
        else {
            show_loading(ui);
            return Ok(());
        };

        match metadata {
            Ok(content) => {
                ui.heading("Audio Metadata");
                ui.separator();
                for line in content.lines() {
                    ui.label(line);
                }
                ui.add_space(10.0);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    fn priority(&self) -> i32 {
        60 // Medium priority
    }
}
