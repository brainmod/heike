// Office document preview handler (docx, xlsx, etc.)

use crate::entry::FileEntry;
use crate::style;
use crate::view::preview::handler::{PreviewContext, PreviewHandler};
use calamine::{open_workbook, Reader, Xls, Xlsx};
use docx_rs::read_docx;
use eframe::egui;
use std::fs;

pub struct OfficePreviewHandler;

impl OfficePreviewHandler {
    pub fn new() -> Self {
        Self
    }

    /// Extract DOCX text content for caching
    fn extract_docx_text(entry: &FileEntry) -> Result<String, String> {
        let data = fs::read(&entry.path).map_err(|e| format!("Failed to read file: {}", e))?;
        let docx = read_docx(&data).map_err(|e| format!("Failed to parse DOCX: {}", e))?;

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
        Ok(text_content)
    }

    fn render_docx_content(&self, ui: &mut egui::Ui, text_content: &str) {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label(egui::RichText::new("📄 Word Document").size(18.0));
            ui.add_space(10.0);
        });

        if text_content.trim().is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(egui::RichText::new("Document appears to be empty").italics().weak());
            });
        } else {
            egui::ScrollArea::vertical()
                .id_salt("docx_preview")
                .auto_shrink([false, false])
                .max_height(ui.available_height())
                .show(ui, |ui| {
                    ui.set_max_width(ui.available_width());
                    ui.add_space(5.0);
                    ui.label(egui::RichText::new(text_content).monospace());
                });
        }
    }

    fn render_docx(&self, ui: &mut egui::Ui, entry: &FileEntry, context: &PreviewContext) -> Result<(), String> {
        // Try cache first
        let cached_content = {
            let cache = context.preview_cache.borrow();
            cache.get(&entry.path, entry.modified)
        };

        let content = if let Some(cached) = cached_content {
            cached
        } else {
            let text = Self::extract_docx_text(entry)?;
            context
                .preview_cache
                .borrow_mut()
                .insert(entry.path.clone(), text.clone(), entry.modified);
            text
        };

        self.render_docx_content(ui, &content);
        Ok(())
    }

    fn render_xlsx(&self, ui: &mut egui::Ui, entry: &FileEntry) -> Result<(), String> {
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
                    return Ok(());
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
                                                ui.strong(format!("{}", (b'A' + col as u8) as char));
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
            Ok(())
        } else if let Ok(mut workbook) = open_workbook::<Xls<_>, _>(&entry.path) {
            render_workbook!(workbook);
            Ok(())
        } else {
            Err("Failed to open spreadsheet file".to_string())
        }
    }
}

impl PreviewHandler for OfficePreviewHandler {
    fn name(&self) -> &str {
        "office"
    }

    fn can_preview(&self, entry: &FileEntry) -> bool {
        matches!(entry.extension.as_str(), "docx" | "doc" | "xlsx" | "xls")
    }

    fn render(
        &self,
        ui: &mut egui::Ui,
        entry: &FileEntry,
        context: &PreviewContext,
    ) -> Result<(), String> {
        // File size check to prevent blocking UI on large documents
        if entry.size > style::MAX_PREVIEW_SIZE {
            ui.centered_and_justified(|ui| {
                ui.label(format!(
                    "Document too large for preview ({} > {})",
                    bytesize::ByteSize(entry.size),
                    bytesize::ByteSize(style::MAX_PREVIEW_SIZE)
                ));
            });
            return Ok(());
        }

        match entry.extension.as_str() {
            "docx" | "doc" => self.render_docx(ui, entry, context),
            "xlsx" | "xls" => self.render_xlsx(ui, entry),
            _ => Err("Unsupported office document type".to_string()),
        }
    }

    fn priority(&self) -> i32 {
        50 // Medium priority
    }
}
