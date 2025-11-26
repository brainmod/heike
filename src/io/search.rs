use crate::state::{SearchOptions, SearchResult};
use calamine::{open_workbook, Reader, Xls, Xlsx};
use docx_rs::read_docx;
use grep_matcher::Matcher;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::{Searcher, Sink, SinkMatch};
use ignore::WalkBuilder;
use lopdf::Document as PdfDocument;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use zip::ZipArchive;

use super::worker::IoResult;

struct SearchSink {
    results: Vec<SearchResult>,
    file_path: PathBuf,
    file_name: String,
    max_results: usize,
}

impl Sink for SearchSink {
    type Error = std::io::Error;

    fn matched(&mut self, _searcher: &Searcher, mat: &SinkMatch) -> Result<bool, Self::Error> {
        if self.results.len() >= self.max_results {
            return Ok(false);
        }

        let line_number = mat.line_number().unwrap_or(0) as usize;
        let line_content = String::from_utf8_lossy(mat.bytes()).to_string();

        let (match_start, match_end) = if mat.bytes().iter().position(|_| true).is_some() {
            (0, line_content.len().min(100))
        } else {
            (0, 0)
        };

        self.results.push(SearchResult {
            file_path: self.file_path.clone(),
            file_name: self.file_name.clone(),
            line_number,
            line_content: line_content.trim_end().to_string(),
            match_start,
            match_end,
        });

        Ok(true)
    }
}

fn search_text_file(
    path: &Path,
    matcher: &impl Matcher,
    max_results: usize,
) -> Result<Vec<SearchResult>, Box<dyn std::error::Error>> {
    let mut sink = SearchSink {
        results: Vec::new(),
        file_path: path.to_path_buf(),
        file_name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        max_results,
    };

    let mut searcher = Searcher::new();
    searcher.search_path(matcher, path, &mut sink)?;

    Ok(sink.results)
}

fn search_pdf_content(path: &Path, query: &str, case_sensitive: bool) -> Vec<SearchResult> {
    let mut results = Vec::new();

    if let Ok(doc) = PdfDocument::load(path) {
        let mut all_text = String::new();

        let pages = doc.get_pages();
        let page_numbers: Vec<u32> = pages.keys().cloned().collect();
        if let Ok(text) = doc.extract_text(&page_numbers) {
            all_text.push_str(&text);
        }

        let search_query = if case_sensitive {
            query.to_string()
        } else {
            query.to_lowercase()
        };

        for (line_num, line) in all_text.lines().enumerate() {
            let check_line = if case_sensitive {
                line.to_string()
            } else {
                line.to_lowercase()
            };
            if check_line.contains(&search_query) {
                if let Some(pos) = check_line.find(&search_query) {
                    results.push(SearchResult {
                        file_path: path.to_path_buf(),
                        file_name: path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                        line_number: line_num + 1,
                        line_content: line.trim().to_string(),
                        match_start: pos,
                        match_end: pos + search_query.len(),
                    });
                }
            }
        }
    }

    results
}

fn search_zip_archive(path: &Path, query: &str, case_sensitive: bool) -> Vec<SearchResult> {
    let mut results = Vec::new();

    if let Ok(file) = fs::File::open(path) {
        if let Ok(mut archive) = ZipArchive::new(file) {
            for i in 0..archive.len() {
                if let Ok(mut file) = archive.by_index(i) {
                    let file_name = file.name().to_string();

                    if file.is_file() && !file.name().ends_with('/') {
                        let mut contents = String::new();
                        if std::io::Read::read_to_string(&mut file, &mut contents).is_ok() {
                            let search_query = if case_sensitive {
                                query.to_string()
                            } else {
                                query.to_lowercase()
                            };

                            for (line_num, line) in contents.lines().enumerate() {
                                let check_line = if case_sensitive {
                                    line.to_string()
                                } else {
                                    line.to_lowercase()
                                };
                                if check_line.contains(&search_query) {
                                    if let Some(pos) = check_line.find(&search_query) {
                                        results.push(SearchResult {
                                            file_path: path.to_path_buf(),
                                            file_name: format!(
                                                "{} -> {}",
                                                path.file_name()
                                                    .unwrap_or_default()
                                                    .to_string_lossy(),
                                                file_name
                                            ),
                                            line_number: line_num + 1,
                                            line_content: line.trim().to_string(),
                                            match_start: pos,
                                            match_end: pos + search_query.len(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    results
}

fn search_docx_content(path: &Path, query: &str, case_sensitive: bool) -> Vec<SearchResult> {
    let mut results = Vec::new();

    if let Ok(data) = fs::read(path) {
        if let Ok(docx) = read_docx(&data) {
            let mut all_text = String::new();
            for child in docx.document.children {
                if let docx_rs::DocumentChild::Paragraph(para) = child {
                    for child in para.children {
                        if let docx_rs::ParagraphChild::Run(run) = child {
                            for child in run.children {
                                if let docx_rs::RunChild::Text(text) = child {
                                    all_text.push_str(&text.text);
                                }
                            }
                        }
                    }
                    all_text.push('\n');
                }
            }

            let search_query = if case_sensitive {
                query.to_string()
            } else {
                query.to_lowercase()
            };

            for (line_num, line) in all_text.lines().enumerate() {
                let check_line = if case_sensitive {
                    line.to_string()
                } else {
                    line.to_lowercase()
                };
                if check_line.contains(&search_query) {
                    if let Some(pos) = check_line.find(&search_query) {
                        results.push(SearchResult {
                            file_path: path.to_path_buf(),
                            file_name: path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string(),
                            line_number: line_num + 1,
                            line_content: line.trim().to_string(),
                            match_start: pos,
                            match_end: pos + search_query.len(),
                        });
                    }
                }
            }
        }
    }

    results
}

fn search_xlsx_content(path: &Path, query: &str, case_sensitive: bool) -> Vec<SearchResult> {
    let mut results = Vec::new();

    macro_rules! search_workbook {
        ($workbook:expr) => {{
            let sheet_names = $workbook.sheet_names().to_vec();
            let search_query = if case_sensitive {
                query.to_string()
            } else {
                query.to_lowercase()
            };

            for sheet_name in sheet_names {
                if let Ok(range) = $workbook.worksheet_range(&sheet_name) {
                    let (rows, cols) = range.get_size();
                    for row in 0..rows {
                        for col in 0..cols {
                            if let Some(cell) = range.get((row, col)) {
                                let cell_text = cell.to_string();
                                let check_text = if case_sensitive {
                                    cell_text.clone()
                                } else {
                                    cell_text.to_lowercase()
                                };

                                if check_text.contains(&search_query) {
                                    if let Some(pos) = check_text.find(&search_query) {
                                        let col_letter = if col < 26 {
                                            format!("{}", (b'A' + col as u8) as char)
                                        } else {
                                            format!(
                                                "{}{}",
                                                (b'A' + (col / 26 - 1) as u8) as char,
                                                (b'A' + (col % 26) as u8) as char
                                            )
                                        };

                                        results.push(SearchResult {
                                            file_path: path.to_path_buf(),
                                            file_name: format!(
                                                "{} -> {} [{}{}]",
                                                path.file_name()
                                                    .unwrap_or_default()
                                                    .to_string_lossy(),
                                                sheet_name,
                                                col_letter,
                                                row + 1
                                            ),
                                            line_number: row + 1,
                                            line_content: cell_text.trim().to_string(),
                                            match_start: pos,
                                            match_end: pos + search_query.len(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }};
    }

    if let Ok(mut workbook) = open_workbook::<Xlsx<_>, _>(path) {
        search_workbook!(workbook);
    } else if let Ok(mut workbook) = open_workbook::<Xls<_>, _>(path) {
        search_workbook!(workbook);
    }

    results
}

pub fn perform_search(
    query: &str,
    root: &Path,
    options: &SearchOptions,
    progress_tx: &Sender<IoResult>,
) -> Result<Vec<SearchResult>, Box<dyn std::error::Error>> {
    let mut all_results = Vec::new();
    let mut file_count = 0;

    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(!options.case_sensitive)
        .build(query)?;

    let walker = WalkBuilder::new(root)
        .hidden(!options.search_hidden)
        .build();

    for entry in walker {
        let entry = entry?;
        let path = entry.path();

        if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
            continue;
        }

        file_count += 1;
        if file_count % 10 == 0 {
            let _ = progress_tx.send(IoResult::SearchProgress(file_count));
        }

        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let mut file_results = match extension.as_str() {
            "pdf" if options.search_pdfs => search_pdf_content(path, query, options.case_sensitive),
            "zip" if options.search_archives => {
                search_zip_archive(path, query, options.case_sensitive)
            }
            "docx" | "doc" => search_docx_content(path, query, options.case_sensitive),
            "xlsx" | "xls" => search_xlsx_content(path, query, options.case_sensitive),
            _ => {
                match search_text_file(path, &matcher, options.max_results - all_results.len()) {
                    Ok(results) => results,
                    Err(_) => Vec::new(),
                }
            }
        };

        all_results.append(&mut file_results);

        if all_results.len() >= options.max_results {
            break;
        }
    }

    Ok(all_results)
}
