use crate::config::BookmarksConfig;
use crate::entry::FileEntry;
use crate::io::{fuzzy_match, is_likely_binary, read_directory, spawn_worker, IoCommand, IoResult};
use crate::state::{
    AppMode, ClipboardOp, SearchOptions, SortOptions, NavigationState, SelectionState,
    EntryState, UIState, ModeState,
};
use crate::style::{self, Theme};
use crate::view;

use chrono::{DateTime, Local};
use eframe::egui;
use notify::{Event, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

pub struct Heike {
    // Logical state groupings
    pub navigation: NavigationState,
    pub selection: SelectionState,
    pub entries: EntryState,
    pub ui: UIState,
    pub mode: ModeState,

    // Clipboard operations
    pub clipboard: HashSet<PathBuf>,
    pub clipboard_op: Option<ClipboardOp>,

    // Async I/O channels
    pub command_tx: Sender<IoCommand>,
    pub result_rx: Receiver<IoResult>,
    pub watcher: Option<Box<dyn Watcher>>,
    pub watcher_rx: Receiver<Result<Event, notify::Error>>,
    pub watched_path: Option<PathBuf>,

    // Resources
    pub syntax_set: SyntaxSet,
    pub theme_set: ThemeSet,
    pub bookmarks: BookmarksConfig,
}
impl Heike {
    pub fn new(ctx: egui::Context, config: crate::config::Config, cli_start_dir: Option<PathBuf>) -> Self {
        let start_path = if let Some(dir) = cli_start_dir {
            // Use CLI-provided directory if valid
            if dir.is_dir() {
                dir
            } else {
                // Fall back to home dir if CLI path doesn't exist
                directories::UserDirs::new()
                    .map(|ud| ud.home_dir().to_path_buf())
                    .unwrap_or_else(|| env::current_dir().unwrap_or_default())
            }
        } else {
            // Use default logic if no CLI arg provided
            directories::UserDirs::new()
                .map(|ud| ud.home_dir().to_path_buf())
                .unwrap_or_else(|| env::current_dir().unwrap_or_default())
        };

        let (cmd_tx, res_rx) = spawn_worker(ctx.clone());
        let (_watch_tx, watch_rx) = channel();

        // Parse theme from config
        let theme = match config.theme.mode.as_str() {
            "light" => Theme::Light,
            _ => Theme::Dark,
        };

        // Parse sort options from config
        let sort_by = match config.ui.sort_by.as_str() {
            "size" => crate::state::SortBy::Size,
            "modified" => crate::state::SortBy::Modified,
            "extension" => crate::state::SortBy::Extension,
            _ => crate::state::SortBy::Name,
        };

        let sort_order = match config.ui.sort_order.as_str() {
            "desc" => crate::state::SortOrder::Descending,
            _ => crate::state::SortOrder::Ascending,
        };

        let sort_options = crate::state::SortOptions {
            sort_by,
            sort_order,
            dirs_first: config.ui.dirs_first,
        };

        let mut ui_state = UIState::new(theme.clone(), sort_options);
        ui_state.show_hidden = config.ui.show_hidden;
        ui_state.panel_widths = [config.panel.parent_width, config.panel.preview_width];

        let mut app = Self {
            navigation: NavigationState::new(start_path.clone()),
            selection: SelectionState::new(),
            entries: EntryState::new(),
            ui: ui_state,
            mode: ModeState::new(),
            clipboard: HashSet::new(),
            clipboard_op: None,
            command_tx: cmd_tx,
            result_rx: res_rx,
            watcher: None,
            watcher_rx: watch_rx,
            watched_path: None,
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            bookmarks: config.bookmarks.clone(),
        };

        app.request_refresh();
        app
    }

    pub(crate) fn request_refresh(&mut self) {
        self.ui.is_loading = true;
        self.ui.error_message = None;
        // Keep info message if it's fresh, or maybe clear it? Let's keep it for feedback.
        let _ = self.command_tx.send(IoCommand::LoadDirectory(
            self.navigation.current_path.clone(),
            self.ui.show_hidden,
        ));
        if let Some(parent) = self.navigation.current_path.parent() {
            let _ = self.command_tx.send(IoCommand::LoadParent(
                parent.to_path_buf(),
                self.ui.show_hidden,
            ));
        } else {
            self.entries.parent_entries.clear();
        }
    }

    pub(crate) fn apply_filter(&mut self) {
        // Save currently selected item path before filtering
        let previously_selected = self
            .selected_index
            .and_then(|idx| self.entries.visible_entries.get(idx))
            .map(|e| e.path.clone());

        if self.mode.mode == AppMode::Filtering && !self.mode.command_buffer.is_empty() {
            let query = self.mode.command_buffer.clone();
            self.entries.visible_entries = self
                .all_entries
                .iter()
                .filter(|e| fuzzy_match(&e.name, &query))
                .cloned()
                .collect();
        } else {
            self.entries.visible_entries = self.entries.all_entries.clone();
        }

        // Apply sorting
        self.sort_visible_entries();

        // Restore selection to previously selected item if possible
        if let Some(path) = previously_selected {
            if let Some(idx) = self.entries.visible_entries.iter().position(|e| e.path == path) {
                self.selection.selected_index = Some(idx);
            }
            // If path not found, keep current selection if valid; otherwise default to 0
        }

        // Validate/fix selection if it's now out of bounds
        if self.entries.visible_entries.is_empty() {
            self.selection.selected_index = None;
        } else if let Some(idx) = self.selection.selected_index {
            if idx >= self.entries.visible_entries.len() {
                self.selection.selected_index = Some(self.entries.visible_entries.len() - 1);
            }
        } else if self.selection.selected_index.is_none() {
            self.selection.selected_index = Some(0);
        }
        self.validate_selection();
    }

    fn sort_visible_entries(&mut self) {
        use crate::state::{SortBy, SortOrder};

        // Separate directories and files if dirs_first is enabled
        let (mut dirs, mut files): (Vec<_>, Vec<_>) = self
            .visible_entries
            .drain(..)
            .partition(|e| e.is_dir);

        // Sort both groups by the selected criteria
        let sort_fn = |a: &FileEntry, b: &FileEntry| -> std::cmp::Ordering {
            let cmp = match self.ui.sort_options.sort_by {
                SortBy::Name => a.name.cmp(&b.name),
                SortBy::Size => a.size.cmp(&b.size),
                SortBy::Modified => a.modified.cmp(&b.modified),
                SortBy::Extension => a.extension.cmp(&b.extension),
            };

            match self.ui.sort_options.sort_order {
                SortOrder::Ascending => cmp,
                SortOrder::Descending => cmp.reverse(),
            }
        };

        dirs.sort_by(sort_fn);
        files.sort_by(sort_fn);

        // Combine back, with dirs first if enabled
        if self.ui.sort_options.dirs_first {
            self.entries.visible_entries.extend(dirs);
            self.entries.visible_entries.extend(files);
        } else {
            self.entries.visible_entries.extend(files);
            self.entries.visible_entries.extend(dirs);
        }
    }

    fn setup_watcher(&mut self, ctx: &egui::Context) {
        // Only setup if path changed
        if self.watched_path.as_ref() == Some(&self.navigation.current_path) {
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
                if let Err(e) = watcher.watch(&self.navigation.current_path, RecursiveMode::NonRecursive) {
                    self.ui.error_message =
                        Some((format!("Failed to watch directory: {}", e), Instant::now()));
                    self.watcher = None;
                    self.watched_path = None;
                } else {
                    self.watcher = Some(Box::new(watcher));
                    self.watched_path = Some(self.navigation.current_path.clone());
                }
            }
            Err(e) => {
                self.ui.error_message =
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
                    if path != self.navigation.current_path {
                        continue;
                    }

                    self.entries.all_entries = entries;
                    self.ui.is_loading = false;
                    self.apply_filter();

                    // If there's a pending selection path, find and select it
                    if let Some(pending_path) = self.navigation.pending_selection_path.take() {
                        if let Some(idx) = self
                            .visible_entries
                            .iter()
                            .position(|e| e.path == pending_path)
                        {
                            self.selection.selected_index = Some(idx);
                        }
                    }

                    // Validate selection after loading
                    if let Some(idx) = self.selection.selected_index {
                        if idx >= self.entries.visible_entries.len() && !self.entries.visible_entries.is_empty() {
                            self.selection.selected_index = Some(self.entries.visible_entries.len() - 1);
                        }
                    }
                }
                IoResult::ParentLoaded(entries) => {
                    self.entries.parent_entries = entries;
                }
                IoResult::SearchCompleted(results) => {
                    self.ui.search_in_progress = false;
                    let result_count = results.len();
                    self.mode.set_mode(AppMode::SearchResults {
                        query: self.ui.search_query.clone(),
                        results,
                        selected_index: 0,
                    });
                    self.ui.info_message = Some((
                        format!(
                            "Found {} matches in {} files",
                            result_count, self.ui.search_file_count
                        ),
                        Instant::now(),
                    ));
                }
                IoResult::SearchProgress(count) => {
                    self.ui.search_file_count = count;
                }
                IoResult::Error(msg) => {
                    self.ui.is_loading = false;
                    self.ui.search_in_progress = false;
                    self.ui.error_message = Some((msg, Instant::now()));
                    self.entries.all_entries.clear();
                    self.entries.visible_entries.clear();
                }
            }
        }
    }

    // --- Navigation Logic ---

    pub(crate) fn navigate_to(&mut self, path: PathBuf) {
        if path.is_dir() {
            // Save current selection before navigating away
            if let Some(idx) = self.selection.selected_index {
                self.selection.directory_selections
                    .insert(self.navigation.current_path.clone(), idx);
            }

            self.navigation.current_path = path.clone();

            if self.navigation.history_index < self.navigation.history.len() - 1 {
                self.navigation.history.truncate(self.navigation.history_index + 1);
            }
            self.navigation.history.push(path);
            self.navigation.history_index = self.navigation.history.len() - 1;

            self.finish_navigation();
        } else if let Err(e) = open::that(&path) {
            self.ui.error_message = Some((format!("Could not open file: {}", e), Instant::now()));
        }
    }

    pub(crate) fn navigate_up(&mut self) {
        if let Some(parent) = self.navigation.current_path.parent() {
            // Save current selection before navigating up
            if let Some(idx) = self.selection.selected_index {
                self.selection.directory_selections
                    .insert(self.navigation.current_path.clone(), idx);
            }
            // When navigating to parent, select the child directory we came from
            self.navigation.pending_selection_path = Some(self.navigation.current_path.clone());
            self.navigate_to(parent.to_path_buf());
        }
    }

    pub(crate) fn navigate_back(&mut self) {
        if self.navigation.history_index == 0 {
            return;
        }

        if let Some(idx) = self.selection.selected_index {
            self.selection.directory_selections
                .insert(self.navigation.current_path.clone(), idx);
        }

        let mut idx = self.navigation.history_index;
        while idx > 0 {
            idx -= 1;
            let target = self.navigation.history[idx].clone();
            if target.is_dir() {
                self.navigation.history_index = idx;
                self.navigation.current_path = target;
                self.finish_navigation();
                return;
            } else {
                self.navigation.history.remove(idx);
                self.navigation.history_index -= 1;
            }
        }

        self.ui.error_message = Some(("Previous directory no longer exists".into(), Instant::now()));
    }

    pub(crate) fn navigate_forward(&mut self) {
        if self.navigation.history_index >= self.navigation.history.len() - 1 {
            return;
        }

        if let Some(idx) = self.selection.selected_index {
            self.selection.directory_selections
                .insert(self.navigation.current_path.clone(), idx);
        }

        let idx = self.navigation.history_index + 1;
        loop {
            if idx >= self.navigation.history.len() {
                break;
            }
            let target = self.navigation.history[idx].clone();
            if target.is_dir() {
                self.navigation.history_index = idx;
                self.navigation.current_path = target;
                self.finish_navigation();
                return;
            }
            self.navigation.history.remove(idx);
        }

        self.ui.error_message = Some(("Next directory no longer exists".into(), Instant::now()));
    }

    fn finish_navigation(&mut self) {
        self.mode.command_buffer.clear();
        self.mode.set_mode(AppMode::Normal);
        self.selection.multi_selection.clear();
        // Restore saved selection for this directory, or default to 0
        self.selection.selected_index = self
            .directory_selections
            .get(&self.navigation.current_path)
            .copied()
            .or(Some(0));
        // Re-enable autoscroll when navigating to ensure view centers on selection
        self.selection.disable_autoscroll = false;
        self.request_refresh();
    }

    // --- File Operations (Injected) ---

    pub(crate) fn yank_selection(&mut self, op: ClipboardOp) {
        self.clipboard.clear();
        self.clipboard_op = Some(op);

        if !self.selection.multi_selection.is_empty() {
            self.clipboard = self.selection.multi_selection.clone();
            self.mode.set_mode(AppMode::Normal);
            self.selection.multi_selection.clear();
        } else if let Some(idx) = self.selection.selected_index {
            if let Some(entry) = self.entries.visible_entries.get(idx) {
                self.clipboard.insert(entry.path.clone());
            }
        }

        let op_text = if self.clipboard_op == Some(ClipboardOp::Copy) {
            "Yanked"
        } else {
            "Cut"
        };
        self.ui.info_message = Some((
            format!("{} {} files", op_text, self.clipboard.len()),
            Instant::now(),
        ));
    }

    pub(crate) fn paste_clipboard(&mut self) {
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
                let dest = self.navigation.current_path.join(name);
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
            self.ui.error_message = Some((errors.join(" | "), Instant::now()));
        } else {
            self.ui.info_message = Some((format!("Processed {} files", count), Instant::now()));
        }

        if op == ClipboardOp::Cut {
            self.clipboard.clear();
            self.clipboard_op = None;
        }
        self.request_refresh();
    }

    pub(crate) fn perform_delete(&mut self) {
        let targets = if !self.selection.multi_selection.is_empty() {
            self.selection.multi_selection.clone()
        } else if let Some(idx) = self.selection.selected_index {
            if let Some(entry) = self.entries.visible_entries.get(idx) {
                HashSet::from([entry.path.clone()])
            } else {
                HashSet::new()
            }
        } else {
            HashSet::new()
        };

        let mut error_count = 0;
        for path in targets {
            match trash::delete(&path) {
                Ok(_) => {},
                Err(e) => {
                    error_count += 1;
                    eprintln!("Failed to move to trash: {}", e);
                }
            }
        }

        self.mode.set_mode(AppMode::Normal);
        self.selection.multi_selection.clear();
        self.request_refresh();

        if error_count > 0 {
            self.ui.error_message = Some((
                format!("Failed to delete {} item(s)", error_count),
                Instant::now(),
            ));
        } else {
            self.ui.info_message = Some(("Items moved to trash".into(), Instant::now()));
        }
    }

    pub(crate) fn perform_rename(&mut self) {
        if let Some(idx) = self.selection.selected_index {
            if let Some(entry) = self.entries.visible_entries.get(idx) {
                let new_name = self.mode.command_buffer.trim();
                if !new_name.is_empty() {
                    let new_path = entry.path.parent().unwrap().join(new_name);
                    if let Err(e) = fs::rename(&entry.path, &new_path) {
                        self.ui.error_message =
                            Some((format!("Rename failed: {}", e), Instant::now()));
                    } else {
                        self.ui.info_message = Some(("Renamed successfully".into(), Instant::now()));
                    }
                }
            }
        }
        self.mode.set_mode(AppMode::Normal);
        self.mode.command_buffer.clear();
        self.request_refresh();
    }

    // --- Selection Validation ---

    fn validate_selection(&mut self) {
        if let Some(idx) = self.selection.selected_index {
            if self.entries.visible_entries.is_empty() {
                self.selection.selected_index = None;
            } else if idx >= self.entries.visible_entries.len() {
                self.selection.selected_index = Some(self.entries.visible_entries.len() - 1);
            }
        }
    }

    // --- Drag and Drop Handling ---
    // (Currently handled in the eframe::App update method)

    fn render_preview(
        &self,
        ui: &mut egui::Ui,
        next_navigation: &std::cell::RefCell<Option<PathBuf>>,
        pending_selection: &std::cell::RefCell<Option<PathBuf>>,
    ) {
        let idx = match self.selection.selected_index {
            Some(i) => i,
            None => {
                ui.centered_and_justified(|ui| {
                    ui.label("No file selected");
                });
                return;
            }
        };
        let entry = match self.entries.visible_entries.get(idx) {
            Some(e) => e,
            None => return,
        };

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
            // Show directory contents in preview pane
            if self.selection.last_selection_change.elapsed() <= Duration::from_millis(200) {
                ui.centered_and_justified(|ui| {
                    ui.spinner();
                });
                return;
            }

            match read_directory(&entry.path, self.ui.show_hidden) {
                Ok(entries) => {
                    let accent = egui::Color32::from_rgb(120, 180, 255);
                    let highlighted_index = self.selection.directory_selections.get(&entry.path).copied();

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
        if self.selection.last_selection_change.elapsed() <= Duration::from_millis(200) {
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
            view::preview::render_markdown_preview(ui, entry);
            return;
        }

        // Archive preview
        if matches!(
            entry.extension.as_str(),
            "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz"
        ) {
            view::preview::render_archive_preview(ui, entry);
            return;
        }

        // Audio metadata preview
        if matches!(
            entry.extension.as_str(),
            "mp3" | "flac" | "ogg" | "m4a" | "wav"
        ) {
            view::preview::render_audio_metadata(ui, entry);
            return;
        }

        // PDF preview
        if matches!(entry.extension.as_str(), "pdf") {
            view::preview::render_pdf_preview(ui, entry);
            return;
        }

        // Word document preview
        if matches!(entry.extension.as_str(), "docx" | "doc") {
            view::preview::render_docx_preview(ui, entry);
            return;
        }

        // Excel spreadsheet preview
        if matches!(entry.extension.as_str(), "xlsx" | "xls") {
            view::preview::render_xlsx_preview(ui, entry);
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
            if entry.size > style::MAX_PREVIEW_SIZE {
                view::preview::render_large_file_message(ui, entry);
                return;
            }

            if !is_likely_binary(&entry.path) {
                view::preview::render_syntax_highlighted(ui, entry, &self.syntax_set, &self.theme_set, self.ui.theme);
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

    // --- Drag and Drop Handling ---


    // --- Rendering Methods ---


    // Note: execute_command method is missing but needed for handle_input to compile
    pub(crate) fn execute_command(&mut self, _ctx: &egui::Context) {
        let parts: Vec<&str> = self.mode.command_buffer.trim().split_whitespace().collect();
        if parts.is_empty() {
            self.mode.set_mode(AppMode::Normal);
            self.mode.command_buffer.clear();
            return;
        }

        match parts[0] {
            "q" | "quit" => {
                std::process::exit(0);
            }
            "mkdir" => {
                if parts.len() < 2 {
                    self.ui.error_message = Some(("Usage: mkdir <name>".into(), Instant::now()));
                } else {
                    let dir_name = parts[1..].join(" ");
                    let new_dir = self.navigation.current_path.join(&dir_name);
                    match fs::create_dir(&new_dir) {
                        Ok(_) => {
                            self.ui.info_message =
                                Some((format!("Created directory: {}", dir_name), Instant::now()));
                            self.request_refresh();
                        }
                        Err(e) => {
                            self.ui.error_message =
                                Some((format!("Failed to create directory: {}", e), Instant::now()));
                        }
                    }
                }
            }
            "touch" => {
                if parts.len() < 2 {
                    self.ui.error_message = Some(("Usage: touch <filename>".into(), Instant::now()));
                } else {
                    let file_name = parts[1..].join(" ");
                    let new_file = self.navigation.current_path.join(&file_name);
                    match fs::File::create(&new_file) {
                        Ok(_) => {
                            self.ui.info_message =
                                Some((format!("Created file: {}", file_name), Instant::now()));
                            self.request_refresh();
                        }
                        Err(e) => {
                            self.ui.error_message =
                                Some((format!("Failed to create file: {}", e), Instant::now()));
                        }
                    }
                }
            }
            _ => {
                self.ui.error_message =
                    Some((format!("Unknown command: {}", parts[0]), Instant::now()));
            }
        }

        self.mode.set_mode(AppMode::Normal);
        self.mode.command_buffer.clear();
    }
}

impl eframe::App for Heike {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply theme
        match self.ui.theme {
            Theme::Light => ctx.set_visuals(egui::Visuals::light()),
            Theme::Dark => ctx.set_visuals(egui::Visuals::dark()),
        }

        // Auto-dismiss old messages
        if let Some((_, time)) = &self.ui.error_message {
            if time.elapsed() > Duration::from_secs(style::MESSAGE_TIMEOUT_SECS) {
                self.ui.error_message = None;
            }
        }
        if let Some((_, time)) = &self.ui.info_message {
            if time.elapsed() > Duration::from_secs(style::MESSAGE_TIMEOUT_SECS) {
                self.ui.info_message = None;
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

        if self.mode.mode == AppMode::Filtering {
            let old_len = self.entries.visible_entries.len();
            self.apply_filter();
            if self.entries.visible_entries.len() != old_len {
                self.selection.last_selection_change = Instant::now();
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
                            let components: Vec<_> = self.navigation.current_path.components().collect();
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
                    if ui.checkbox(&mut self.ui.show_hidden, "Hidden (.)").changed() {
                        self.request_refresh();
                    }

                    // Theme toggle
                    let theme_icon = match self.ui.theme {
                        Theme::Light => "🌙",
                        Theme::Dark => "☀",
                    };
                    if ui
                        .button(theme_icon)
                        .on_hover_text("Toggle theme")
                        .clicked()
                    {
                        self.ui.theme = match self.ui.theme {
                            Theme::Light => Theme::Dark,
                            Theme::Dark => Theme::Light,
                        };
                    }

                    if ui.button("?").clicked() {
                        self.mode.set_mode(AppMode::Help);
                    }

                    // Mode Indicator
                    match &self.mode.mode {
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
                    self.entries.visible_entries.len(),
                    self.entries.all_entries.len()
                ));
                
                // Show current selected file info
                if let Some(idx) = self.selection.selected_index {
                    if let Some(entry) = self.entries.visible_entries.get(idx) {
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
                style::truncated_label(ui, format!("{}", self.navigation.current_path.display()));

                if self.ui.is_loading {
                    ui.spinner();
                }

                if let Some((msg, _)) = &self.ui.info_message {
                    ui.colored_label(egui::Color32::GREEN, msg);
                }
                if let Some((err, _)) = &self.ui.error_message {
                    ui.colored_label(egui::Color32::RED, format!(" | {}", err));
                }

                if !self.selection.multi_selection.is_empty() {
                    ui.separator();
                    // Calculate total size of selected files
                    let total_size: u64 = self.entries.all_entries.iter()
                        .filter(|e| self.selection.multi_selection.contains(&e.path))
                        .map(|e| e.size)
                        .sum();
                    ui.colored_label(
                        egui::Color32::LIGHT_BLUE,
                        format!("{} selected ({})", 
                            self.selection.multi_selection.len(),
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
            // Track click selection
            let next_result_selection = std::cell::RefCell::new(None);

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

                                // Match main view scroll behavior - use None instead of Center
                                if !results.is_empty() && selected_index < results.len() {
                                    table = table.scroll_to_row(selected_index, None);
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

                                                // Make the label clickable
                                                let label_response = style::truncated_label_with_sense(
                                                    ui,
                                                    text,
                                                    egui::Sense::click(),
                                                );

                                                if label_response.clicked() {
                                                    *next_result_selection.borrow_mut() = Some(row_index);
                                                }

                                                // Show line content preview (truncated safely at char boundaries)
                                                let preview = if result.line_content.chars().count() > 60 {
                                                    let truncated: String = result.line_content
                                                        .chars()
                                                        .take(60)
                                                        .collect();
                                                    format!("{}...", truncated)
                                                } else {
                                                    result.line_content.clone()
                                                };
                                                let preview_response = style::truncated_label_with_sense(
                                                    ui,
                                                    egui::RichText::new(preview)
                                                        .size(10.0)
                                                        .color(egui::Color32::GRAY),
                                                    egui::Sense::click(),
                                                );

                                                if preview_response.clicked() {
                                                    *next_result_selection.borrow_mut() = Some(row_index);
                                                }
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

            // Apply deferred selection from click
            if let Some(new_index) = next_result_selection.into_inner() {
                if let AppMode::SearchResults {
                    ref query,
                    ref results,
                    selected_index: _,
                } = self.mode
                {
                    self.mode.set_mode(AppMode::SearchResults {
                        query: query.clone(),
                        results: results.clone(),
                        selected_index: new_index,
                    });
                }
            }
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
                // Render modals
                self.render_help_modal(ctx);
                self.render_search_input_modal(ctx);
                self.render_input_modal(ctx);

                // Strip-based layout with three panes and dividers
                use egui_extras::{Size, StripBuilder};
                StripBuilder::new(ui)
                    .size(Size::exact(self.ui.panel_widths[0]).at_least(style::PARENT_MIN))
                    .size(Size::exact(style::DIVIDER_WIDTH))
                    .size(Size::remainder())
                    .size(Size::exact(style::DIVIDER_WIDTH))
                    .size(Size::exact(self.ui.panel_widths[1]).at_least(style::PREVIEW_MIN))
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
            self.selection.selected_index = Some(idx);
        }
        if let Some(pending) = pending_selection.into_inner() {
            self.navigation.pending_selection_path = Some(pending);
        }
        if let Some(path) = next_navigation.into_inner() {
            self.navigate_to(path);
        }
        if let Some(action) = context_action.into_inner() {
            action(self);
        }
    }
}

