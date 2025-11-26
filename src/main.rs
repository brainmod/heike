mod layout;

use calamine::{open_workbook, Reader, Xls, Xlsx};
use chrono::{DateTime, Local};
use docx_rs::read_docx;
use eframe::egui;
use grep_matcher::Matcher;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::{Searcher, Sink, SinkMatch};
use id3::TagLike;
use ignore::WalkBuilder;
use lopdf::Document as PdfDocument;
use notify::{Event, RecursiveMode, Watcher};
use pulldown_cmark::{Event as MarkdownEvent, HeadingLevel, Parser, Tag, TagEnd};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime};
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;
use tar::Archive;
use zip::ZipArchive;

// --- Data Structures ---

#[derive(Clone, Copy, Debug, PartialEq)]
enum Theme {
    Light,
    Dark,
}

#[derive(Clone, Debug)]
struct FileEntry {
    path: PathBuf,
    name: String,
    is_dir: bool,
    is_symlink: bool,
    size: u64,
    modified: SystemTime,
    extension: String,
}

impl FileEntry {
    fn from_path(path: PathBuf) -> Option<Self> {
        // Use symlink_metadata to detect symlinks without following them
        let symlink_meta = fs::symlink_metadata(&path).ok()?;
        let is_symlink = symlink_meta.is_symlink();

        let name = path.file_name()?.to_string_lossy().to_string();
        let extension = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        // Prefer real metadata (follows symlinks) but fall back to the link metadata
        let metadata = fs::metadata(&path).ok();
        let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);
        let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
        let modified = metadata
            .as_ref()
            .and_then(|m| m.modified().ok())
            .or_else(|| symlink_meta.modified().ok())
            .unwrap_or(SystemTime::now());

        Some(Self {
            path,
            name,
            is_dir,
            is_symlink,
            size,
            modified,
            extension,
        })
    }

    fn get_icon(&self) -> &str {
        // Using Nerd Font icons for consistent rendering
        // Icons use Unicode escape sequences for the Nerd Font glyphs
        if self.is_dir {
            return "\u{f07b}";
        } // folder icon
        match self.extension.as_str() {
            "rs" => "\u{e7a8}",
            "toml" => "\u{e615}",
            "md" => "\u{e73e}",
            "txt" => "\u{f15c}",
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" => "\u{f1c5}",
            "mp4" | "mkv" | "mov" | "avi" | "webm" => "\u{f03d}",
            "mp3" | "wav" | "flac" | "ogg" | "m4a" => "\u{f001}",
            "zip" | "tar" | "gz" | "7z" | "rar" | "xz" | "bz2" => "\u{f410}",
            "py" => "\u{e73c}",
            "pyc" => "\u{e73c}",
            "js" | "mjs" => "\u{e74e}",
            "ts" | "tsx" => "\u{e628}",
            "jsx" => "\u{e7ba}",
            "html" | "htm" => "\u{e736}",
            "css" | "scss" | "sass" => "\u{e749}",
            "json" => "\u{e60b}",
            "yaml" | "yml" => "\u{e615}",
            "xml" => "\u{e619}",
            "pdf" => "\u{f1c1}",
            "doc" | "docx" => "\u{f1c2}",
            "xls" | "xlsx" => "\u{f1c3}",
            "exe" | "msi" => "\u{f17a}",
            "bat" | "cmd" => "\u{e795}",
            "sh" | "bash" | "zsh" => "\u{f489}",
            "c" | "h" => "\u{e61e}",
            "cpp" | "cc" | "cxx" | "hpp" => "\u{e61d}",
            "java" => "\u{e738}",
            "class" | "jar" => "\u{e738}",
            "go" => "\u{e626}",
            "rb" => "\u{e739}",
            "php" => "\u{e73d}",
            "sql" | "db" | "sqlite" => "\u{f1c0}",
            "env" => "\u{f462}",
            "lock" => "\u{f023}",
            "log" => "\u{f18d}",
            "git" | "gitignore" => "\u{e725}",
            _ => "\u{f15b}",
        }
    }

    fn display_name(&self) -> String {
        if self.is_symlink {
            format!("{} \u{2192}", self.name)
        } else {
            self.name.clone()
        }
    }
}

// --- Search Data Structures ---

#[derive(Clone, Debug, PartialEq)]
struct SearchResult {
    file_path: PathBuf,
    file_name: String,
    line_number: usize,
    line_content: String,
    match_start: usize,
    match_end: usize,
}

#[derive(Clone, Debug)]
struct SearchOptions {
    case_sensitive: bool,
    use_regex: bool,
    search_hidden: bool,
    search_pdfs: bool,
    search_archives: bool,
    max_results: usize,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            use_regex: false,
            search_hidden: false,
            search_pdfs: true,
            search_archives: true,
            max_results: 1000,
        }
    }
}

// --- Modes ---

#[derive(Debug, PartialEq, Clone)]
enum AppMode {
    Normal,
    Visual,
    Filtering,
    Command,
    Help,
    Rename,
    DeleteConfirm,
    SearchInput,
    SearchResults {
        query: String,
        results: Vec<SearchResult>,
        selected_index: usize,
    },
}

#[derive(Clone, Copy, PartialEq)]
enum ClipboardOp {
    Copy,
    Cut,
} // New

// --- Async Architecture ---

enum IoCommand {
    LoadDirectory(PathBuf, bool),
    LoadParent(PathBuf, bool),
    SearchContent {
        query: String,
        root_path: PathBuf,
        options: SearchOptions,
    },
}

enum IoResult {
    DirectoryLoaded {
        path: PathBuf,
        entries: Vec<FileEntry>,
    },
    ParentLoaded(Vec<FileEntry>),
    SearchCompleted(Vec<SearchResult>),
    SearchProgress(usize),
    Error(String),
}

fn read_directory(path: &Path, show_hidden: bool) -> Result<Vec<FileEntry>, std::io::Error> {
    let mut entries = Vec::new();
    let read_dir = fs::read_dir(path)?;
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !show_hidden {
            if let Some(name) = path.file_name() {
                if name.to_string_lossy().starts_with('.') {
                    continue;
                }
            }
        }
        if let Some(file_entry) = FileEntry::from_path(path) {
            entries.push(file_entry);
        }
    }
    entries.sort_by(|a, b| {
        if a.is_dir != b.is_dir {
            return b.is_dir.cmp(&a.is_dir);
        }
        a.name.to_lowercase().cmp(&b.name.to_lowercase())
    });
    Ok(entries)
}

fn fuzzy_match(text: &str, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let mut q_chars = query.chars();
    let mut q_char = match q_chars.next() {
        Some(c) => c,
        None => return true,
    };
    for t_char in text.chars() {
        if t_char.eq_ignore_ascii_case(&q_char) {
            q_char = match q_chars.next() {
                Some(c) => c,
                None => return true,
            };
        }
    }
    false
}

fn is_likely_binary(path: &Path) -> bool {
    let mut buf = [0u8; 8192];
    if let Ok(mut f) = fs::File::open(path) {
        if let Ok(n) = std::io::Read::read(&mut f, &mut buf) {
            if n == 0 {
                return false; // Empty files are not binary
            }
            // Check for null bytes (binary indicator)
            // But allow UTF-8 BOM and other common text markers
            let null_count = buf[..n].iter().filter(|&&b| b == 0).count();
            // If more than 1% null bytes, consider it binary
            return null_count > (n / 100).max(1);
        }
    }
    false
}

// --- Search Implementation ---

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
            return Ok(false); // Stop searching
        }

        let line_number = mat.line_number().unwrap_or(0) as usize;
        let line_content = String::from_utf8_lossy(mat.bytes()).to_string();

        // Find match position in the line
        let (match_start, match_end) = if mat.bytes().iter().position(|_| true).is_some() {
            (0, line_content.len().min(100)) // Simplified for now
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

        // Extract text from all pages
        let pages = doc.get_pages();
        let page_numbers: Vec<u32> = pages.keys().cloned().collect();
        if let Ok(text) = doc.extract_text(&page_numbers) {
            all_text.push_str(&text);
        }

        // Search through extracted text
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
            // Extract text from paragraphs
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

            // Search through extracted text
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

    // Helper macro to search a workbook
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

    // Try XLSX first, then fall back to XLS
    if let Ok(mut workbook) = open_workbook::<Xlsx<_>, _>(path) {
        search_workbook!(workbook);
    } else if let Ok(mut workbook) = open_workbook::<Xls<_>, _>(path) {
        search_workbook!(workbook);
    }

    results
}

fn perform_search(
    query: &str,
    root: &Path,
    options: &SearchOptions,
    progress_tx: &Sender<IoResult>,
) -> Result<Vec<SearchResult>, Box<dyn std::error::Error>> {
    let mut all_results = Vec::new();
    let mut file_count = 0;

    // Build regex matcher
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(!options.case_sensitive)
        .build(query)?;

    // Build file walker with gitignore support
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

        // Determine file type and search accordingly
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
            // Text files and source code
            _ => {
                match search_text_file(path, &matcher, options.max_results - all_results.len()) {
                    Ok(results) => results,
                    Err(_) => Vec::new(), // Skip binary files that can't be read as text
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

// --- Main App Struct ---

struct Heike {
    // Core State
    current_path: PathBuf,
    history: Vec<PathBuf>,
    history_index: usize,

    all_entries: Vec<FileEntry>,
    visible_entries: Vec<FileEntry>,
    parent_entries: Vec<FileEntry>,

    // Navigation State
    selected_index: Option<usize>,
    multi_selection: HashSet<PathBuf>,
    directory_selections: HashMap<PathBuf, usize>, // Track last selected index per directory
    pending_selection_path: Option<PathBuf>,       // Track item to select after navigation

    // Mode State
    mode: AppMode,
    command_buffer: String,
    focus_input: bool,

    // Clipboard State (New)
    clipboard: HashSet<PathBuf>,
    clipboard_op: Option<ClipboardOp>,

    // Search State
    search_query: String,
    search_options: SearchOptions,
    search_in_progress: bool,
    search_file_count: usize,

    // UI State
    error_message: Option<(String, Instant)>, // Changed to include timestamp
    info_message: Option<(String, Instant)>,  // Changed to include timestamp
    show_hidden: bool,
    theme: Theme,
    is_loading: bool,
    last_g_press: Option<Instant>,
    last_selection_change: Instant,
    disable_autoscroll: bool,

    // Layout State (Strip-based layout)
    panel_widths: [f32; 2], // [parent, preview] - current is remainder
    dragging_divider: Option<usize>,
    last_screen_size: egui::Vec2,

    // Async Communication
    command_tx: Sender<IoCommand>,
    result_rx: Receiver<IoResult>,

    // File System Watcher
    watcher: Option<Box<dyn Watcher>>,
    watcher_rx: Receiver<Result<Event, notify::Error>>,
    watched_path: Option<PathBuf>,

    // Syntax Highlighting
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

impl Heike {
    fn new(ctx: egui::Context) -> Self {
        let start_path = directories::UserDirs::new()
            .map(|ud| ud.home_dir().to_path_buf())
            .unwrap_or_else(|| env::current_dir().unwrap_or_default());

        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
        let (res_tx, res_rx) = std::sync::mpsc::channel();
        let (_watch_tx, watch_rx) = channel();

        let ctx_clone = ctx.clone();
        thread::spawn(move || {
            while let Ok(cmd) = cmd_rx.recv() {
                match cmd {
                    IoCommand::LoadDirectory(path, hidden) => match read_directory(&path, hidden) {
                        Ok(entries) => {
                            let _ = res_tx.send(IoResult::DirectoryLoaded {
                                path: path.clone(),
                                entries,
                            });
                        }
                        Err(e) => {
                            let _ = res_tx.send(IoResult::Error(e.to_string()));
                        }
                    },
                    IoCommand::LoadParent(path, hidden) => match read_directory(&path, hidden) {
                        Ok(entries) => {
                            let _ = res_tx.send(IoResult::ParentLoaded(entries));
                        }
                        Err(_) => {
                            let _ = res_tx.send(IoResult::ParentLoaded(Vec::new()));
                        }
                    },
                    IoCommand::SearchContent {
                        query,
                        root_path,
                        options,
                    } => match perform_search(&query, &root_path, &options, &res_tx) {
                        Ok(results) => {
                            let _ = res_tx.send(IoResult::SearchCompleted(results));
                        }
                        Err(e) => {
                            let _ = res_tx.send(IoResult::Error(format!("Search error: {}", e)));
                        }
                    },
                }
                ctx_clone.request_repaint();
            }
        });

        let mut app = Self {
            current_path: start_path.clone(),
            history: vec![start_path.clone()],
            history_index: 0,
            all_entries: Vec::new(),
            visible_entries: Vec::new(),
            parent_entries: Vec::new(),
            selected_index: Some(0),
            multi_selection: HashSet::new(),
            directory_selections: HashMap::new(),
            pending_selection_path: None,
            mode: AppMode::Normal,
            command_buffer: String::new(),
            focus_input: false,
            clipboard: HashSet::new(), // Init
            clipboard_op: None,        // Init
            search_query: String::new(),
            search_options: SearchOptions::default(),
            search_in_progress: false,
            search_file_count: 0,
            error_message: None,
            info_message: None,
            show_hidden: false,
            theme: Theme::Dark,
            is_loading: false,
            last_g_press: None,
            last_selection_change: Instant::now(),
            disable_autoscroll: false,
            panel_widths: [layout::PARENT_DEFAULT, layout::PREVIEW_DEFAULT],
            dragging_divider: None,
            last_screen_size: egui::Vec2::ZERO,
            command_tx: cmd_tx,
            result_rx: res_rx,
            watcher: None,
            watcher_rx: watch_rx,
            watched_path: None,
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        };

        app.request_refresh();
        app
    }

    fn request_refresh(&mut self) {
        self.is_loading = true;
        self.error_message = None;
        // Keep info message if it's fresh, or maybe clear it? Let's keep it for feedback.
        let _ = self.command_tx.send(IoCommand::LoadDirectory(
            self.current_path.clone(),
            self.show_hidden,
        ));
        if let Some(parent) = self.current_path.parent() {
            let _ = self.command_tx.send(IoCommand::LoadParent(
                parent.to_path_buf(),
                self.show_hidden,
            ));
        } else {
            self.parent_entries.clear();
        }
    }

    fn apply_filter(&mut self) {
        // Save currently selected item path before filtering
        let previously_selected = self
            .selected_index
            .and_then(|idx| self.visible_entries.get(idx))
            .map(|e| e.path.clone());

        if self.mode == AppMode::Filtering && !self.command_buffer.is_empty() {
            let query = self.command_buffer.clone();
            self.visible_entries = self
                .all_entries
                .iter()
                .filter(|e| fuzzy_match(&e.name, &query))
                .cloned()
                .collect();
        } else {
            self.visible_entries = self.all_entries.clone();
        }

        // Restore selection to previously selected item if possible
        if let Some(path) = previously_selected {
            if let Some(idx) = self.visible_entries.iter().position(|e| e.path == path) {
                self.selected_index = Some(idx);
            } else if !self.visible_entries.is_empty() {
                self.selected_index = Some(0);
            } else {
                self.selected_index = None;
            }
        } else if self.visible_entries.is_empty() {
            self.selected_index = None;
        } else if self.selected_index.is_none() {
            self.selected_index = Some(0);
        }
        self.validate_selection();
    }

    fn setup_watcher(&mut self, ctx: &egui::Context) {
        // Only setup if path changed
        if self.watched_path.as_ref() == Some(&self.current_path) {
            return;
        }

        // Get the channel sender for watcher events
        let (tx, rx) = channel();
        self.watcher_rx = rx;

        // Create the watcher
        let ctx_clone = ctx.clone();
        match notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            let _ = tx.send(res);
            ctx_clone.request_repaint();
        }) {
            Ok(mut watcher) => {
                // Watch the current directory
                if let Err(e) = watcher.watch(&self.current_path, RecursiveMode::NonRecursive) {
                    self.error_message =
                        Some((format!("Failed to watch directory: {}", e), Instant::now()));
                    self.watcher = None;
                    self.watched_path = None;
                } else {
                    self.watcher = Some(Box::new(watcher));
                    self.watched_path = Some(self.current_path.clone());
                }
            }
            Err(e) => {
                self.error_message =
                    Some((format!("Failed to create watcher: {}", e), Instant::now()));
                self.watcher = None;
                self.watched_path = None;
            }
        }
    }

    fn process_watcher_events(&mut self) {
        while let Ok(event_result) = self.watcher_rx.try_recv() {
            match event_result {
                Ok(_event) => {
                    // File system changed, trigger refresh
                    self.request_refresh();
                }
                Err(e) => {
                    // Watcher error, but don't show it to avoid spam
                    eprintln!("Watcher error: {}", e);
                }
            }
        }
    }

    fn process_async_results(&mut self) {
        while let Ok(result) = self.result_rx.try_recv() {
            match result {
                IoResult::DirectoryLoaded { path, entries } => {
                    if path != self.current_path {
                        continue;
                    }

                    self.all_entries = entries;
                    self.is_loading = false;
                    self.apply_filter();

                    // If there's a pending selection path, find and select it
                    if let Some(pending_path) = self.pending_selection_path.take() {
                        if let Some(idx) = self
                            .visible_entries
                            .iter()
                            .position(|e| e.path == pending_path)
                        {
                            self.selected_index = Some(idx);
                        }
                    }

                    // Validate selection after loading
                    if let Some(idx) = self.selected_index {
                        if idx >= self.visible_entries.len() && !self.visible_entries.is_empty() {
                            self.selected_index = Some(self.visible_entries.len() - 1);
                        }
                    }
                }
                IoResult::ParentLoaded(entries) => {
                    self.parent_entries = entries;
                }
                IoResult::SearchCompleted(results) => {
                    self.search_in_progress = false;
                    let result_count = results.len();
                    self.mode = AppMode::SearchResults {
                        query: self.search_query.clone(),
                        results,
                        selected_index: 0,
                    };
                    self.info_message = Some((
                        format!(
                            "Found {} matches in {} files",
                            result_count, self.search_file_count
                        ),
                        Instant::now(),
                    ));
                }
                IoResult::SearchProgress(count) => {
                    self.search_file_count = count;
                }
                IoResult::Error(msg) => {
                    self.is_loading = false;
                    self.search_in_progress = false;
                    self.error_message = Some((msg, Instant::now()));
                    self.all_entries.clear();
                    self.visible_entries.clear();
                }
            }
        }
    }

    // --- Navigation Logic ---

    fn navigate_to(&mut self, path: PathBuf) {
        if path.is_dir() {
            // Save current selection before navigating away
            if let Some(idx) = self.selected_index {
                self.directory_selections
                    .insert(self.current_path.clone(), idx);
            }

            self.current_path = path.clone();

            if self.history_index < self.history.len() - 1 {
                self.history.truncate(self.history_index + 1);
            }
            self.history.push(path);
            self.history_index = self.history.len() - 1;

            self.finish_navigation();
        } else if let Err(e) = open::that(&path) {
            self.error_message = Some((format!("Could not open file: {}", e), Instant::now()));
        }
    }

    fn navigate_up(&mut self) {
        if let Some(parent) = self.current_path.parent() {
            // Save current selection before navigating up
            if let Some(idx) = self.selected_index {
                self.directory_selections
                    .insert(self.current_path.clone(), idx);
            }
            self.navigate_to(parent.to_path_buf());
        }
    }

    fn navigate_back(&mut self) {
        if self.history_index == 0 {
            return;
        }

        if let Some(idx) = self.selected_index {
            self.directory_selections
                .insert(self.current_path.clone(), idx);
        }

        let mut idx = self.history_index;
        while idx > 0 {
            idx -= 1;
            let target = self.history[idx].clone();
            if target.is_dir() {
                self.history_index = idx;
                self.current_path = target;
                self.finish_navigation();
                return;
            } else {
                self.history.remove(idx);
                self.history_index -= 1;
            }
        }

        self.error_message = Some(("Previous directory no longer exists".into(), Instant::now()));
    }

    fn navigate_forward(&mut self) {
        if self.history_index >= self.history.len() - 1 {
            return;
        }

        if let Some(idx) = self.selected_index {
            self.directory_selections
                .insert(self.current_path.clone(), idx);
        }

        let idx = self.history_index + 1;
        loop {
            if idx >= self.history.len() {
                break;
            }
            let target = self.history[idx].clone();
            if target.is_dir() {
                self.history_index = idx;
                self.current_path = target;
                self.finish_navigation();
                return;
            }
            self.history.remove(idx);
        }

        self.error_message = Some(("Next directory no longer exists".into(), Instant::now()));
    }

    fn finish_navigation(&mut self) {
        self.command_buffer.clear();
        self.mode = AppMode::Normal;
        self.multi_selection.clear();
        // Restore saved selection for this directory, or default to 0
        self.selected_index = self
            .directory_selections
            .get(&self.current_path)
            .copied()
            .or(Some(0));
        self.request_refresh();
    }

    // --- File Operations (Injected) ---

    fn yank_selection(&mut self, op: ClipboardOp) {
        self.clipboard.clear();
        self.clipboard_op = Some(op);

        if !self.multi_selection.is_empty() {
            self.clipboard = self.multi_selection.clone();
            self.mode = AppMode::Normal;
            self.multi_selection.clear();
        } else if let Some(idx) = self.selected_index {
            if let Some(entry) = self.visible_entries.get(idx) {
                self.clipboard.insert(entry.path.clone());
            }
        }

        let op_text = if self.clipboard_op == Some(ClipboardOp::Copy) {
            "Yanked"
        } else {
            "Cut"
        };
        self.info_message = Some((
            format!("{} {} files", op_text, self.clipboard.len()),
            Instant::now(),
        ));
    }

    fn paste_clipboard(&mut self) {
        if self.clipboard.is_empty() {
            return;
        }
        let op = match self.clipboard_op {
            Some(o) => o,
            None => return,
        };

        let mut count = 0;
        let mut errors = Vec::new();
        let mut missing_paths = Vec::new();

        for src in &self.clipboard {
            if !src.exists() {
                errors.push(format!("Source missing: {}", src.display()));
                missing_paths.push(src.clone());
                continue;
            }

            if let Some(name) = src.file_name() {
                let dest = self.current_path.join(name);
                if src.is_dir() {
                    if op == ClipboardOp::Cut {
                        if let Err(e) = fs::rename(src, &dest) {
                            errors.push(format!("Move dir failed: {}", e));
                        } else {
                            count += 1;
                        }
                    } else {
                        errors.push("Copying directories not supported in  Heike (lite)".into());
                    }
                } else if op == ClipboardOp::Copy {
                    if let Err(e) = fs::copy(src, &dest) {
                        errors.push(format!("Copy file failed: {}", e));
                    } else {
                        count += 1;
                    }
                } else if let Err(e) = fs::rename(src, &dest) {
                    errors.push(format!("Move file failed: {}", e));
                } else {
                    count += 1;
                }
            }
        }

        for path in missing_paths {
            self.clipboard.remove(&path);
        }

        if !errors.is_empty() {
            self.error_message = Some((errors.join(" | "), Instant::now()));
        } else {
            self.info_message = Some((format!("Processed {} files", count), Instant::now()));
        }

        if op == ClipboardOp::Cut {
            self.clipboard.clear();
            self.clipboard_op = None;
        }
        self.request_refresh();
    }

    fn perform_delete(&mut self) {
        let targets = if !self.multi_selection.is_empty() {
            self.multi_selection.clone()
        } else if let Some(idx) = self.selected_index {
            if let Some(entry) = self.visible_entries.get(idx) {
                HashSet::from([entry.path.clone()])
            } else {
                HashSet::new()
            }
        } else {
            HashSet::new()
        };

        for path in targets {
            if path.is_dir() {
                let _ = fs::remove_dir_all(&path);
            } else {
                let _ = fs::remove_file(&path);
            }
        }

        self.mode = AppMode::Normal;
        self.multi_selection.clear();
        self.request_refresh();
        self.info_message = Some(("Items deleted".into(), Instant::now()));
    }

    fn perform_rename(&mut self) {
        if let Some(idx) = self.selected_index {
            if let Some(entry) = self.visible_entries.get(idx) {
                let new_name = self.command_buffer.trim();
                if !new_name.is_empty() {
                    let new_path = entry.path.parent().unwrap().join(new_name);
                    if let Err(e) = fs::rename(&entry.path, &new_path) {
                        self.error_message =
                            Some((format!("Rename failed: {}", e), Instant::now()));
                    } else {
                        self.info_message = Some(("Renamed successfully".into(), Instant::now()));
                    }
                }
            }
        }
        self.mode = AppMode::Normal;
        self.command_buffer.clear();
        self.request_refresh();
    }

    // --- Selection Validation ---

    fn validate_selection(&mut self) {
        if let Some(idx) = self.selected_index {
            if self.visible_entries.is_empty() {
                self.selected_index = None;
            } else if idx >= self.visible_entries.len() {
                self.selected_index = Some(self.visible_entries.len() - 1);
            }
        }
    }

    // --- Drag and Drop Handling ---

    fn handle_dropped_files(&mut self, dropped_files: &[egui::DroppedFile]) {
        let mut count = 0;
        let mut errors = Vec::new();

        for file in dropped_files {
            if let Some(path) = &file.path {
                let dest = self.current_path.join(path.file_name().unwrap_or_default());

                // Copy the dropped file to current directory
                if path.is_dir() {
                    errors.push("Copying directories not supported".into());
                } else {
                    match fs::copy(path, &dest) {
                        Ok(_) => count += 1,
                        Err(e) => errors.push(format!("Copy failed: {}", e)),
                    }
                }
            }
        }

        if !errors.is_empty() {
            self.error_message = Some((errors.join(" | "), Instant::now()));
        } else if count > 0 {
            self.info_message = Some((format!("Copied {} file(s)", count), Instant::now()));
        }

        if count > 0 {
            self.request_refresh();
        }
    }

    // --- Input Handling ---

    fn execute_command(&mut self, ctx: &egui::Context) {
        let cmd = self.command_buffer.trim().to_string();
        self.command_buffer.clear();
        self.mode = AppMode::Normal;
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.is_empty() {
            return;
        }

        match parts[0] {
            "q" | "quit" => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            "mkdir" => {
                if parts.len() > 1 {
                    let new_dir = self.current_path.join(parts[1]);
                    // Security: Canonicalize paths and verify they're within current directory
                    match (new_dir.canonicalize().or_else(|_| {
                        // If path doesn't exist yet, canonicalize parent and append last component
                        if let Some(parent) = new_dir.parent() {
                            parent.canonicalize().map(|p| {
                                if let Some(name) = new_dir.file_name() {
                                    p.join(name)
                                } else {
                                    p
                                }
                            })
                        } else {
                            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "Invalid path"))
                        }
                    }), self.current_path.canonicalize()) {
                        (Ok(target), Ok(current)) => {
                            if target.starts_with(&current) {
                                if let Err(e) = fs::create_dir(&new_dir) {
                                    self.error_message = Some((format!("mkdir failed: {}", e), Instant::now()));
                                } else {
                                    self.request_refresh();
                                }
                            } else {
                                self.error_message = Some(("Path traversal not allowed".into(), Instant::now()));
                            }
                        }
                        _ => {
                            self.error_message = Some(("Invalid path".into(), Instant::now()));
                        }
                    }
                }
            }
            "touch" => {
                if parts.len() > 1 {
                    let new_file = self.current_path.join(parts[1]);
                    // Security: Canonicalize paths and verify they're within current directory
                    match (new_file.parent().and_then(|p| p.canonicalize().ok()), self.current_path.canonicalize()) {
                        (Some(parent), Ok(current)) => {
                            if parent.starts_with(&current) {
                                if let Err(e) = fs::File::create(&new_file) {
                                    self.error_message = Some((format!("touch failed: {}", e), Instant::now()));
                                } else {
                                    self.request_refresh();
                                }
                            } else {
                                self.error_message = Some(("Path traversal not allowed".into(), Instant::now()));
                            }
                        }
                        _ => {
                            self.error_message = Some(("Invalid path".into(), Instant::now()));
                        }
                    }
                }
            }
            _ => {
                self.error_message =
                    Some((format!("Unknown command: {}", parts[0]), Instant::now()));
            }
        }
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        // 1. Modal Inputs (Command, Filter, Rename, SearchInput)
        if matches!(
            self.mode,
            AppMode::Command | AppMode::Filtering | AppMode::Rename | AppMode::SearchInput
        ) {
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                match self.mode {
                    AppMode::Rename => self.perform_rename(),
                    AppMode::Command => self.execute_command(ctx),
                    AppMode::Filtering => {
                        // Finalize search and allow navigation in filtered results
                        self.mode = AppMode::Normal;
                        // Keep the filtered results
                    }
                    AppMode::SearchInput => {
                        // Start search
                        if !self.search_query.is_empty() {
                            self.search_in_progress = true;
                            self.search_file_count = 0;
                            let _ = self.command_tx.send(IoCommand::SearchContent {
                                query: self.search_query.clone(),
                                root_path: self.current_path.clone(),
                                options: self.search_options.clone(),
                            });
                        }
                        self.mode = AppMode::Normal;
                    }
                    _ => {}
                }
            }
            if self.mode == AppMode::Filtering && !ctx.input(|i| i.pointer.any_pressed()) {
                // Implicitly handled
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.mode = AppMode::Normal;
                self.command_buffer.clear();
                self.apply_filter();
            }
            return;
        }

        // 2. Confirmation Modals
        if self.mode == AppMode::DeleteConfirm {
            if ctx.input(|i| i.key_pressed(egui::Key::Y) || i.key_pressed(egui::Key::Enter)) {
                self.perform_delete();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::N) || i.key_pressed(egui::Key::Escape)) {
                self.mode = AppMode::Normal;
            }
            return;
        }

        if self.mode == AppMode::Help {
            if ctx.input(|i| {
                i.key_pressed(egui::Key::Escape)
                    || i.key_pressed(egui::Key::Q)
                    || i.key_pressed(egui::Key::Questionmark)
            }) {
                self.mode = AppMode::Normal;
            }
            return;
        }

        // Handle SearchResults mode navigation
        if let AppMode::SearchResults {
            query: ref current_query,
            ref results,
            ref mut selected_index,
        } = self.mode
        {
            if ctx.input(|i| i.key_pressed(egui::Key::S) && i.modifiers.shift) {
                self.search_query = current_query.clone();
                self.search_in_progress = false;
                self.search_file_count = 0;
                self.mode = AppMode::SearchInput;
                self.focus_input = true;
                return;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.mode = AppMode::Normal;
                return;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::N) && !i.modifiers.shift) {
                if !results.is_empty() {
                    *selected_index = (*selected_index + 1) % results.len();
                }
                return;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::N) && i.modifiers.shift) {
                if !results.is_empty() {
                    *selected_index = if *selected_index == 0 {
                        results.len() - 1
                    } else {
                        *selected_index - 1
                    };
                }
                return;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                // Open the file at the match location
                if let Some(result) = results.get(*selected_index) {
                    if result.file_path.is_file() {
                        let _ = open::that(&result.file_path);
                    }
                }
                return;
            }
            // Allow other navigation within search results
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::J)) {
                if !results.is_empty() {
                    *selected_index = (*selected_index + 1) % results.len();
                }
                return;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::K)) {
                if !results.is_empty() {
                    *selected_index = if *selected_index == 0 {
                        results.len() - 1
                    } else {
                        *selected_index - 1
                    };
                }
                return;
            }
            return; // Don't process other keys in search results mode
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.mode = AppMode::Normal;
            self.command_buffer.clear();
            self.multi_selection.clear();
            self.apply_filter();
            return;
        }

        // 3. Global History keys
        if ctx.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::ArrowLeft)) {
            self.navigate_back();
            return;
        }
        if ctx.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::ArrowRight)) {
            self.navigate_forward();
            return;
        }

        // 4. Normal Mode Triggers
        if ctx.input(|i| i.key_pressed(egui::Key::Colon)) {
            self.mode = AppMode::Command;
            self.focus_input = true;
            self.command_buffer.clear();
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Slash)) {
            self.mode = AppMode::Filtering;
            self.focus_input = true;
            self.command_buffer.clear();
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Period)) {
            self.show_hidden = !self.show_hidden;
            self.request_refresh();
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Questionmark)) {
            self.mode = AppMode::Help;
            return;
        }
        if self.mode == AppMode::Normal
            && ctx.input(|i| i.key_pressed(egui::Key::V) && !i.modifiers.shift)
        {
            self.mode = AppMode::Visual;
            if let Some(idx) = self.selected_index {
                if let Some(entry) = self.visible_entries.get(idx) {
                    self.multi_selection.insert(entry.path.clone());
                }
            }
            return;
        }
        if self.mode == AppMode::Normal
            && ctx.input(|i| i.key_pressed(egui::Key::V) && i.modifiers.shift)
        {
            // Shift+V: Enter visual mode and select all
            self.mode = AppMode::Visual;
            self.multi_selection.clear();
            for entry in &self.visible_entries {
                self.multi_selection.insert(entry.path.clone());
            }
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::A) && i.modifiers.ctrl) {
            // Ctrl+A: Select all
            if self.mode != AppMode::Visual {
                self.mode = AppMode::Visual;
            }
            self.multi_selection.clear();
            for entry in &self.visible_entries {
                self.multi_selection.insert(entry.path.clone());
            }
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
            // Space: Toggle selection of current item
            if let Some(idx) = self.selected_index {
                if let Some(entry) = self.visible_entries.get(idx) {
                    if self.multi_selection.contains(&entry.path) {
                        self.multi_selection.remove(&entry.path);
                    } else {
                        if self.mode != AppMode::Visual {
                            self.mode = AppMode::Visual;
                        }
                        self.multi_selection.insert(entry.path.clone());
                    }
                }
            }
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::S) && i.modifiers.shift) {
            self.search_in_progress = false;
            self.search_file_count = 0;
            self.mode = AppMode::SearchInput;
            self.focus_input = true;
            return;
        }

        // 5. File Operation Triggers (Phase 6)
        if ctx.input(|i| i.key_pressed(egui::Key::Y)) {
            self.yank_selection(ClipboardOp::Copy);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::X)) {
            self.yank_selection(ClipboardOp::Cut);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::P)) {
            self.paste_clipboard();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::D)) {
            self.mode = AppMode::DeleteConfirm;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::R)) {
            if let Some(idx) = self.selected_index {
                if let Some(entry) = self.visible_entries.get(idx) {
                    self.command_buffer = entry.name.clone();
                    self.mode = AppMode::Rename;
                    self.focus_input = true;
                }
            }
        }

        // 6. Navigation (j/k/arrows)
        if self.visible_entries.is_empty() {
            if ctx.input(|i| {
                i.key_pressed(egui::Key::Backspace)
                    || i.key_pressed(egui::Key::H)
                    || i.key_pressed(egui::Key::ArrowLeft)
            }) {
                self.navigate_up();
            }
            return;
        }

        let mut changed = false;
        let max_idx = self.visible_entries.len() - 1;
        let current = self.selected_index.unwrap_or(0);
        let mut new_index = current;

        if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::J)) {
            new_index = if current >= max_idx { 0 } else { current + 1 };
            changed = true;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::K)) {
            new_index = if current == 0 { max_idx } else { current - 1 };
            changed = true;
        }
        if ctx.input(|i| {
            i.key_pressed(egui::Key::Backspace)
                || i.key_pressed(egui::Key::H)
                || i.key_pressed(egui::Key::ArrowLeft)
        }) {
            self.navigate_up();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            if let Some(idx) = self.selected_index {
                if let Some(entry) = self.visible_entries.get(idx) {
                    let path = entry.path.clone();
                    self.navigate_to(path);
                }
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::L) || i.key_pressed(egui::Key::ArrowRight)) {
            if let Some(idx) = self.selected_index {
                if let Some(entry) = self.visible_entries.get(idx) {
                    if entry.is_dir {
                        let path = entry.path.clone();
                        self.navigate_to(path);
                    }
                }
            }
        }

        if ctx.input(|i| i.key_pressed(egui::Key::G) && i.modifiers.shift) {
            new_index = max_idx;
            changed = true;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::G) && !i.modifiers.shift) {
            let now = Instant::now();
            if let Some(last) = self.last_g_press {
                if now.duration_since(last) < Duration::from_millis(500) {
                    new_index = 0;
                    self.last_g_press = None;
                    changed = true;
                } else {
                    self.last_g_press = Some(now);
                }
            } else {
                self.last_g_press = Some(now);
            }
        }
        if let Some(last) = self.last_g_press {
            if Instant::now().duration_since(last) > Duration::from_millis(500) {
                self.last_g_press = None;
            }
        }

        if changed {
            self.selected_index = Some(new_index);
            self.last_selection_change = Instant::now();
            self.disable_autoscroll = false; // Re-enable autoscroll on keyboard navigation
            if self.mode == AppMode::Visual {
                if let Some(entry) = self.visible_entries.get(new_index) {
                    self.multi_selection.insert(entry.path.clone());
                }
            }
        }
    }

    // --- Preview Helper Functions ---

    fn render_large_file_message(&self, ui: &mut egui::Ui, entry: &FileEntry) {
        ui.centered_and_justified(|ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.label(egui::RichText::new("📄 File Too Large").size(18.0));
                ui.add_space(10.0);
                ui.label(format!("File size: {}", bytesize::ByteSize(entry.size)));
                ui.label(format!(
                    "Preview limit: {}",
                    bytesize::ByteSize(layout::MAX_PREVIEW_SIZE)
                ));
            });
        });
    }

    fn render_syntax_highlighted(&self, ui: &mut egui::Ui, entry: &FileEntry) {
        if entry.size > layout::MAX_PREVIEW_SIZE {
            self.render_large_file_message(ui, entry);
            return;
        }

        match fs::read(&entry.path) {
            Ok(data) => {
                let content = String::from_utf8_lossy(&data);
                let syntax = self
                    .syntax_set
                    .find_syntax_by_extension(&entry.extension)
                    .or_else(|| self.syntax_set.find_syntax_by_first_line(&content))
                    .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

                let theme_name = if self.theme == Theme::Dark {
                    "base16-ocean.dark"
                } else {
                    "base16-ocean.light"
                };
                let theme = &self.theme_set.themes[theme_name];

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
                                .highlight_line(line, &self.syntax_set)
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

    fn render_markdown_preview(&self, ui: &mut egui::Ui, entry: &FileEntry) {
        if entry.size > layout::MAX_PREVIEW_SIZE {
            self.render_large_file_message(ui, entry);
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

    fn render_archive_preview(&self, ui: &mut egui::Ui, entry: &FileEntry) {
        const MAX_PREVIEW_ITEMS: usize = 100; // Limit items to prevent performance issues

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
                        Box::new(flate2::read::GzDecoder::new(file))
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

    fn render_audio_metadata(&self, ui: &mut egui::Ui, entry: &FileEntry) {
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

                    // Show album art if available
                    if let Some(picture) = tag.pictures().next() {
                        ui.label(format!(
                            "Album art: {} ({})",
                            picture.mime_type,
                            bytesize::ByteSize(picture.data.len() as u64)
                        ));
                        // Could render the image here if we decode it
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

    fn render_docx_preview(&self, ui: &mut egui::Ui, entry: &FileEntry) {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label(egui::RichText::new("📄 Word Document").size(18.0));
            ui.add_space(10.0);
        });

        match fs::read(&entry.path) {
            Ok(data) => {
                match read_docx(&data) {
                    Ok(docx) => {
                        // Extract text from paragraphs
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

    fn render_xlsx_preview(&self, ui: &mut egui::Ui, entry: &FileEntry) {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label(egui::RichText::new("📊 Excel Spreadsheet").size(18.0));
            ui.add_space(10.0);
        });

        // Helper macro to reduce code duplication
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
                            // Preview first 3 sheets
                            if let Ok(range) = $workbook.worksheet_range(sheet_name) {
                                ui.add_space(10.0);
                                ui.label(
                                    egui::RichText::new(format!("Sheet: {}", sheet_name)).strong(),
                                );
                                ui.add_space(5.0);

                                // Show dimensions
                                let (rows, cols) = range.get_size();
                                ui.label(format!("Dimensions: {} rows × {} columns", rows, cols));
                                ui.add_space(5.0);

                                // Preview first few rows in a table
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

        // Try XLSX first
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

    fn render_pdf_preview(&self, ui: &mut egui::Ui, entry: &FileEntry) {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label(egui::RichText::new("📕 PDF Document").size(18.0));
            ui.add_space(10.0);

            match PdfDocument::load(&entry.path) {
                Ok(doc) => {
                    ui.label(format!("Pages: {}", doc.get_pages().len()));
                    ui.add_space(5.0);

                    // Try to extract basic metadata
                    let mut has_metadata = false;
                    if let Ok(info_ref) = doc.trailer.get(b"Info") {
                        if let Ok(info_id) = info_ref.as_reference() {
                            if let Ok(info_obj) = doc.get_object(info_id) {
                                if let Ok(info_dict) = info_obj.as_dict() {
                                    // Try to extract title
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
                                    // Try to extract author
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

    fn render_parent_pane(
        &self,
        ui: &mut egui::Ui,
        next_navigation: &std::cell::RefCell<Option<PathBuf>>,
    ) {
        ui.add_space(4.0);
        ui.vertical_centered(|ui| {
            ui.heading("Parent");
        });
        ui.separator();
        let accent = egui::Color32::from_rgb(120, 180, 255);
        let default_color = ui.visuals().text_color();

        egui::ScrollArea::vertical()
            .id_salt("parent_scroll")
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
                    .body(|body| {
                        body.rows(24.0, self.parent_entries.len(), |mut row| {
                            let entry = &self.parent_entries[row.index()];
                            let is_active = entry.path == self.current_path;

                            let icon_color = if is_active { accent } else { default_color };

                            row.col(|ui| {
                                ui.label(
                                    egui::RichText::new(entry.get_icon())
                                        .size(14.0)
                                        .color(icon_color),
                                );
                            });
                            row.col(|ui| {
                                let text_color = if is_active { accent } else { default_color };
                                let response = layout::truncated_label_with_sense(
                                    ui,
                                    egui::RichText::new(entry.display_name()).color(text_color),
                                    egui::Sense::click(),
                                );
                                if response.clicked() {
                                    // Navigate to the clicked directory in the parent pane
                                    *next_navigation.borrow_mut() = Some(entry.path.clone());
                                }
                            });
                        });
                    });
            });
    }

    fn render_current_pane(
        &mut self,
        ui: &mut egui::Ui,
        next_navigation: &std::cell::RefCell<Option<PathBuf>>,
        next_selection: &std::cell::RefCell<Option<usize>>,
        context_action: &std::cell::RefCell<Option<Box<dyn FnOnce(&mut Self)>>>,
        ctx: &egui::Context,
    ) {
        // Detect manual scrolling in the central panel
        if ui.ui_contains_pointer()
            && ctx.input(|i| {
                i.smooth_scroll_delta != egui::Vec2::ZERO || i.raw_scroll_delta != egui::Vec2::ZERO
            })
        {
            self.disable_autoscroll = true;
        }

        egui::ScrollArea::vertical()
            .id_salt("current_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                use egui_extras::{Column, TableBuilder};
                let mut table = TableBuilder::new(ui)
                    .striped(true)
                    .resizable(false)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::initial(30.0))
                    .column(Column::remainder().clip(true));

                // Only scroll to selected row if autoscroll is not disabled
                if !self.disable_autoscroll {
                    if let Some(idx) = self.selected_index {
                        table = table.scroll_to_row(idx, None);
                    }
                }

                table
                    .header(20.0, |mut header| {
                        header.col(|ui| {
                            ui.label("");
                        });
                        header.col(|ui| {
                            ui.label("Name");
                        });
                    })
                    .body(|body| {
                        body.rows(24.0, self.visible_entries.len(), |mut row| {
                            let row_index = row.index();
                            let entry = &self.visible_entries[row_index];
                            let is_focused = self.selected_index == Some(row_index);
                            let is_multi_selected = self.multi_selection.contains(&entry.path);
                            let is_cut = self.clipboard_op == Some(ClipboardOp::Cut)
                                && self.clipboard.contains(&entry.path);

                            if is_multi_selected || is_focused {
                                row.set_selected(true);
                            }

                            // Icon column
                            row.col(|ui| {
                                ui.label(egui::RichText::new(entry.get_icon()).size(14.0));
                            });

                            // Name column with context menu
                            row.col(|ui| {
                                let mut text = egui::RichText::new(entry.display_name());
                                if is_multi_selected {
                                    text = text.color(egui::Color32::LIGHT_BLUE);
                                } else if is_cut {
                                    text = text.color(egui::Color32::from_white_alpha(100));
                                // Dimmed
                                } else if entry.is_dir {
                                    text = text.color(egui::Color32::from_rgb(120, 180, 255));
                                // Subtle blue for directories
                                } else {
                                    // Keep default text color for files
                                }

                                let response = layout::truncated_label_with_sense(
                                    ui,
                                    text,
                                    egui::Sense::click(),
                                );

                                // Single click for selection only
                                if response.clicked() {
                                    *next_selection.borrow_mut() = Some(row_index);
                                }

                                // Double click to open/navigate
                                if response.double_clicked() {
                                    if let Some(entry) = self.visible_entries.get(row_index) {
                                        *next_navigation.borrow_mut() = Some(entry.path.clone());
                                    }
                                }

                                // Context menu on right-click
                                let entry_clone = entry.clone();
                                response.context_menu(|ui| {
                                    if ui.button("📂 Open").clicked() {
                                        if entry_clone.is_dir {
                                            *next_navigation.borrow_mut() =
                                                Some(entry_clone.path.clone());
                                        } else {
                                            let _ = open::that(&entry_clone.path);
                                        }
                                        ui.close();
                                    }

                                    ui.separator();

                                    if ui.button("📋 Copy (y)").clicked() {
                                        let path = entry_clone.path.clone();
                                        *context_action.borrow_mut() =
                                            Some(Box::new(move |app: &mut Self| {
                                                app.clipboard.clear();
                                                app.clipboard.insert(path);
                                                app.clipboard_op = Some(ClipboardOp::Copy);
                                                app.info_message =
                                                    Some(("Copied 1 file".into(), Instant::now()));
                                            }));
                                        ui.close();
                                    }

                                    if ui.button("✂️ Cut (x)").clicked() {
                                        let path = entry_clone.path.clone();
                                        *context_action.borrow_mut() =
                                            Some(Box::new(move |app: &mut Self| {
                                                app.clipboard.clear();
                                                app.clipboard.insert(path);
                                                app.clipboard_op = Some(ClipboardOp::Cut);
                                                app.info_message =
                                                    Some(("Cut 1 file".into(), Instant::now()));
                                            }));
                                        ui.close();
                                    }

                                    if ui.button("📥 Paste (p)").clicked() {
                                        *context_action.borrow_mut() =
                                            Some(Box::new(|app: &mut Self| {
                                                app.paste_clipboard();
                                            }));
                                        ui.close();
                                    }

                                    ui.separator();

                                    if ui.button("✏️ Rename (r)").clicked() {
                                        *next_selection.borrow_mut() = Some(row_index);
                                        let name = entry_clone.name.clone();
                                        *context_action.borrow_mut() =
                                            Some(Box::new(move |app: &mut Self| {
                                                app.command_buffer = name;
                                                app.mode = AppMode::Rename;
                                                app.focus_input = true;
                                            }));
                                        ui.close();
                                    }

                                    if ui.button("🗑️ Delete (d)").clicked() {
                                        *next_selection.borrow_mut() = Some(row_index);
                                        *context_action.borrow_mut() =
                                            Some(Box::new(|app: &mut Self| {
                                                app.mode = AppMode::DeleteConfirm;
                                            }));
                                        ui.close();
                                    }

                                    ui.separator();

                                    if ui.button("ℹ️ Properties").clicked() {
                                        let size = entry_clone.size;
                                        let modified = entry_clone.modified;
                                        let is_dir = entry_clone.is_dir;
                                        *context_action.borrow_mut() =
                                            Some(Box::new(move |app: &mut Self| {
                                                app.info_message =
                                                    Some((
                                                        format!(
                                            "{} | {} | Modified: {}",
                                            if is_dir { "Directory" } else { "File" },
                                            bytesize::ByteSize(size),
                                            chrono::DateTime::<chrono::Local>::from(modified)
                                                .format("%Y-%m-%d %H:%M")
                                        ),
                                                        Instant::now(),
                                                    ));
                                            }));
                                        ui.close();
                                    }
                                });
                            });
                        });
                    });
            });
    }

    fn render_divider(&mut self, ui: &mut egui::Ui, index: usize) {
        let response = ui.allocate_response(ui.available_size(), egui::Sense::drag());

        let color = if response.hovered() || response.dragged() {
            ui.visuals().widgets.active.bg_fill
        } else {
            egui::Color32::from_gray(60)
        };
        ui.painter().rect_filled(response.rect, 0.0, color);

        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }

        if response.dragged() {
            let delta = response.drag_delta().x;
            match index {
                0 => {
                    self.panel_widths[0] =
                        (self.panel_widths[0] + delta).clamp(layout::PARENT_MIN, layout::PARENT_MAX)
                }
                1 => {
                    self.panel_widths[1] = (self.panel_widths[1] - delta)
                        .clamp(layout::PREVIEW_MIN, layout::PREVIEW_MAX)
                }
                _ => {}
            }
        }
    }

    fn render_preview(
        &self,
        ui: &mut egui::Ui,
        next_navigation: &std::cell::RefCell<Option<PathBuf>>,
        pending_selection: &std::cell::RefCell<Option<PathBuf>>,
    ) {
        let idx = match self.selected_index {
            Some(i) => i,
            None => {
                ui.centered_and_justified(|ui| {
                    ui.label("No file selected");
                });
                return;
            }
        };
        let entry = match self.visible_entries.get(idx) {
            Some(e) => e,
            None => return,
        };

        layout::truncated_label(
            ui,
            egui::RichText::new(format!("{} {}", entry.get_icon(), entry.display_name())).heading(),
        );
        ui.add_space(5.0);
        layout::truncated_label(
            ui,
            format!("Size: {}", bytesize::ByteSize(entry.size)),
        );
        let datetime: DateTime<Local> = entry.modified.into();
        ui.label(format!("Modified: {}", datetime.format("%Y-%m-%d %H:%M")));
        ui.separator();

        if entry.is_dir {
            // Show directory contents in preview pane
            if self.last_selection_change.elapsed() <= Duration::from_millis(200) {
                ui.centered_and_justified(|ui| {
                    ui.spinner();
                });
                return;
            }

            match read_directory(&entry.path, self.show_hidden) {
                Ok(entries) => {
                    let accent = egui::Color32::from_rgb(120, 180, 255);
                    let highlighted_index = self.directory_selections.get(&entry.path).copied();

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
                                            let response = layout::truncated_label_with_sense(
                                                ui,
                                                egui::RichText::new(preview_entry.display_name())
                                                    .color(text_color),
                                                egui::Sense::click(),
                                            );
                                            if response.clicked() {
                                                // Navigate to the directory being previewed (the currently selected item)
                                                // and set the clicked item to be selected after navigation
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
        if self.last_selection_change.elapsed() <= Duration::from_millis(200) {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        }

        // Image preview
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

        // Markdown preview
        if matches!(entry.extension.as_str(), "md" | "markdown") {
            self.render_markdown_preview(ui, entry);
            return;
        }

        // Archive preview
        if matches!(
            entry.extension.as_str(),
            "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz"
        ) {
            self.render_archive_preview(ui, entry);
            return;
        }

        // Audio metadata preview
        if matches!(
            entry.extension.as_str(),
            "mp3" | "flac" | "ogg" | "m4a" | "wav"
        ) {
            self.render_audio_metadata(ui, entry);
            return;
        }

        // PDF preview
        if matches!(entry.extension.as_str(), "pdf") {
            self.render_pdf_preview(ui, entry);
            return;
        }

        // Word document preview
        if matches!(entry.extension.as_str(), "docx" | "doc") {
            self.render_docx_preview(ui, entry);
            return;
        }

        // Excel spreadsheet preview
        if matches!(entry.extension.as_str(), "xlsx" | "xls") {
            self.render_xlsx_preview(ui, entry);
            return;
        }

        // Code/text files with syntax highlighting
        let text_extensions = [
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

        let check_as_text = text_extensions.contains(&entry.extension.as_str())
            || entry.extension.is_empty()
            || entry.name.starts_with('.'); // Hidden config files often have no extension

        if check_as_text {
            if entry.size > layout::MAX_PREVIEW_SIZE {
                self.render_large_file_message(ui, entry);
                return;
            }

            if !is_likely_binary(&entry.path) {
                self.render_syntax_highlighted(ui, entry);
                return;
            }
        }

        // Binary file - show info instead of auto-loading hex
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
}

impl eframe::App for Heike {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply theme
        match self.theme {
            Theme::Light => ctx.set_visuals(egui::Visuals::light()),
            Theme::Dark => ctx.set_visuals(egui::Visuals::dark()),
        }

        // Auto-dismiss old messages
        if let Some((_, time)) = &self.error_message {
            if time.elapsed() > Duration::from_secs(layout::MESSAGE_TIMEOUT_SECS) {
                self.error_message = None;
            }
        }
        if let Some((_, time)) = &self.info_message {
            if time.elapsed() > Duration::from_secs(layout::MESSAGE_TIMEOUT_SECS) {
                self.info_message = None;
            }
        }

        self.setup_watcher(ctx);
        self.process_watcher_events();
        self.process_async_results();
        self.handle_input(ctx);

        // Handle files dropped from external sources
        ctx.input(|i| {
            if !i.raw.dropped_files.is_empty() {
                self.handle_dropped_files(&i.raw.dropped_files);
            }
        });

        if self.mode == AppMode::Filtering {
            let old_len = self.visible_entries.len();
            self.apply_filter();
            if self.visible_entries.len() != old_len {
                self.last_selection_change = Instant::now();
            }
        }

        let next_navigation = std::cell::RefCell::new(None);
        let next_selection = std::cell::RefCell::new(None);
        let pending_selection = std::cell::RefCell::new(None);
        let context_action = std::cell::RefCell::new(None::<Box<dyn FnOnce(&mut Self)>>);

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                // History Controls (fixed)
                if ui.button("⬅").on_hover_text("Back (Alt+Left)").clicked() {
                    self.navigate_back();
                }
                if ui
                    .button("➡")
                    .on_hover_text("Forward (Alt+Right)")
                    .clicked()
                {
                    self.navigate_forward();
                }
                if ui.button("⬆").on_hover_text("Up (Backspace)").clicked() {
                    self.navigate_up();
                }
                ui.add_space(10.0);

                // Breadcrumbs (scrollable) - reserve space for right controls
                let breadcrumb_width = ui.available_width() - 180.0;
                egui::ScrollArea::horizontal()
                    .id_salt("breadcrumbs")
                    .max_width(breadcrumb_width)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let components: Vec<_> = self.current_path.components().collect();
                            let mut path_acc = PathBuf::new();
                            for component in components {
                                path_acc.push(component);
                                let name = component.as_os_str().to_string_lossy();
                                let label = if name.is_empty() { "/" } else { &name };
                                if ui.button(label).clicked() {
                                    *next_navigation.borrow_mut() = Some(path_acc.clone());
                                }
                                ui.label(">");
                            }
                        });
                    });

                // Right controls in remaining space
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.checkbox(&mut self.show_hidden, "Hidden (.)").changed() {
                        self.request_refresh();
                    }

                    // Theme toggle
                    let theme_icon = match self.theme {
                        Theme::Light => "🌙",
                        Theme::Dark => "☀",
                    };
                    if ui
                        .button(theme_icon)
                        .on_hover_text("Toggle theme")
                        .clicked()
                    {
                        self.theme = match self.theme {
                            Theme::Light => Theme::Dark,
                            Theme::Dark => Theme::Light,
                        };
                    }

                    if ui.button("?").clicked() {
                        self.mode = AppMode::Help;
                    }

                    // Mode Indicator
                    match &self.mode {
                        AppMode::Normal => {
                            ui.label("NORMAL");
                        }
                        AppMode::Visual => {
                            ui.colored_label(egui::Color32::LIGHT_BLUE, "VISUAL");
                        }
                        AppMode::Filtering => {
                            ui.colored_label(egui::Color32::YELLOW, "FILTER");
                        }
                        AppMode::Command => {
                            ui.colored_label(egui::Color32::RED, "COMMAND");
                        }
                        AppMode::Help => {
                            ui.colored_label(egui::Color32::GREEN, "HELP");
                        }
                        AppMode::Rename => {
                            ui.colored_label(egui::Color32::ORANGE, "RENAME");
                        }
                        AppMode::DeleteConfirm => {
                            ui.colored_label(egui::Color32::RED, "CONFIRM DELETE?");
                        }
                        AppMode::SearchInput => {
                            ui.colored_label(egui::Color32::LIGHT_BLUE, "SEARCH");
                        }
                        AppMode::SearchResults { results, .. } => {
                            ui.colored_label(
                                egui::Color32::LIGHT_BLUE,
                                format!("SEARCH ({} results)", results.len()),
                            );
                        }
                    }
                });
            });
            ui.add_space(4.0);
        });

        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Item counts
                ui.label(format!(
                    "{}/{} items",
                    self.visible_entries.len(),
                    self.all_entries.len()
                ));
                
                // Show current selected file info
                if let Some(idx) = self.selected_index {
                    if let Some(entry) = self.visible_entries.get(idx) {
                        ui.separator();
                        let type_str = if entry.is_dir { "dir" } else { "file" };
                        ui.label(format!(
                            "{}: {}",
                            type_str,
                            bytesize::ByteSize(entry.size)
                        ));
                    }
                }
                
                // Show current path
                ui.separator();
                layout::truncated_label(ui, format!("{}", self.current_path.display()));

                if self.is_loading {
                    ui.spinner();
                }

                if let Some((msg, _)) = &self.info_message {
                    ui.colored_label(egui::Color32::GREEN, msg);
                }
                if let Some((err, _)) = &self.error_message {
                    ui.colored_label(egui::Color32::RED, format!(" | {}", err));
                }

                if !self.multi_selection.is_empty() {
                    ui.separator();
                    // Calculate total size of selected files
                    let total_size: u64 = self.all_entries.iter()
                        .filter(|e| self.multi_selection.contains(&e.path))
                        .map(|e| e.size)
                        .sum();
                    ui.colored_label(
                        egui::Color32::LIGHT_BLUE,
                        format!("{} selected ({})", 
                            self.multi_selection.len(),
                            bytesize::ByteSize(total_size)
                        ),
                    );
                }
            });
        });

        // Search Results View
        if let AppMode::SearchResults {
            ref query,
            ref results,
            selected_index,
        } = self.mode
        {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.heading(format!("Search Results: \"{}\"", query));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!("{} matches", results.len()));
                    });
                });
                ui.separator();
                ui.add_space(4.0);

                ui.columns(2, |columns| {
                    // Left column: Results list
                    columns[0].vertical(|ui| {
                        ui.heading("Matches");
                        ui.separator();
                        egui::ScrollArea::vertical()
                            .id_salt("search_results_scroll")
                            .auto_shrink([false, false])
                            .max_height(ui.available_height())
                            .show(ui, |ui| {
                                ui.set_max_width(ui.available_width());
                                use egui_extras::{Column, TableBuilder};
                                let mut table = TableBuilder::new(ui)
                                    .striped(true)
                                    .resizable(false)
                                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                                    .column(Column::remainder().clip(true));

                                if !results.is_empty() && selected_index < results.len() {
                                    table = table
                                        .scroll_to_row(selected_index, Some(egui::Align::Center));
                                }

                                table.body(|body| {
                                    body.rows(40.0, results.len(), |mut row| {
                                        let row_index = row.index();
                                        let result = &results[row_index];
                                        let is_selected = selected_index == row_index;

                                        if is_selected {
                                            row.set_selected(true);
                                        }

                                        row.col(|ui| {
                                            ui.vertical(|ui| {
                                                let file_label = format!(
                                                    "{}:{}",
                                                    result.file_name, result.line_number
                                                );
                                                let text = if is_selected {
                                                    egui::RichText::new(&file_label).color(
                                                        egui::Color32::from_rgb(100, 200, 255),
                                                    )
                                                } else {
                                                    egui::RichText::new(&file_label)
                                                };
                                                ui.label(text);

                                                // Show line content preview (truncated)
                                                let preview = if result.line_content.len() > 60 {
                                                    format!("{}...", &result.line_content[..60])
                                                } else {
                                                    result.line_content.clone()
                                                };
                                                ui.label(
                                                    egui::RichText::new(preview)
                                                        .size(10.0)
                                                        .color(egui::Color32::GRAY),
                                                );
                                            });
                                        });
                                    });
                                });
                            });
                    });

                    // Right column: Preview
                    columns[1].vertical(|ui| {
                        ui.heading("Preview");
                        ui.separator();

                        if let Some(result) = results.get(selected_index) {
                            ui.label(egui::RichText::new(&result.file_name).strong());
                            ui.separator();

                            // Show context around the match
                            egui::ScrollArea::vertical()
                                .id_salt("search_preview_scroll")
                                .auto_shrink([false, false])
                                .max_height(ui.available_height())
                                .show(ui, |ui| {
                                    ui.set_max_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.label(format!("Line {}:", result.line_number));
                                        ui.label(egui::RichText::new(&result.line_content).code());
                                    });

                                    ui.add_space(10.0);
                                    ui.label("Full file path:");
                                    ui.label(
                                        egui::RichText::new(result.file_path.display().to_string())
                                            .code(),
                                    );

                                    ui.add_space(10.0);
                                    ui.horizontal(|ui| {
                                        ui.label("Press");
                                        ui.label(egui::RichText::new("Enter").strong());
                                        ui.label("to open file,");
                                        ui.label(egui::RichText::new("n/N").strong());
                                        ui.label("for next/previous,");
                                        ui.label(egui::RichText::new("Esc").strong());
                                        ui.label("to return");
                                    });
                                });
                        }
                    });
                });
            });
        } else {
            // Normal file browser view
            // Visual feedback for drag and drop
            let is_being_dragged_over = ctx.input(|i| !i.raw.hovered_files.is_empty());

            egui::CentralPanel::default().show(ctx, |ui| {
                // Show drop zone overlay when files are being dragged over
                if is_being_dragged_over {
                    let painter = ui.painter();
                    let rect = ui.available_rect_before_wrap();
                    painter.rect_stroke(
                        rect,
                        5.0,
                        egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 200, 255)),
                        egui::epaint::StrokeKind::Outside,
                    );
                    ui.label(
                        egui::RichText::new("📁 Drop files here to copy them to this directory")
                            .size(16.0)
                            .color(egui::Color32::from_rgb(100, 200, 255)),
                    );
                }
                // Help Modal
                if self.mode == AppMode::Help {
                    egui::Window::new("Help")
                        .collapsible(false)
                        .resizable(false)
                        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                        .default_width(layout::modal_width(ctx))
                        .show(ctx, |ui| {
                            ui.set_max_height(layout::modal_max_height(ctx));
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                ui.heading("Key Bindings");
                                ui.separator();
                                egui::Grid::new("help_grid").striped(true).show(ui, |ui| {
                                    ui.label("j / Down");
                                    ui.label("Next Item");
                                    ui.end_row();
                                    ui.label("k / Up");
                                    ui.label("Previous Item");
                                    ui.end_row();
                                    ui.label("h / Left Arrow / Backspace");
                                    ui.label("Go to Parent");
                                    ui.end_row();
                                    ui.label("l / Right Arrow");
                                    ui.label("Enter Directory");
                                    ui.end_row();
                                    ui.label("Enter");
                                    ui.label("Open File / Enter Dir");
                                    ui.end_row();
                                    ui.label("gg / G");
                                    ui.label("Top / Bottom");
                                    ui.end_row();
                                    ui.label("Alt + Arrows");
                                    ui.label("History");
                                    ui.end_row();
                                    ui.label(".");
                                    ui.label("Toggle Hidden");
                                    ui.end_row();
                                    ui.label("/");
                                    ui.label("Filter Mode");
                                    ui.end_row();
                                    ui.label("S (Shift+s)");
                                    ui.label("Content Search");
                                    ui.end_row();
                                    ui.label(":");
                                    ui.label("Command Mode");
                                    ui.end_row();
                                    ui.label("v");
                                    ui.label("Visual Select Mode");
                                    ui.end_row();
                                    ui.label("y / x / p");
                                    ui.label("Copy / Cut / Paste");
                                    ui.end_row();
                                    ui.label("d / r");
                                    ui.label("Delete / Rename");
                                    ui.end_row();
                                    ui.label("?");
                                    ui.label("Toggle Help");
                                    ui.end_row();
                                    ui.label("Shift+V");
                                    ui.label("Visual Mode (Select All)");
                                    ui.end_row();
                                    ui.label("Ctrl+A");
                                    ui.label("Select All Items");
                                    ui.end_row();
                                    ui.label("Space");
                                    ui.label("Toggle Selection");
                                    ui.end_row();
                                });
                                ui.add_space(10.0);
                                if ui.button("Close").clicked() {
                                    self.mode = AppMode::Normal;
                                }
                            });
                        });
                }

                // Search Input Modal
                if self.mode == AppMode::SearchInput {
                    egui::Window::new("Content Search")
                        .collapsible(false)
                        .resizable(false)
                        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                        .default_width(layout::modal_width(ctx))
                        .show(ctx, |ui| {
                            ui.set_max_height(layout::modal_max_height(ctx));
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                ui.label("Search for content in files:");
                                ui.add_space(5.0);

                                let response = ui.text_edit_singleline(&mut self.search_query);
                                if self.focus_input {
                                    response.request_focus();
                                    self.focus_input = false;
                                }

                                ui.add_space(10.0);
                                ui.label("Options:");
                                ui.checkbox(
                                    &mut self.search_options.case_sensitive,
                                    "Case sensitive",
                                );
                                ui.checkbox(&mut self.search_options.use_regex, "Use regex");
                                ui.checkbox(
                                    &mut self.search_options.search_hidden,
                                    "Search hidden files",
                                );
                                ui.checkbox(&mut self.search_options.search_pdfs, "Search PDFs");
                                ui.checkbox(
                                    &mut self.search_options.search_archives,
                                    "Search archives",
                                );

                                ui.add_space(10.0);
                                ui.horizontal(|ui| {
                                    if ui.button("Search").clicked()
                                        && !self.search_query.is_empty()
                                    {
                                        self.search_in_progress = true;
                                        self.search_file_count = 0;
                                        let _ = self.command_tx.send(IoCommand::SearchContent {
                                            query: self.search_query.clone(),
                                            root_path: self.current_path.clone(),
                                            options: self.search_options.clone(),
                                        });
                                        self.mode = AppMode::Normal;
                                    }
                                    if ui.button("Cancel").clicked() {
                                        self.mode = AppMode::Normal;
                                    }
                                });

                                if self.search_in_progress {
                                    ui.add_space(10.0);
                                    ui.horizontal(|ui| {
                                        ui.spinner();
                                        ui.label(format!(
                                            "Searching... ({} files)",
                                            self.search_file_count
                                        ));
                                    });
                                }
                            });
                        });
                }

                // Command/Filter/Rename Input Modal
                if matches!(
                    self.mode,
                    AppMode::Command | AppMode::Filtering | AppMode::Rename
                ) {
                    egui::Area::new("input_popup".into())
                        .anchor(egui::Align2::CENTER_TOP, [0.0, 50.0])
                        .order(egui::Order::Foreground)
                        .show(ctx, |ui| {
                            egui::Frame::popup(ui.style()).show(ui, |ui| {
                                ui.set_min_width(400.0);
                                let prefix = match self.mode {
                                    AppMode::Rename => "Rename:",
                                    AppMode::Filtering => "/",
                                    _ => ":",
                                };
                                ui.horizontal(|ui| {
                                    ui.label(prefix);
                                    let response =
                                        ui.text_edit_singleline(&mut self.command_buffer);
                                    if self.focus_input {
                                        response.request_focus();
                                        self.focus_input = false;
                                    }
                                });
                            });
                        });
                }

                // Strip-based layout with three panes and dividers
                use egui_extras::{Size, StripBuilder};
                StripBuilder::new(ui)
                    .size(Size::exact(self.panel_widths[0]).at_least(layout::PARENT_MIN))
                    .size(Size::exact(layout::DIVIDER_WIDTH))
                    .size(Size::remainder())
                    .size(Size::exact(layout::DIVIDER_WIDTH))
                    .size(Size::exact(self.panel_widths[1]).at_least(layout::PREVIEW_MIN))
                    .horizontal(|mut strip| {
                        strip.cell(|ui| self.render_parent_pane(ui, &next_navigation));
                        strip.cell(|ui| self.render_divider(ui, 0));
                        strip.cell(|ui| {
                            self.render_current_pane(
                                ui,
                                &next_navigation,
                                &next_selection,
                                &context_action,
                                ctx,
                            )
                        });
                        strip.cell(|ui| self.render_divider(ui, 1));
                        strip.cell(|ui| {
                            ui.add_space(4.0);
                            ui.vertical_centered(|ui| {
                                ui.heading("Preview");
                            });
                            ui.separator();
                            self.render_preview(ui, &next_navigation, &pending_selection);
                        });
                    });
            });
        } // End of else block for normal file browser view

        if let Some(idx) = next_selection.into_inner() {
            self.selected_index = Some(idx);
        }
        if let Some(pending) = pending_selection.into_inner() {
            self.pending_selection_path = Some(pending);
        }
        if let Some(path) = next_navigation.into_inner() {
            self.navigate_to(path);
        }
        if let Some(action) = context_action.into_inner() {
            action(self);
        }
    }
}

fn main() -> eframe::Result<()> {
    // Load the app icon
    let icon_bytes = include_bytes!("../assets/heike_icon.png");
    let icon_image = image::load_from_memory(icon_bytes)
        .expect("Failed to load icon")
        .to_rgba8();
    let (icon_width, icon_height) = icon_image.dimensions();
    let icon_data = egui::IconData {
        rgba: icon_image.into_raw(),
        width: icon_width,
        height: icon_height,
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 700.0])
            .with_title("Heike")
            .with_icon(icon_data)
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        "Heike",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);

            // Configure fonts to use bundled Nerd Font for icon rendering
            let mut fonts = egui::FontDefinitions::default();

            // Use bundled JetBrainsMono Nerd Font
            let nerd_font_data = include_bytes!("../assets/JetBrainsMonoNerdFont-Regular.ttf");
            fonts.font_data.insert(
                "nerd_font".to_owned(),
                egui::FontData::from_static(nerd_font_data).into(),
            );

            // Add to proportional and monospace families (prioritize Nerd Font)
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "nerd_font".to_owned());

            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .insert(0, "nerd_font".to_owned());

            cc.egui_ctx.set_fonts(fonts);

            Ok(Box::new(Heike::new(cc.egui_ctx.clone())))
        }),
    )
}
