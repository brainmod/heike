// Text file preview handler with syntax highlighting

use crate::entry::FileEntry;
use crate::io::directory::is_likely_binary;
use crate::style;
use crate::view::preview::handler::{PreviewContext, PreviewHandler};
use eframe::egui;
use std::fs;
use syntect::easy::HighlightLines;
use syntect::util::LinesWithEndings;

pub struct TextPreviewHandler;

impl TextPreviewHandler {
    pub fn new() -> Self {
        Self
    }

    /// Maximum number of lines to syntax-highlight for performance
    /// Files with more lines will be truncated in preview
    const MAX_HIGHLIGHTED_LINES: usize = 1000;

    const TEXT_EXTENSIONS: &'static [&'static str] = &[
        "rs", "py", "js", "ts", "jsx", "tsx", "c", "cpp", "h", "hpp", "java", "go", "rb", "php",
        "swift", "kt", "scala", "sh", "bash", "zsh", "fish", "ps1", "bat", "cmd", "html", "css",
        "scss", "sass", "less", "xml", "yaml", "yml", "toml", "json", "ini", "cfg", "txt", "log",
        "conf", "config", "env", "gitignore", "dockerignore", "editorconfig", "sql", "r", "lua",
        "vim", "el", "clj", "ex", "exs", "erl", "hrl", "hs", "ml", "fs", "cs", "vb", "pl", "pm",
        "t", "asm", "s", "d", "diff", "patch", "mak", "makefile", "cmake", "gradle", "properties",
        "prefs", "plist", "nix", "lisp", "scm", "rkt", "proto", "thrift", "graphql", "gql", "vue",
        "svelte", "astro", "dart", "nim", "zig", "v", "vala", "cr", "rst", "adoc", "tex", "bib",
        "lock",
    ];

    fn is_text_file(entry: &FileEntry) -> bool {
        Self::TEXT_EXTENSIONS.contains(&entry.extension.as_str())
            || entry.extension.is_empty()
            || entry.name.starts_with('.')
    }
}

impl PreviewHandler for TextPreviewHandler {
    fn name(&self) -> &str {
        "text"
    }

    fn can_preview(&self, entry: &FileEntry) -> bool {
        // Only handle non-binary text files
        Self::is_text_file(entry) && !is_likely_binary(&entry.path)
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
        let content = if let Some(cached) = context.preview_cache.borrow().get(&entry.path, entry.modified) {
            // Cache hit - use cached content
            cached
        } else {
            // Cache miss - read from disk
            let data = fs::read(&entry.path).map_err(|e| format!("Read error: {}", e))?;
            let content = String::from_utf8_lossy(&data).to_string();

            // Store in cache for future use
            context.preview_cache.borrow_mut().insert(entry.path.clone(), content.clone(), entry.modified);

            content
        };

        let syntax = context
            .syntax_set
            .find_syntax_by_extension(&entry.extension)
            .or_else(|| context.syntax_set.find_syntax_by_first_line(&content))
            .unwrap_or_else(|| context.syntax_set.find_syntax_plain_text());

        let theme_name = if context.theme == style::Theme::Dark {
            "base16-ocean.dark"
        } else {
            "base16-ocean.light"
        };
        let theme = &context.theme_set.themes[theme_name];

        // Count total lines and check if we need to truncate
        let all_lines: Vec<&str> = content.lines().collect();
        let total_lines = all_lines.len();
        let is_truncated = total_lines > Self::MAX_HIGHLIGHTED_LINES;

        // Show truncation warning if needed
        if is_truncated {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("⚠").color(egui::Color32::YELLOW));
                ui.label(egui::RichText::new(format!(
                    "Large file: showing first {} of {} lines for performance",
                    Self::MAX_HIGHLIGHTED_LINES,
                    total_lines
                )).italics());
            });
            ui.separator();
        }

        egui::ScrollArea::vertical()
            .id_salt("preview_code")
            .auto_shrink([false, false])
            .max_height(ui.available_height())
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width());
                let mut highlighter = HighlightLines::new(syntax, theme);

                let mut job = egui::text::LayoutJob::default();

                // Only highlight up to MAX_HIGHLIGHTED_LINES
                let lines_to_highlight = if is_truncated {
                    Self::MAX_HIGHLIGHTED_LINES
                } else {
                    total_lines
                };

                for line in LinesWithEndings::from(content.as_ref()).take(lines_to_highlight) {
                    let ranges = highlighter
                        .highlight_line(line, context.syntax_set)
                        .unwrap_or_default();

                    for (style, text) in ranges {
                        let color = egui::Color32::from_rgb(
                            style.foreground.r,
                            style.foreground.g,
                            style.foreground.b,
                        );
                        job.append(
                            text,
                            0.0,
                            egui::TextFormat {
                                font_id: egui::FontId::monospace(12.0),
                                color,
                                ..Default::default()
                            },
                        );
                    }
                }

                ui.label(job);
            });

        Ok(())
    }

    fn priority(&self) -> i32 {
        90 // Lower priority - generic text handler
    }
}
