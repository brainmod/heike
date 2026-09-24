// Text file preview handler with syntax highlighting

use crate::entry::FileEntry;
use crate::io::directory::is_likely_binary;
use crate::style;
use crate::view::preview::handler::{show_loading, PreviewContext, PreviewHandler};
use eframe::egui;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;
use syntect::easy::HighlightLines;
use syntect::util::LinesWithEndings;

struct Highlighted {
    path: PathBuf,
    modified: SystemTime,
    theme: style::Theme,
    job: egui::text::LayoutJob,
    total_lines: usize,
}

pub struct TextPreviewHandler {
    // Last binary check, so can_preview doesn't read the file every frame
    binary_check: Mutex<Option<(PathBuf, SystemTime, bool)>>,
    highlighted: Mutex<Option<Highlighted>>,
}

impl TextPreviewHandler {
    pub fn new() -> Self {
        Self {
            binary_check: Mutex::new(None),
            highlighted: Mutex::new(None),
        }
    }

    fn is_binary(&self, entry: &FileEntry) -> bool {
        let mut check = self.binary_check.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((path, modified, binary)) = check.as_ref() {
            if *path == entry.path && *modified == entry.modified {
                return *binary;
            }
        }
        let binary = is_likely_binary(&entry.path);
        *check = Some((entry.path.clone(), entry.modified, binary));
        binary
    }

    fn highlight(entry: &FileEntry, content: &str, context: &PreviewContext) -> Highlighted {
        let syntax = context
            .syntax_set
            .find_syntax_by_extension(&entry.extension)
            .or_else(|| context.syntax_set.find_syntax_by_first_line(content))
            .unwrap_or_else(|| context.syntax_set.find_syntax_plain_text());

        let theme_name = if context.theme == style::Theme::Dark {
            "base16-ocean.dark"
        } else {
            "base16-ocean.light"
        };
        let theme = &context.theme_set.themes[theme_name];
        let mut highlighter = HighlightLines::new(syntax, theme);
        let mut job = egui::text::LayoutJob::default();

        // Only highlight up to MAX_HIGHLIGHTED_LINES
        for line in LinesWithEndings::from(content).take(Self::MAX_HIGHLIGHTED_LINES) {
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

        Highlighted {
            path: entry.path.clone(),
            modified: entry.modified,
            theme: context.theme,
            job,
            total_lines: content.lines().count(),
        }
    }

    /// Maximum number of lines to syntax-highlight for performance
    /// Files with more lines will be truncated in preview
    const MAX_HIGHLIGHTED_LINES: usize = 1000;

    const TEXT_EXTENSIONS: &'static [&'static str] = &[
        "rs",
        "py",
        "js",
        "ts",
        "jsx",
        "tsx",
        "c",
        "cpp",
        "h",
        "hpp",
        "java",
        "go",
        "rb",
        "php",
        "swift",
        "kt",
        "scala",
        "sh",
        "bash",
        "zsh",
        "fish",
        "ps1",
        "bat",
        "cmd",
        "html",
        "css",
        "scss",
        "sass",
        "less",
        "xml",
        "yaml",
        "yml",
        "toml",
        "json",
        "ini",
        "cfg",
        "txt",
        "log",
        "conf",
        "config",
        "env",
        "gitignore",
        "dockerignore",
        "editorconfig",
        "sql",
        "r",
        "lua",
        "vim",
        "el",
        "clj",
        "ex",
        "exs",
        "erl",
        "hrl",
        "hs",
        "ml",
        "fs",
        "cs",
        "vb",
        "pl",
        "pm",
        "t",
        "asm",
        "s",
        "d",
        "diff",
        "patch",
        "mak",
        "makefile",
        "cmake",
        "gradle",
        "properties",
        "prefs",
        "plist",
        "nix",
        "lisp",
        "scm",
        "rkt",
        "proto",
        "thrift",
        "graphql",
        "gql",
        "vue",
        "svelte",
        "astro",
        "dart",
        "nim",
        "zig",
        "v",
        "vala",
        "cr",
        "rst",
        "adoc",
        "tex",
        "bib",
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
        Self::is_text_file(entry) && !self.is_binary(entry)
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

        let Some(content) = context
            .preview_cache
            .borrow_mut()
            .load(ui.ctx(), entry, |entry| {
                let data = fs::read(&entry.path).map_err(|e| format!("Read error: {}", e))?;
                Ok(String::from_utf8_lossy(&data).into_owned())
            })
        else {
            show_loading(ui);
            return Ok(());
        };
        let content = content?;

        // Syntax highlighting is expensive: do it once per (file, mtime, theme)
        let mut highlighted = self.highlighted.lock().unwrap_or_else(|e| e.into_inner());
        let is_current = highlighted.as_ref().is_some_and(|h| {
            h.path == entry.path && h.modified == entry.modified && h.theme == context.theme
        });
        if !is_current {
            *highlighted = Some(Self::highlight(entry, &content, context));
        }
        let Some(highlighted) = highlighted.as_ref() else {
            return Ok(());
        };

        // Show truncation warning if needed
        if highlighted.total_lines > Self::MAX_HIGHLIGHTED_LINES {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("⚠").color(egui::Color32::YELLOW));
                ui.label(
                    egui::RichText::new(format!(
                        "Large file: showing first {} of {} lines for performance",
                        Self::MAX_HIGHLIGHTED_LINES,
                        highlighted.total_lines
                    ))
                    .italics(),
                );
            });
            ui.separator();
        }

        egui::ScrollArea::vertical()
            .id_salt("preview_code")
            .auto_shrink([false, false])
            .max_height(ui.available_height())
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width());
                ui.label(highlighted.job.clone());
            });

        Ok(())
    }

    fn priority(&self) -> i32 {
        90 // Lower priority - generic text handler
    }
}
