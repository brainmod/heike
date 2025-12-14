// Markdown preview handler

use crate::entry::FileEntry;
use crate::style;
use crate::view::preview::handler::{PreviewContext, PreviewHandler};
use eframe::egui;
use pulldown_cmark::{Event as MarkdownEvent, HeadingLevel, Parser, Tag, TagEnd};
use std::fs;

pub struct MarkdownPreviewHandler;

impl MarkdownPreviewHandler {
    pub fn new() -> Self {
        Self
    }
}

impl PreviewHandler for MarkdownPreviewHandler {
    fn name(&self) -> &str {
        "markdown"
    }

    fn can_preview(&self, entry: &FileEntry) -> bool {
        matches!(entry.extension.as_str(), "md" | "markdown")
    }

    fn render(
        &self,
        ui: &mut egui::Ui,
        entry: &FileEntry,
        context: &PreviewContext,
    ) -> Result<(), String> {
        if entry.size > style::MAX_PREVIEW_SIZE {
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.label(egui::RichText::new("📄 File Too Large").size(18.0));
                    ui.add_space(10.0);
                    ui.label(format!("File size: {}", bytesize::ByteSize(entry.size)));
                    ui.label(format!(
                        "Preview limit: {}",
                        bytesize::ByteSize(style::MAX_PREVIEW_SIZE)
                    ));
                });
            });
            return Ok(());
        }

        // Try to get cached content first
        let cached_content = {
            let cache = context.preview_cache.borrow();
            cache.get(&entry.path, entry.modified)
        };

        let content = if let Some(cached) = cached_content {
            // Cache hit - use cached content
            cached
        } else {
            // Cache miss - read from disk
            let content = fs::read_to_string(&entry.path)
                .map_err(|e| format!("Failed to read file: {}", e))?;

            // Store in cache for future use
            context
                .preview_cache
                .borrow_mut()
                .insert(entry.path.clone(), content.clone(), entry.modified);

            content
        };

        egui::ScrollArea::vertical()
            .id_salt("preview_md")
            .auto_shrink([false, false])
            .max_height(ui.available_height())
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width());
                let parser = Parser::new(&content);
                let mut in_code_block = false;
                let mut in_heading = false;
                let mut heading_level = 1;

                for event in parser {
                    match event {
                        MarkdownEvent::Start(tag) => match tag {
                            Tag::Heading { level, .. } => {
                                in_heading = true;
                                heading_level = match level {
                                    HeadingLevel::H1 => 1,
                                    HeadingLevel::H2 => 2,
                                    HeadingLevel::H3 => 3,
                                    HeadingLevel::H4 => 4,
                                    HeadingLevel::H5 => 5,
                                    HeadingLevel::H6 => 6,
                                };
                            }
                            Tag::CodeBlock(_) => in_code_block = true,
                            _ => {}
                        },
                        MarkdownEvent::End(tag) => match tag {
                            TagEnd::Heading(_) => {
                                in_heading = false;
                                ui.add_space(5.0);
                            }
                            TagEnd::CodeBlock => {
                                in_code_block = false;
                                ui.add_space(5.0);
                            }
                            TagEnd::Paragraph => ui.add_space(5.0),
                            _ => {}
                        },
                        MarkdownEvent::Text(text) => {
                            if in_heading {
                                let size = match heading_level {
                                    1 => 24.0,
                                    2 => 20.0,
                                    3 => 18.0,
                                    4 => 16.0,
                                    _ => 14.0,
                                };
                                ui.label(egui::RichText::new(text.as_ref()).size(size).strong());
                            } else if in_code_block {
                                ui.monospace(text.as_ref());
                            } else {
                                ui.label(text.as_ref());
                            }
                        }
                        MarkdownEvent::Code(code) => {
                            ui.monospace(
                                egui::RichText::new(code.as_ref())
                                    .background_color(egui::Color32::from_gray(50)),
                            );
                        }
                        MarkdownEvent::SoftBreak | MarkdownEvent::HardBreak => {
                            ui.label("");
                        }
                        _ => {}
                    }
                }
            });

        Ok(())
    }

    fn priority(&self) -> i32 {
        20 // High priority - specific file type
    }
}
