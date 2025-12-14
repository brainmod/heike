// Audio metadata preview handler

use crate::entry::FileEntry;
use crate::view::preview::handler::{PreviewContext, PreviewHandler};
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
        _context: &PreviewContext,
    ) -> Result<(), String> {
        if entry.extension == "mp3" {
            match id3::Tag::read_from_path(&entry.path) {
                Ok(tag) => {
                    ui.heading("Audio Metadata");
                    ui.separator();

                    if let Some(title) = tag.title() {
                        ui.label(format!("Title: {}", title));
                    }
                    if let Some(artist) = tag.artist() {
                        ui.label(format!("Artist: {}", artist));
                    }
                    if let Some(album) = tag.album() {
                        ui.label(format!("Album: {}", album));
                    }
                    if let Some(year) = tag.year() {
                        ui.label(format!("Year: {}", year));
                    }
                    if let Some(genre) = tag.genre() {
                        ui.label(format!("Genre: {}", genre));
                    }

                    ui.add_space(10.0);

                    if let Some(picture) = tag.pictures().next() {
                        ui.label(format!(
                            "Album art: {} ({})",
                            picture.mime_type,
                            bytesize::ByteSize(picture.data.len() as u64)
                        ));
                    }
                    Ok(())
                }
                Err(e) => Err(format!("No ID3 tags: {}", e)),
            }
        } else {
            ui.label("Audio metadata preview only available for MP3 files");
            Ok(())
        }
    }

    fn priority(&self) -> i32 {
        60 // Medium priority
    }
}
