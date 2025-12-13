use crate::entry::FileEntry;
use crate::io::directory::{is_likely_binary, read_directory};
use crate::style::{self, Theme};
use calamine::{open_workbook, Reader, Xls, Xlsx};
use chrono::{DateTime, Local};
use docx_rs::read_docx;
use eframe::egui;
use flate2::read::GzDecoder;
use id3::TagLike;
use lopdf::Document as PdfDocument;
use pulldown_cmark::{Event as MarkdownEvent, HeadingLevel, Parser, Tag, TagEnd};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;
use tar::Archive;
use zip::ZipArchive;

/// Cached preview content with metadata for invalidation
#[derive(Clone)]
pub struct CachedPreview {
    pub content: String,
    pub modified_time: SystemTime,
    pub cached_at: Instant,
}

/// Preview cache to avoid re-rendering identical files
pub struct PreviewCache {
    cache: HashMap<PathBuf, CachedPreview>,
    max_entries: usize,
}

impl PreviewCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            max_entries: 100, // Cache up to 100 file previews
        }
    }

    /// Get cached preview if valid (not modified since caching)
    pub fn get(&self, path: &PathBuf, current_mtime: SystemTime) -> Option<String> {
        if let Some(cached) = self.cache.get(path) {
            // Validate that file hasn't been modified
            if cached.modified_time == current_mtime {
                return Some(cached.content.clone());
            }
        }
        None
    }

    /// Store preview in cache
    pub fn insert(&mut self, path: PathBuf, content: String, mtime: SystemTime) {
        // Simple LRU: remove oldest entry if cache is full
        if self.cache.len() >= self.max_entries {
            if let Some(oldest_key) = self
                .cache
                .iter()
                .min_by_key(|(_, v)| v.cached_at)
                .map(|(k, _)| k.clone())
            {
                self.cache.remove(&oldest_key);
            }
        }

        self.cache.insert(
            path,
            CachedPreview {
                content,
                modified_time: mtime,
                cached_at: Instant::now(),
            },
        );
    }

    /// Clear cache
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    /// Get cache statistics
    pub fn stats(&self) -> (usize, usize) {
        (self.cache.len(), self.max_entries)
    }
}

impl Default for PreviewCache {
    fn default() -> Self {
        Self::new()
    }
}

pub fn render_large_file_message(ui: &mut egui::Ui, entry: &FileEntry) {
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
}

pub fn render_syntax_highlighted(
    ui: &mut egui::Ui,
    entry: &FileEntry,
    syntax_set: &SyntaxSet,
    theme_set: &ThemeSet,
    theme: Theme,
) {
    if entry.size > style::MAX_PREVIEW_SIZE {
        render_large_file_message(ui, entry);
        return;
    }

    match fs::read(&entry.path) {
        Ok(data) => {
            let content = String::from_utf8_lossy(&data);
            let syntax = syntax_set
                .find_syntax_by_extension(&entry.extension)
                .or_else(|| syntax_set.find_syntax_by_first_line(&content))
                .unwrap_or_else(|| syntax_set.find_syntax_plain_text());

            let theme_name = if theme == Theme::Dark {
                "base16-ocean.dark"
            } else {
                "base16-ocean.light"
            };
            let theme = &theme_set.themes[theme_name];

            egui::ScrollArea::vertical()
                .id_salt("preview_code")
                .auto_shrink([false, false])
                .max_height(ui.available_height())
                .show(ui, |ui| {
                    ui.set_max_width(ui.available_width());
                    let mut highlighter = HighlightLines::new(syntax, theme);

                    let mut job = egui::text::LayoutJob::default();

                    for line in LinesWithEndings::from(content.as_ref()) {
                        let ranges = highlighter
                            .highlight_line(line, syntax_set)
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
        }
        Err(e) => {
            ui.colored_label(egui::Color32::RED, format!("Read error: {}", e));
        }
    }
}

pub fn render_markdown_preview(ui: &mut egui::Ui, entry: &FileEntry) {
    if entry.size > style::MAX_PREVIEW_SIZE {
        render_large_file_message(ui, entry);
        return;
    }

    match fs::read_to_string(&entry.path) {
        Ok(content) => {
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
                                Tag::Paragraph => {}
                                Tag::List(_) => {}
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
                                    ui.label(
                                        egui::RichText::new(text.as_ref()).size(size).strong(),
                                    );
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
        }
        Err(e) => {
            ui.colored_label(egui::Color32::RED, format!("Read error: {}", e));
        }
    }
}

pub fn render_archive_preview(ui: &mut egui::Ui, entry: &FileEntry) {
    const MAX_PREVIEW_ITEMS: usize = 100;

    let result = if entry.extension == "zip" {
        fs::File::open(&entry.path).ok().and_then(|file| {
            ZipArchive::new(file).ok().map(|mut archive| {
                let total = archive.len();
                let mut items = Vec::new();
                for i in 0..total.min(MAX_PREVIEW_ITEMS) {
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
                    .take(MAX_PREVIEW_ITEMS)
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
                return;
            }

            ui.label(format!(
                "Archive contains {} items{}:",
                total,
                if total > MAX_PREVIEW_ITEMS {
                    format!(" (showing first {})", MAX_PREVIEW_ITEMS)
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
        }
        None => {
            ui.centered_and_justified(|ui| {
                ui.colored_label(egui::Color32::RED, "Failed to read archive");
            });
        }
    }
}

pub fn render_audio_metadata(ui: &mut egui::Ui, entry: &FileEntry) {
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
            }
            Err(e) => {
                ui.colored_label(egui::Color32::YELLOW, format!("No ID3 tags: {}", e));
            }
        }
    } else {
        ui.label("Audio metadata preview only available for MP3 files");
    }
}

pub fn render_docx_preview(ui: &mut egui::Ui, entry: &FileEntry) {
    ui.vertical_centered(|ui| {
        ui.add_space(20.0);
        ui.label(egui::RichText::new("📄 Word Document").size(18.0));
        ui.add_space(10.0);
    });

    match fs::read(&entry.path) {
        Ok(data) => {
            match read_docx(&data) {
                Ok(docx) => {
                    let mut text_content = String::new();
                    for child in docx.document.children {
                        if let docx_rs::DocumentChild::Paragraph(para) = child {
                            for child in para.children {
                                if let docx_rs::ParagraphChild::Run(run) = child {
                                    for child in run.children {
                                        if let docx_rs::RunChild::Text(text) = child {
                                            text_content.push_str(&text.text);
                                        }
                                    }
                                }
                            }
                            text_content.push('\n');
                        }
                    }

                    if text_content.trim().is_empty() {
                        ui.centered_and_justified(|ui| {
                            ui.label(
                                egui::RichText::new("Document appears to be empty")
                                    .italics()
                                    .weak(),
                            );
                        });
                    } else {
                        egui::ScrollArea::vertical()
                            .id_salt("docx_preview")
                            .auto_shrink([false, false])
                            .max_height(ui.available_height())
                            .show(ui, |ui| {
                                ui.set_max_width(ui.available_width());
                                ui.add_space(5.0);
                                ui.label(egui::RichText::new(&text_content).monospace());
                            });
                    }
                }
                Err(e) => {
                    ui.centered_and_justified(|ui| {
                        ui.colored_label(
                            egui::Color32::RED,
                            format!("Failed to parse DOCX: {}", e),
                        );
                    });
                }
            }
        }
        Err(e) => {
            ui.centered_and_justified(|ui| {
                ui.colored_label(egui::Color32::RED, format!("Failed to read file: {}", e));
            });
        }
    }
}

pub fn render_xlsx_preview(ui: &mut egui::Ui, entry: &FileEntry) {
    ui.vertical_centered(|ui| {
        ui.add_space(20.0);
        ui.label(egui::RichText::new("📊 Excel Spreadsheet").size(18.0));
        ui.add_space(10.0);
    });

    macro_rules! render_workbook {
        ($workbook:expr) => {{
            let sheet_names = $workbook.sheet_names().to_vec();

            if sheet_names.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        egui::RichText::new("No sheets found in workbook")
                            .italics()
                            .weak(),
                    );
                });
                return;
            }

            ui.vertical_centered(|ui| {
                ui.label(format!("Sheets: {}", sheet_names.len()));
                ui.add_space(5.0);
            });

            egui::ScrollArea::vertical()
                .id_salt("xlsx_preview")
                .auto_shrink([false, false])
                .max_height(ui.available_height())
                .show(ui, |ui| {
                    ui.set_max_width(ui.available_width());
                    for sheet_name in sheet_names.iter().take(3) {
                        if let Ok(range) = $workbook.worksheet_range(sheet_name) {
                            ui.add_space(10.0);
                            ui.label(
                                egui::RichText::new(format!("Sheet: {}", sheet_name)).strong(),
                            );
                            ui.add_space(5.0);

                            let (rows, cols) = range.get_size();
                            ui.label(format!("Dimensions: {} rows × {} columns", rows, cols));
                            ui.add_space(5.0);

                            let preview_rows = rows.min(10);
                            let preview_cols = cols.min(6);

                            use egui_extras::{Column, TableBuilder};
                            TableBuilder::new(ui)
                                .striped(true)
                                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                                .columns(Column::auto().at_least(80.0), preview_cols)
                                .header(20.0, |mut header| {
                                    for col in 0..preview_cols {
                                        header.col(|ui| {
                                            ui.strong(format!(
                                                "{}",
                                                (b'A' + col as u8) as char
                                            ));
                                        });
                                    }
                                })
                                .body(|mut body| {
                                    for row in 0..preview_rows {
                                        body.row(18.0, |mut row_ui| {
                                            for col in 0..preview_cols {
                                                row_ui.col(|ui| {
                                                    if let Some(cell) = range.get((row, col)) {
                                                        ui.label(cell.to_string());
                                                    } else {
                                                        ui.label("");
                                                    }
                                                });
                                            }
                                        });
                                    }
                                });

                            if rows > preview_rows || cols > preview_cols {
                                ui.add_space(5.0);
                                ui.label(
                                    egui::RichText::new(format!(
                                        "Showing {}/{} rows, {}/{} columns",
                                        preview_rows, rows, preview_cols, cols
                                    ))
                                    .italics()
                                    .weak(),
                                );
                            }
                        }
                    }

                    if sheet_names.len() > 3 {
                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "... and {} more sheets",
                                sheet_names.len() - 3
                            ))
                            .italics()
                            .weak(),
                        );
                    }
                });
        }};
    }

    if let Ok(mut workbook) = open_workbook::<Xlsx<_>, _>(&entry.path) {
        render_workbook!(workbook);
    } else if let Ok(mut workbook) = open_workbook::<Xls<_>, _>(&entry.path) {
        render_workbook!(workbook);
    } else {
        ui.centered_and_justified(|ui| {
            ui.colored_label(egui::Color32::RED, "Failed to open spreadsheet file");
        });
    }
}

pub fn render_pdf_preview(ui: &mut egui::Ui, entry: &FileEntry) {
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
}

pub fn render_preview_dispatcher(
    ui: &mut egui::Ui,
    entry: &FileEntry,
    show_hidden: bool,
    last_selection_change: Instant,
    directory_selections: &HashMap<PathBuf, usize>,
    syntax_set: &SyntaxSet,
    theme_set: &ThemeSet,
    theme: Theme,
    next_navigation: &std::cell::RefCell<Option<PathBuf>>,
    pending_selection: &std::cell::RefCell<Option<PathBuf>>,
) {
    style::truncated_label(
        ui,
        egui::RichText::new(format!("{} {}", entry.get_icon(), entry.display_name())).heading(),
    );
    ui.add_space(5.0);
    style::truncated_label(
        ui,
        format!("Size: {}", bytesize::ByteSize(entry.size)),
    );
    let datetime: DateTime<Local> = entry.modified.into();
    ui.label(format!("Modified: {}", datetime.format("%Y-%m-%d %H:%M")));
    ui.separator();

    if entry.is_dir {
        if last_selection_change.elapsed() <= Duration::from_millis(200) {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        }

        match read_directory(&entry.path, show_hidden) {
            Ok(entries) => {
                let accent = egui::Color32::from_rgb(120, 180, 255);
                let highlighted_index = directory_selections.get(&entry.path).copied();

                egui::ScrollArea::vertical()
                    .id_salt("preview_dir")
                    .auto_shrink([false, false])
                    .max_height(ui.available_height())
                    .show(ui, |ui| {
                        ui.set_max_width(ui.available_width());
                        let default_color = ui.visuals().text_color();
                        use egui_extras::{Column, TableBuilder};
                        TableBuilder::new(ui)
                            .striped(true)
                            .resizable(false)
                            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                            .column(Column::auto().at_least(30.0))
                            .column(Column::remainder().clip(true))
                            .body(|body| {
                                body.rows(24.0, entries.len(), |mut row| {
                                    let row_index = row.index();
                                    let preview_entry = &entries[row_index];
                                    let is_highlighted = highlighted_index == Some(row_index);
                                    let text_color = if is_highlighted || preview_entry.is_dir {
                                        accent
                                    } else {
                                        default_color
                                    };
                                    row.col(|ui| {
                                        ui.label(
                                            egui::RichText::new(preview_entry.get_icon())
                                                .size(14.0)
                                                .color(text_color),
                                        );
                                    });
                                    row.col(|ui| {
                                        let response = style::truncated_label_with_sense(
                                            ui,
                                            egui::RichText::new(preview_entry.display_name())
                                                .color(text_color),
                                            egui::Sense::click(),
                                        );
                                        if response.clicked() {
                                            *next_navigation.borrow_mut() =
                                                Some(entry.path.clone());
                                            *pending_selection.borrow_mut() =
                                                Some(preview_entry.path.clone());
                                        }
                                    });
                                });
                            });
                    });
            }
            Err(e) => {
                ui.centered_and_justified(|ui| {
                    ui.colored_label(
                        egui::Color32::RED,
                        format!("Cannot read directory: {}", e),
                    );
                });
            }
        }
        return;
    }
    if last_selection_change.elapsed() <= Duration::from_millis(200) {
        ui.centered_and_justified(|ui| {
            ui.spinner();
        });
        return;
    }

    if matches!(
        entry.extension.as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" | "ico"
    ) {
        let uri = format!("file://{}", entry.path.display());
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
        return;
    }

    if matches!(entry.extension.as_str(), "md" | "markdown") {
        render_markdown_preview(ui, entry);
        return;
    }

    if matches!(
        entry.extension.as_str(),
        "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz"
    ) {
        render_archive_preview(ui, entry);
        return;
    }

    if matches!(
        entry.extension.as_str(),
        "mp3" | "flac" | "ogg" | "m4a" | "wav"
    ) {
        render_audio_metadata(ui, entry);
        return;
    }

    if matches!(entry.extension.as_str(), "pdf") {
        render_pdf_preview(ui, entry);
        return;
    }

    if matches!(entry.extension.as_str(), "docx" | "doc") {
        render_docx_preview(ui, entry);
        return;
    }

    if matches!(entry.extension.as_str(), "xlsx" | "xls") {
        render_xlsx_preview(ui, entry);
        return;
    }

    let text_extensions = [
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

    let check_as_text = text_extensions.contains(&entry.extension.as_str())
        || entry.extension.is_empty()
        || entry.name.starts_with('.');

    if check_as_text {
        if entry.size > style::MAX_PREVIEW_SIZE {
            render_large_file_message(ui, entry);
            return;
        }

        if !is_likely_binary(&entry.path) {
            render_syntax_highlighted(ui, entry, syntax_set, theme_set, theme);
            return;
        }
    }

    ui.centered_and_justified(|ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label(egui::RichText::new("📦 Binary File").size(18.0));
            ui.add_space(10.0);
            ui.label("Preview not available for this file type");
            ui.add_space(5.0);
            ui.label(format!("Extension: .{}", entry.extension));
        });
    });
}
