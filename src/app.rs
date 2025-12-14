use crate::config::BookmarksConfig;
use crate::entry::FileEntry;
use crate::io::{fuzzy_match, spawn_worker, IoCommand, IoResult};
use crate::state::{
    AppMode, ClipboardOp, NavigationState, SelectionState, EntryState, UIState, ModeState, TabsManager,
};
use crate::style::{self, Theme};
use crate::view;

use eframe::egui;
use notify::{Event, RecursiveMode, Watcher};
use std::cell::RefCell;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, SyncSender};
use std::time::{Duration, Instant};
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

enum TabAction {
    SwitchTo(usize),
    Close(usize),
    New,
}

pub struct Heike {
    // Tabs management
    pub tabs: TabsManager,

    // Current tab state (synced with active tab)
    pub navigation: NavigationState,
    pub selection: SelectionState,
    pub entries: EntryState,

    // Global state
    pub ui: UIState,
    pub mode: ModeState,

    // Clipboard operations (shared across tabs)
    pub clipboard: HashSet<PathBuf>,
    pub clipboard_op: Option<ClipboardOp>,

    // Async I/O channels (bounded to prevent memory exhaustion)
    pub command_tx: SyncSender<IoCommand>,
    pub result_rx: Receiver<IoResult>,
    pub watcher: Option<Box<dyn Watcher>>,
    pub watcher_rx: Receiver<Result<Event, notify::Error>>,
    pub watched_path: Option<PathBuf>,

    // Resources
    pub syntax_set: SyntaxSet,
    pub theme_set: ThemeSet,
    pub bookmarks: BookmarksConfig,

    // Preview system
    pub preview_registry: view::PreviewRegistry,

    // Caching (interior mutability for preview cache)
    pub preview_cache: RefCell<view::PreviewCache>,

    // Parent directory cache to avoid redundant reads
    pub cached_parent_path: Option<PathBuf>,
    pub cached_show_hidden: bool,
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

        // Create preview registry and configure enabled handlers
        let mut preview_registry = view::create_default_registry();
        preview_registry.set_enabled_handlers(config.previews.enabled.clone());

        // Initialize tabs manager
        let tabs = TabsManager::new(start_path.clone());

        let mut app = Self {
            tabs,
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
            preview_registry,
            preview_cache: RefCell::new(view::PreviewCache::new()),
            cached_parent_path: None,
            cached_show_hidden: false,
        };

        app.request_refresh();
        app
    }

    // --- Tab Management ---

    fn save_current_tab_state(&mut self) {
        if let Some(tab) = self.tabs.get_active_mut() {
            tab.current_path = self.navigation.current_path.clone();
            tab.history = self.navigation.history.clone();
            tab.history_index = self.navigation.history_index;
            tab.all_entries = self.entries.all_entries.clone();
            tab.visible_entries = self.entries.visible_entries.clone();
            tab.parent_entries = self.entries.parent_entries.clone();
            tab.selected_index = self.selection.selected_index;
            tab.directory_selections = self.selection.directory_selections.clone();
            tab.pending_selection_path = self.navigation.pending_selection_path.clone();
            tab.update_label();
        }
    }

    fn load_active_tab_state(&mut self) {
        if let Some(tab) = self.tabs.get_active() {
            self.navigation.current_path = tab.current_path.clone();
            self.navigation.history = tab.history.clone();
            self.navigation.history_index = tab.history_index;
            self.entries.all_entries = tab.all_entries.clone();
            self.entries.visible_entries = tab.visible_entries.clone();
            self.entries.parent_entries = tab.parent_entries.clone();
            self.selection.selected_index = tab.selected_index;
            self.selection.directory_selections = tab.directory_selections.clone();
            self.navigation.pending_selection_path = tab.pending_selection_path.clone();
        }
    }

    pub(crate) fn switch_to_tab(&mut self, index: usize) {
        if index >= self.tabs.tab_count() {
            return;
        }
        // Save current tab state
        self.save_current_tab_state();
        // Switch to new tab
        self.tabs.switch_to_tab(index);
        // Load new tab state
        self.load_active_tab_state();
        // Refresh the new tab's directory
        self.request_refresh();
    }

    pub(crate) fn new_tab(&mut self, path: Option<PathBuf>) {
        let path = path.unwrap_or_else(|| self.navigation.current_path.clone());
        // Save current tab state
        self.save_current_tab_state();
        // Create new tab
        self.tabs.new_tab(path);
        // Load new tab state
        self.load_active_tab_state();
        // Refresh the new tab's directory
        self.request_refresh();
    }

    pub(crate) fn close_current_tab(&mut self) {
        if self.tabs.tab_count() <= 1 {
            self.ui.set_error("Cannot close the last tab".into());
            return;
        }
        // Close the tab (this automatically switches to another tab)
        if self.tabs.close_current_tab() {
            // Load the new active tab's state
            self.load_active_tab_state();
            // Refresh
            self.request_refresh();
        }
    }

    pub(crate) fn next_tab(&mut self) {
        if self.tabs.tab_count() <= 1 {
            return;
        }
        self.save_current_tab_state();
        self.tabs.next_tab();
        self.load_active_tab_state();
    }

    pub(crate) fn prev_tab(&mut self) {
        if self.tabs.tab_count() <= 1 {
            return;
        }
        self.save_current_tab_state();
        self.tabs.prev_tab();
        self.load_active_tab_state();
    }

    // --- Directory and File Operations ---

    pub(crate) fn request_refresh(&mut self) {
        self.ui.is_loading = true;
        self.ui.error_message = None;
        // Keep info message if it's fresh, or maybe clear it? Let's keep it for feedback.
        let _ = self.command_tx.send(IoCommand::LoadDirectory(
            self.navigation.current_path.clone(),
            self.ui.show_hidden,
        ));
        if let Some(parent) = self.navigation.current_path.parent() {
            let parent_path = parent.to_path_buf();

            // Only reload parent if it's different from the cached one or show_hidden changed
            let cache_valid = self.cached_parent_path.as_ref() == Some(&parent_path)
                && self.cached_show_hidden == self.ui.show_hidden;

            if !cache_valid {
                let _ = self.command_tx.send(IoCommand::LoadParent(
                    parent_path.clone(),
                    self.ui.show_hidden,
                ));
                // Mark that we've requested this parent (will be updated when result arrives)
                self.cached_parent_path = Some(parent_path);
                self.cached_show_hidden = self.ui.show_hidden;
            }
            // else: parent is cached and settings unchanged, skip redundant read
        } else {
            self.entries.parent_entries.clear();
            self.cached_parent_path = None;
        }
    }

    pub(crate) fn apply_filter(&mut self) {
        // Save currently selected item path before filtering
        let previously_selected = self
            .selection.selected_index
            .and_then(|idx| self.entries.visible_entries.get(idx))
            .map(|e| e.path.clone());

        if self.mode.mode == AppMode::Filtering && !self.mode.command_buffer.is_empty() {
            let query = self.mode.command_buffer.clone();
            self.entries.visible_entries = self
                .entries.all_entries
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
            .entries.visible_entries
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
                Ok(event) => {
                    // Handle file system events incrementally
                    self.handle_fs_event(event);
                }
                Err(e) => {
                    // Watcher error, but don't show it to avoid spam
                    eprintln!("Watcher error: {}", e);
                }
            }
        }
    }

    fn handle_fs_event(&mut self, event: Event) {
        use notify::EventKind;

        // Check if event affects the cached parent directory
        if let Some(cached_parent) = &self.cached_parent_path {
            let affects_parent = event.paths.iter().any(|p| {
                p.parent() == Some(cached_parent.as_path())
                    || p.as_path() == cached_parent.as_path()
            });
            if affects_parent {
                // Invalidate parent cache - parent directory has changed
                self.cached_parent_path = None;
            }
        }

        // Only handle events for the current directory
        let in_current_dir = event.paths.iter().any(|p| {
            p.parent() == Some(self.navigation.current_path.as_path())
                || p.as_path() == self.navigation.current_path.as_path()
        });

        if !in_current_dir {
            return;
        }

        match event.kind {
            EventKind::Create(_) => {
                // File/directory created - add to entries
                for path in &event.paths {
                    if path.parent() == Some(self.navigation.current_path.as_path()) {
                        if let Some(new_entry) = FileEntry::from_path(path.clone()) {
                            // Check if entry already exists
                            if !self.entries.all_entries.iter().any(|e| &e.path == path) {
                                self.entries.all_entries.push(new_entry);
                            }
                        }
                    }
                }
                self.apply_filter(); // Re-sort and filter
            }
            EventKind::Remove(_) => {
                // File/directory removed - remove from entries
                for path in &event.paths {
                    self.entries.all_entries.retain(|e| &e.path != path);
                    self.entries.visible_entries.retain(|e| &e.path != path);
                    self.entries.parent_entries.retain(|e| &e.path != path);
                    // Remove from multi-selection if present
                    self.selection.multi_selection.remove(path);
                }
                self.apply_filter();
                self.validate_selection();
            }
            EventKind::Modify(_) => {
                // File modified - update entry metadata
                for path in &event.paths {
                    if let Some(updated_entry) = FileEntry::from_path(path.clone()) {
                        // Update in all_entries
                        if let Some(entry) = self.entries.all_entries.iter_mut().find(|e| &e.path == path) {
                            *entry = updated_entry.clone();
                        }
                        // Update in visible_entries
                        if let Some(entry) = self.entries.visible_entries.iter_mut().find(|e| &e.path == path) {
                            *entry = updated_entry.clone();
                        }
                        // Update in parent_entries
                        if let Some(entry) = self.entries.parent_entries.iter_mut().find(|e| &e.path == path) {
                            *entry = updated_entry;
                        }
                    }
                }
            }
            _ => {
                // For other events (move, etc.), do a full refresh to be safe
                self.request_refresh();
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
            .entries.visible_entries
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
                    // Handle empty results: use None-like value (usize::MAX) to indicate no selection
                    let selected_index = if results.is_empty() { usize::MAX } else { 0 };
                    self.mode.set_mode(AppMode::SearchResults {
                        query: self.ui.search_query.clone(),
                        results,
                        selected_index,
                    });
                    if result_count == 0 {
                        self.ui.set_info("No matches found".into());
                    } else {
                        self.ui.set_info(format!(
                            "Found {} matches in {} files",
                            result_count, self.ui.search_file_count
                        ));
                    }
                }
                IoResult::SearchProgress {
                    files_searched,
                    files_skipped,
                    errors,
                } => {
                    self.ui.search_file_count = files_searched;
                    self.ui.search_files_skipped = files_skipped;
                    self.ui.search_errors = errors;
                }
                IoResult::Error(msg) => {
                    self.ui.is_loading = false;
                    self.ui.search_in_progress = false;
                    self.ui.set_error(msg);
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
            self.ui.set_error(format!("Could not open file: {}", e));
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

        self.ui.set_error("Previous directory no longer exists".into());
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
        // idx doesn't change - when we remove at idx, the next element shifts down to idx
        while idx < self.navigation.history.len() {
            let target = self.navigation.history[idx].clone();
            if target.is_dir() {
                self.navigation.history_index = idx;
                self.navigation.current_path = target;
                self.finish_navigation();
                return;
            }
            // Remove invalid (non-directory) entry; next element shifts to current idx
            // so we don't increment idx. history_index doesn't change since entries
            // before current position don't shift.
            self.navigation.history.remove(idx);
        }

        self.ui.set_error("Next directory no longer exists".into());
    }

    fn finish_navigation(&mut self) {
        self.mode.command_buffer.clear();
        self.mode.set_mode(AppMode::Normal);
        self.selection.multi_selection.clear();
        // Restore saved selection for this directory, or default to 0
        self.selection.selected_index = self
            .selection.directory_selections
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
        self.ui.set_info(format!("{} {} files", op_text, self.clipboard.len()));
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
            self.ui.set_error(errors.join(" | "));
        } else {
            self.ui.set_info(format!("Processed {} files", count));
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
            self.ui.set_error(format!("Failed to delete {} item(s)", error_count));
        } else {
            self.ui.set_info("Items moved to trash".into());
        }
    }

    pub(crate) fn perform_rename(&mut self) {
        if let Some(idx) = self.selection.selected_index {
            if let Some(entry) = self.entries.visible_entries.get(idx) {
                let new_name = self.mode.command_buffer.trim();
                if !new_name.is_empty() {
                    if let Some(parent) = entry.path.parent() {
                        let new_path = parent.join(new_name);
                        if let Err(e) = fs::rename(&entry.path, &new_path) {
                            self.ui.set_error(format!("Rename failed: {}", e));
                        } else {
                            self.ui.set_info("Renamed successfully".into());
                        }
                    } else {
                        self.ui.set_error("Cannot rename root path".into());
                    }
                }
            }
        }
        self.mode.set_mode(AppMode::Normal);
        self.mode.command_buffer.clear();
        self.request_refresh();
    }

    pub(crate) fn enter_bulk_rename_mode(&mut self) {
        // Determine which files to rename
        let files_to_rename: Vec<PathBuf> = if !self.selection.multi_selection.is_empty() {
            // Use multi-selection if available
            self.selection
                .multi_selection
                .iter()
                .cloned()
                .collect()
        } else if let Some(idx) = self.selection.selected_index {
            // Use current selection if no multi-selection
            if let Some(entry) = self.entries.visible_entries.get(idx) {
                vec![entry.path.clone()]
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        if files_to_rename.is_empty() {
            self.ui.set_error("No files selected for bulk rename".into());
            return;
        }

        // Create edit buffer with one filename per line
        let edit_buffer = files_to_rename
            .iter()
            .filter_map(|p| p.file_name().and_then(|n| n.to_str()))
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        self.mode.set_mode(AppMode::BulkRename {
            original_paths: files_to_rename,
            edit_buffer,
            cursor_line: 0,
        });
        self.mode.focus_input = true;
    }

    pub(crate) fn apply_bulk_rename(&mut self) {
        if let AppMode::BulkRename {
            original_paths,
            edit_buffer,
            ..
        } = &self.mode.mode
        {
            let new_names: Vec<&str> = edit_buffer.lines().collect();

            // Validation: number of lines must match number of files
            if new_names.len() != original_paths.len() {
                self.ui.set_error(format!(
                    "Line count mismatch: {} files but {} names",
                    original_paths.len(),
                    new_names.len()
                ));
                return;
            }

            // Validation: no empty names
            if new_names.iter().any(|n| n.trim().is_empty()) {
                self.ui.set_error("Empty filename not allowed".into());
                return;
            }

            // Validation: no duplicate names
            let mut seen = std::collections::HashSet::new();
            for name in &new_names {
                if !seen.insert(name.trim()) {
                    self.ui.set_error(format!("Duplicate filename: {}", name.trim()));
                    return;
                }
            }

            // Perform renames
            let mut success_count = 0;
            let mut errors = Vec::new();

            for (old_path, new_name) in original_paths.iter().zip(new_names.iter()) {
                let new_name = new_name.trim();
                if let Some(parent) = old_path.parent() {
                    let new_path = parent.join(new_name);

                    // Skip if name hasn't changed
                    if let Some(old_name) = old_path.file_name().and_then(|n| n.to_str()) {
                        if old_name == new_name {
                            success_count += 1;
                            continue;
                        }
                    }

                    // Check if target already exists (unless it's a case-only change)
                    if new_path.exists() && new_path != *old_path {
                        errors.push(format!("{}: target already exists", new_name));
                        continue;
                    }

                    match fs::rename(old_path, &new_path) {
                        Ok(()) => success_count += 1,
                        Err(e) => errors.push(format!("{}: {}", new_name, e)),
                    }
                }
            }

            // Clear multi-selection after bulk rename
            self.selection.multi_selection.clear();

            // Show results
            if !errors.is_empty() {
                self.ui.set_error(format!(
                    "Renamed {}/{} files. Errors: {}",
                    success_count,
                    original_paths.len(),
                    errors.join(", ")
                ));
            } else {
                self.ui.set_info(format!("Successfully renamed {} file(s)", success_count));
            }

            self.mode.set_mode(AppMode::Normal);
            self.request_refresh();
        }
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

    /// Save current UI settings to configuration file
    fn save_settings(&mut self) {
        use crate::config::{Config, ThemeConfig, PanelConfig, UiConfig, FontConfig};

        let theme_mode = match self.ui.theme {
            Theme::Light => "light",
            Theme::Dark => "dark",
        };

        let config = Config {
            theme: ThemeConfig {
                mode: theme_mode.to_string(),
            },
            font: FontConfig {
                font_size: 12.0,
                icon_size: 14.0,
            },
            panel: PanelConfig {
                parent_width: self.ui.panel_widths[0],
                preview_width: self.ui.panel_widths[1],
            },
            ui: UiConfig {
                show_hidden: self.ui.show_hidden,
                sort_by: match self.ui.sort_options.sort_by {
                    crate::state::SortBy::Name => "name",
                    crate::state::SortBy::Size => "size",
                    crate::state::SortBy::Modified => "modified",
                    crate::state::SortBy::Extension => "extension",
                }.to_string(),
                sort_order: match self.ui.sort_options.sort_order {
                    crate::state::SortOrder::Ascending => "asc",
                    crate::state::SortOrder::Descending => "desc",
                }.to_string(),
                dirs_first: self.ui.sort_options.dirs_first,
            },
            bookmarks: self.bookmarks.clone(),
            previews: crate::config::PreviewConfig {
                enabled: self.preview_registry.enabled_handler_names(),
            },
        };

        let _ = config.save();
        self.ui.last_settings_save = Instant::now();
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

        // Use modular preview system (header is rendered inside)
        view::render_preview(
            ui,
            entry,
            &self.preview_registry,
            self.ui.show_hidden,
            self.selection.last_selection_change,
            &self.selection.directory_selections,
            &self.syntax_set,
            &self.theme_set,
            self.ui.theme,
            next_navigation,
            pending_selection,
            &self.preview_cache,
        );
    }

    // --- Drag and Drop Handling ---


    // --- Rendering Methods ---

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
                    self.ui.set_error("Usage: mkdir <name>".into());
                } else {
                    let dir_name = parts[1..].join(" ");
                    let new_dir = self.navigation.current_path.join(&dir_name);
                    match fs::create_dir(&new_dir) {
                        Ok(_) => {
                            self.ui.set_info(format!("Created directory: {}", dir_name));
                            self.request_refresh();
                        }
                        Err(e) => {
                            self.ui.set_error(format!("Failed to create directory: {}", e));
                        }
                    }
                }
            }
            "touch" => {
                if parts.len() < 2 {
                    self.ui.set_error("Usage: touch <filename>".into());
                } else {
                    let file_name = parts[1..].join(" ");
                    let new_file = self.navigation.current_path.join(&file_name);
                    match fs::File::create(&new_file) {
                        Ok(_) => {
                            self.ui.set_info(format!("Created file: {}", file_name));
                            self.request_refresh();
                        }
                        Err(e) => {
                            self.ui.set_error(format!("Failed to create file: {}", e));
                        }
                    }
                }
            }
            "cd" => {
                if parts.len() < 2 {
                    // Navigate to home directory if no argument provided
                    if let Some(home) = directories::UserDirs::new() {
                        self.navigate_to(home.home_dir().to_path_buf());
                    }
                } else {
                    let path_str = parts[1..].join(" ");
                    let path = if path_str.starts_with('~') {
                        if let Some(home) = directories::UserDirs::new() {
                            let rest = &path_str[1..];
                            home.home_dir().join(rest)
                        } else {
                            PathBuf::from(path_str)
                        }
                    } else if path_str.starts_with('/') {
                        PathBuf::from(path_str)
                    } else {
                        self.navigation.current_path.join(path_str)
                    };
                    self.navigate_to(path);
                }
            }
            "help" => {
                self.ui.set_info("Commands: q/quit, mkdir <name>, touch <file>, cd <path>, help".into());
            }
            _ => {
                self.ui.set_error(format!("Unknown command: {}. Type 'help' for available commands.", parts[0]));
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
        self.ui.clear_expired_messages(style::MESSAGE_TIMEOUT_SECS);

        // Periodically save settings (every 10 seconds)
        if self.ui.last_settings_save.elapsed() > Duration::from_secs(10) {
            self.save_settings();
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

        // Render tab bar if multiple tabs exist
        let tab_count = self.tabs.tab_count();
        if tab_count > 1 {
            // Collect tab info before entering UI closure
            let tab_labels: Vec<String> = self.tabs.tabs.iter().map(|t| t.label.clone()).collect();
            let active_tab_index = self.tabs.active_tab;

            let tab_action = std::cell::RefCell::new(None::<TabAction>);

            egui::TopBottomPanel::top("tab_bar").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for (i, label) in tab_labels.iter().enumerate() {
                        let is_active = i == active_tab_index;
                        let response = ui.selectable_label(is_active, label);

                        if response.clicked() {
                            *tab_action.borrow_mut() = Some(TabAction::SwitchTo(i));
                        }

                        // Close button
                        if response.hovered() && tab_count > 1 {
                            let close_response = ui.small_button("×");
                            if close_response.clicked() {
                                *tab_action.borrow_mut() = Some(TabAction::Close(i));
                            }
                        }
                    }

                    // New tab button
                    if ui.button("+").clicked() {
                        *tab_action.borrow_mut() = Some(TabAction::New);
                    }
                });
            });

            // Execute tab action after UI rendering
            if let Some(action) = tab_action.into_inner() {
                match action {
                    TabAction::SwitchTo(i) => self.switch_to_tab(i),
                    TabAction::Close(i) => {
                        if i == active_tab_index {
                            self.close_current_tab();
                        } else {
                            self.save_current_tab_state();
                            self.tabs.close_tab(i);
                            self.load_active_tab_state();
                        }
                    }
                    TabAction::New => self.new_tab(None),
                }
            }
        }

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
                        AppMode::BulkRename { .. } => {
                            ui.colored_label(egui::Color32::ORANGE, "BULK RENAME");
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

                // Show sort options
                ui.separator();
                ui.label(self.ui.sort_options.display_string());

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
        } = self.mode.mode
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
                } = self.mode.mode
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
                self.render_bulk_rename_modal(ctx);

                self.render_tab_bar(ui);
                ui.add_space(6.0);

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

