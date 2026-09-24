// Office document preview handler (docx, xlsx, etc.)

use crate::entry::FileEntry;
use crate::style;
use crate::view::preview::handler::{show_loading, PreviewContext, PreviewHandler};
use calamine::{open_workbook, Reader, Xls, Xlsx};
use docx_rs::read_docx;
use eframe::egui;
use std::fs;

const XLSX_PREVIEW_SHEETS: usize = 3;
const XLSX_PREVIEW_ROWS: usize = 10;
const XLSX_PREVIEW_COLS: usize = 6;

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
                    ui.label(egui::RichText::new(text_content).monospace());
                });
        }
    }

    fn render_docx(
        &self,
        ui: &mut egui::Ui,
        entry: &FileEntry,
        context: &PreviewContext,
    ) -> Result<(), String> {
        let Some(content) =
            context
                .preview_cache
                .borrow_mut()
                .load(ui.ctx(), entry, Self::extract_docx_text)
        else {
            show_loading(ui);
            return Ok(());
        };
        let content = content?;

        self.render_docx_content(ui, &content);
        Ok(())
    }

    /// Summarize the first sheets of a workbook.
    /// Format: sheet count on line 1, then per sheet a "#\tname\trows\tcols"
    /// header followed by up to XLSX_PREVIEW_ROWS tab-separated rows.
    fn extract_xlsx(entry: &FileEntry) -> Result<String, String> {
        fn summarize<R: Reader<std::io::BufReader<fs::File>>>(workbook: &mut R) -> String {
            let sheet_names = workbook.sheet_names().to_vec();
            let mut lines = vec![sheet_names.len().to_string()];
            for sheet_name in sheet_names.iter().take(XLSX_PREVIEW_SHEETS) {
                let Ok(range) = workbook.worksheet_range(sheet_name) else {
                    continue;
                };
                let (rows, cols) = range.get_size();
                lines.push(format!("#\t{}\t{}\t{}", clean(sheet_name), rows, cols));
                for row in 0..rows.min(XLSX_PREVIEW_ROWS) {
                    let cells: Vec<String> = (0..cols.min(XLSX_PREVIEW_COLS))
                        .map(|col| {
                            range
                                .get((row, col))
                                .map(|c| clean(&c.to_string()))
                                .unwrap_or_default()
                        })
                        .collect();
                    lines.push(cells.join("\t"));
                }
            }
            lines.join("\n")
        }
        fn clean(s: &str) -> String {
            s.replace(['\t', '\n', '\r'], " ")
        }

        if let Ok(mut workbook) = open_workbook::<Xlsx<_>, _>(&entry.path) {
            Ok(summarize(&mut workbook))
        } else if let Ok(mut workbook) = open_workbook::<Xls<_>, _>(&entry.path) {
            Ok(summarize(&mut workbook))
        } else {
            Err("Failed to open spreadsheet file".to_string())
        }
    }

    fn render_xlsx(
        &self,
        ui: &mut egui::Ui,
        entry: &FileEntry,
        context: &PreviewContext,
    ) -> Result<(), String> {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label(egui::RichText::new("📊 Excel Spreadsheet").size(18.0));
            ui.add_space(10.0);
        });

        let Some(content) =
            context
                .preview_cache
                .borrow_mut()
                .load(ui.ctx(), entry, Self::extract_xlsx)
        else {
            show_loading(ui);
            return Ok(());
        };
        let content = content?;
        let mut lines = content.lines();
        let sheet_count: usize = lines.next().and_then(|l| l.parse().ok()).unwrap_or(0);

        if sheet_count == 0 {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new("No sheets found in workbook")
                        .italics()
                        .weak(),
                );
            });
            return Ok(());
        }

        // Group lines into (name, rows, cols, preview rows)
        let mut sheets: Vec<(&str, usize, usize, Vec<Vec<&str>>)> = Vec::new();
        for line in lines {
            if let Some(header) = line.strip_prefix("#\t") {
                let mut parts = header.split('\t');
                let name = parts.next().unwrap_or("");
                let rows = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
                let cols = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
                sheets.push((name, rows, cols, Vec::new()));
            } else if let Some(sheet) = sheets.last_mut() {
                sheet.3.push(line.split('\t').collect());
            }
        }

        ui.vertical_centered(|ui| {
            ui.label(format!("Sheets: {}", sheet_count));
            ui.add_space(5.0);
        });

        egui::ScrollArea::vertical()
            .id_salt("xlsx_preview")
            .auto_shrink([false, false])
            .max_height(ui.available_height())
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width());
                for (sheet_index, (name, rows, cols, preview)) in sheets.iter().enumerate() {
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new(format!("Sheet: {}", name)).strong());
                    ui.add_space(5.0);
                    ui.label(format!("Dimensions: {} rows × {} columns", rows, cols));
                    ui.add_space(5.0);

                    let preview_rows = preview.len();
                    let preview_cols = (*cols).min(XLSX_PREVIEW_COLS);

                    use egui_extras::{Column, TableBuilder};
                    TableBuilder::new(ui)
                        .id_salt(("xlsx_sheet", sheet_index))
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
                            for cells in preview {
                                body.row(18.0, |mut row_ui| {
                                    for col in 0..preview_cols {
                                        row_ui.col(|ui| {
                                            ui.label(cells.get(col).copied().unwrap_or(""));
                                        });
                                    }
                                });
                            }
                        });

                    if *rows > preview_rows || *cols > preview_cols {
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

                if sheet_count > XLSX_PREVIEW_SHEETS {
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "... and {} more sheets",
                            sheet_count - XLSX_PREVIEW_SHEETS
                        ))
                        .italics()
                        .weak(),
                    );
                }
            });
        Ok(())
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
            "xlsx" | "xls" => self.render_xlsx(ui, entry, context),
            _ => Err("Unsupported office document type".to_string()),
        }
    }

    fn priority(&self) -> i32 {
        50 // Medium priority
    }
}
