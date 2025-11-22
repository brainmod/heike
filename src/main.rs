use eframe::egui;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, Instant, Duration};
use chrono::{DateTime, Local};
use std::env;
use std::io::Read;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::collections::{HashSet, HashMap};
use notify::{Watcher, RecursiveMode, Event};
use std::sync::mpsc::channel;

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
    size: u64,
    modified: SystemTime,
    extension: String,
}

impl FileEntry {
    fn from_path(path: PathBuf) -> Option<Self> {
        let metadata = fs::metadata(&path).ok()?;
        let name = path.file_name()?.to_string_lossy().to_string();
        let extension = path.extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        
        Some(Self {
            path,
            name,
            is_dir: metadata.is_dir(),
            size: metadata.len(),
            modified: metadata.modified().unwrap_or(SystemTime::now()),
            extension,
        })
    }

    fn get_icon(&self) -> &str {
        // Using emoji icons for universal compatibility
        // For better icon rendering, consider using Nerd Fonts with custom glyph mappings
        if self.is_dir { return "📁"; }
        match self.extension.as_str() {
            "rs" => "🦀", "toml" => "⚙️", "md" => "📝", "txt" => "📄",
            "png" | "jpg" | "jpeg" | "gif" | "webp" => "🖼️",
            "mp4" | "mkv" | "mov" => "🎬", "mp3" | "wav" | "flac" => "🎵",
            "zip" | "tar" | "gz" | "7z" | "rar" => "📦", "py" => "🐍",
            "js" | "ts" | "jsx" | "tsx" => "📜", "html" | "css" => "🌐",
            "json" | "yaml" | "yml" | "xml" => "📋", "pdf" => "📕",
            "exe" | "msi" | "bat" | "sh" => "🚀", _ => "📄",
        }
    }
}

// --- Modes ---

#[derive(Debug, PartialEq, Clone, Copy)]
enum AppMode {
    Normal,
    Visual,
    Filtering,
    Command,
    Help,
    Rename,        // New
    DeleteConfirm, // New
}

#[derive(Clone, Copy, PartialEq)]
enum ClipboardOp { Copy, Cut } // New

// --- Async Architecture ---

enum IoCommand { LoadDirectory(PathBuf, bool), LoadParent(PathBuf, bool) }
enum IoResult { DirectoryLoaded(Vec<FileEntry>), ParentLoaded(Vec<FileEntry>), Error(String) }

fn read_directory(path: &Path, show_hidden: bool) -> Result<Vec<FileEntry>, std::io::Error> {
    let mut entries = Vec::new();
    let read_dir = fs::read_dir(path)?;
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !show_hidden {
            if let Some(name) = path.file_name() {
                if name.to_string_lossy().starts_with('.') { continue; }
            }
        }
        if let Some(file_entry) = FileEntry::from_path(path) { entries.push(file_entry); }
    }
    entries.sort_by(|a, b| {
        if a.is_dir != b.is_dir { return b.is_dir.cmp(&a.is_dir); }
        a.name.to_lowercase().cmp(&b.name.to_lowercase())
    });
    Ok(entries)
}

fn fuzzy_match(text: &str, query: &str) -> bool {
    if query.is_empty() { return true; }
    let mut q_chars = query.chars();
    let mut q_char = match q_chars.next() { Some(c) => c, None => return true };
    for t_char in text.chars() {
        if t_char.eq_ignore_ascii_case(&q_char) {
            q_char = match q_chars.next() { Some(c) => c, None => return true };
        }
    }
    false
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
    
    // Mode State
    mode: AppMode,
    command_buffer: String,
    focus_input: bool,
    
    // Clipboard State (New)
    clipboard: HashSet<PathBuf>,
    clipboard_op: Option<ClipboardOp>,
    
    // UI State
    error_message: Option<String>,
    info_message: Option<String>, // New
    show_hidden: bool,
    theme: Theme,
    is_loading: bool,
    last_g_press: Option<Instant>,
    last_selection_change: Instant,

    // Async Communication
    command_tx: Sender<IoCommand>,
    result_rx: Receiver<IoResult>,

    // File System Watcher
    watcher: Option<Box<dyn Watcher>>,
    watcher_rx: Receiver<Result<Event, notify::Error>>,
    watched_path: Option<PathBuf>,
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
                    IoCommand::LoadDirectory(path, hidden) => {
                        match read_directory(&path, hidden) {
                            Ok(entries) => { let _ = res_tx.send(IoResult::DirectoryLoaded(entries)); }
                            Err(e) => { let _ = res_tx.send(IoResult::Error(e.to_string())); }
                        }
                    }
                    IoCommand::LoadParent(path, hidden) => {
                        match read_directory(&path, hidden) {
                            Ok(entries) => { let _ = res_tx.send(IoResult::ParentLoaded(entries)); }
                            Err(_) => { let _ = res_tx.send(IoResult::ParentLoaded(Vec::new())); }
                        }
                    }
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
            mode: AppMode::Normal,
            command_buffer: String::new(),
            focus_input: false,
            clipboard: HashSet::new(),    // Init
            clipboard_op: None,           // Init
            error_message: None,
            info_message: None,           // Init
            show_hidden: false,
            theme: Theme::Dark,
            is_loading: false,
            last_g_press: None,
            last_selection_change: Instant::now(),
            command_tx: cmd_tx,
            result_rx: res_rx,
            watcher: None,
            watcher_rx: watch_rx,
            watched_path: None,
        };
        
        app.request_refresh();
        app
    }

    fn request_refresh(&mut self) {
        self.is_loading = true;
        self.error_message = None;
        // Keep info message if it's fresh, or maybe clear it? Let's keep it for feedback.
        let _ = self.command_tx.send(IoCommand::LoadDirectory(self.current_path.clone(), self.show_hidden));
        if let Some(parent) = self.current_path.parent() {
            let _ = self.command_tx.send(IoCommand::LoadParent(parent.to_path_buf(), self.show_hidden));
        } else {
            self.parent_entries.clear();
        }
    }

    fn apply_filter(&mut self) {
        if self.mode == AppMode::Filtering && !self.command_buffer.is_empty() {
            let query = self.command_buffer.clone();
            self.visible_entries = self.all_entries.iter()
                .filter(|e| fuzzy_match(&e.name, &query))
                .cloned()
                .collect();
        } else {
            self.visible_entries = self.all_entries.clone();
        }
        if self.visible_entries.is_empty() {
            self.selected_index = None;
        } else if self.selected_index.is_none() {
            // Only set to 0 if there's no selection yet
            self.selected_index = Some(0);
        }
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
                    self.error_message = Some(format!("Failed to watch directory: {}", e));
                    self.watcher = None;
                    self.watched_path = None;
                } else {
                    self.watcher = Some(Box::new(watcher));
                    self.watched_path = Some(self.current_path.clone());
                }
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to create watcher: {}", e));
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
                IoResult::DirectoryLoaded(entries) => {
                    self.all_entries = entries;
                    self.is_loading = false;
                    self.apply_filter();
                    // Validate selection after loading
                    if let Some(idx) = self.selected_index {
                        if idx >= self.visible_entries.len() && !self.visible_entries.is_empty() {
                            self.selected_index = Some(self.visible_entries.len() - 1);
                        }
                    }
                }
                IoResult::ParentLoaded(entries) => { self.parent_entries = entries; }
                IoResult::Error(msg) => {
                    self.is_loading = false;
                    self.error_message = Some(msg);
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
                self.directory_selections.insert(self.current_path.clone(), idx);
            }

            self.current_path = path.clone();

            if self.history_index < self.history.len() - 1 {
                self.history.truncate(self.history_index + 1);
            }
            self.history.push(path);
            self.history_index = self.history.len() - 1;

            self.finish_navigation();
        } else if let Err(e) = open::that(&path) {
            self.error_message = Some(format!("Could not open file: {}", e));
        }
    }

    fn navigate_up(&mut self) {
        if let Some(parent) = self.current_path.parent() {
            // Save current selection before navigating up
            if let Some(idx) = self.selected_index {
                self.directory_selections.insert(self.current_path.clone(), idx);
            }
            self.navigate_to(parent.to_path_buf());
        }
    }

    fn navigate_back(&mut self) {
        if self.history_index > 0 {
            // Save current selection before navigating back
            if let Some(idx) = self.selected_index {
                self.directory_selections.insert(self.current_path.clone(), idx);
            }
            self.history_index -= 1;
            self.current_path = self.history[self.history_index].clone();
            self.finish_navigation();
        }
    }

    fn navigate_forward(&mut self) {
        if self.history_index < self.history.len() - 1 {
            // Save current selection before navigating forward
            if let Some(idx) = self.selected_index {
                self.directory_selections.insert(self.current_path.clone(), idx);
            }
            self.history_index += 1;
            self.current_path = self.history[self.history_index].clone();
            self.finish_navigation();
        }
    }

    fn finish_navigation(&mut self) {
        self.command_buffer.clear();
        self.mode = AppMode::Normal;
        self.multi_selection.clear();
        // Restore saved selection for this directory, or default to 0
        self.selected_index = self.directory_selections.get(&self.current_path).copied().or(Some(0));
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
        
        let op_text = if self.clipboard_op == Some(ClipboardOp::Copy) { "Yanked" } else { "Cut" };
        self.info_message = Some(format!("{} {} files", op_text, self.clipboard.len()));
    }

    fn paste_clipboard(&mut self) {
        if self.clipboard.is_empty() { return; }
        let op = match self.clipboard_op { Some(o) => o, None => return };

        let mut count = 0;
        let mut errors = Vec::new();

        for src in &self.clipboard {
            if let Some(name) = src.file_name() {
                let dest = self.current_path.join(name);
                if src.is_dir() {
                    if op == ClipboardOp::Cut {
                        if let Err(e) = fs::rename(src, &dest) { errors.push(format!("Move dir failed: {}", e)); }
                        else { count += 1; }
                    } else {
                        errors.push("Copying directories not supported in  Heike (lite)".into());
                    }
                } else if op == ClipboardOp::Copy {
                    if let Err(e) = fs::copy(src, &dest) { errors.push(format!("Copy file failed: {}", e)); }
                    else { count += 1; }
                } else if let Err(e) = fs::rename(src, &dest) {
                    errors.push(format!("Move file failed: {}", e));
                } else {
                    count += 1;
                }
            }
        }

        if !errors.is_empty() { self.error_message = Some(errors.join(" | ")); } 
        else { self.info_message = Some(format!("Processed {} files", count)); }

        if op == ClipboardOp::Cut { self.clipboard.clear(); self.clipboard_op = None; }
        self.request_refresh();
    }

    fn perform_delete(&mut self) {
        let targets = if !self.multi_selection.is_empty() {
            self.multi_selection.clone()
        } else if let Some(idx) = self.selected_index {
            if let Some(entry) = self.visible_entries.get(idx) {
                HashSet::from([entry.path.clone()])
            } else { HashSet::new() }
        } else { HashSet::new() };

        for path in targets {
            if path.is_dir() { let _ = fs::remove_dir_all(&path); } 
            else { let _ = fs::remove_file(&path); }
        }
        
        self.mode = AppMode::Normal;
        self.multi_selection.clear();
        self.request_refresh();
        self.info_message = Some("Items deleted".into());
    }

    fn perform_rename(&mut self) {
        if let Some(idx) = self.selected_index {
            if let Some(entry) = self.visible_entries.get(idx) {
                let new_name = self.command_buffer.trim();
                if !new_name.is_empty() {
                    let new_path = entry.path.parent().unwrap().join(new_name);
                    if let Err(e) = fs::rename(&entry.path, &new_path) {
                        self.error_message = Some(format!("Rename failed: {}", e));
                    } else {
                        self.info_message = Some("Renamed successfully".into());
                    }
                }
            }
        }
        self.mode = AppMode::Normal;
        self.command_buffer.clear();
        self.request_refresh();
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
            self.error_message = Some(errors.join(" | "));
        } else if count > 0 {
            self.info_message = Some(format!("Copied {} file(s)", count));
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
        if parts.is_empty() { return; }

        match parts[0] {
            "q" | "quit" => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            "mkdir" => {
                if parts.len() > 1 {
                    let new_dir = self.current_path.join(parts[1]);
                    if let Err(e) = fs::create_dir(&new_dir) { self.error_message = Some(format!("mkdir failed: {}", e)); } 
                    else { self.request_refresh(); }
                }
            },
            "touch" => {
                 if parts.len() > 1 {
                    let new_file = self.current_path.join(parts[1]);
                    if let Err(e) = fs::File::create(&new_file) { self.error_message = Some(format!("touch failed: {}", e)); } 
                    else { self.request_refresh(); }
                }
            }
            _ => { self.error_message = Some(format!("Unknown command: {}", parts[0])); }
        }
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        // 1. Modal Inputs (Command, Filter, Rename)
        if matches!(self.mode, AppMode::Command | AppMode::Filtering | AppMode::Rename) {
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                match self.mode {
                    AppMode::Rename => self.perform_rename(),
                    AppMode::Command => self.execute_command(ctx),
                    AppMode::Filtering => {
                        // Finalize search and allow navigation in filtered results
                        self.mode = AppMode::Normal;
                        // Keep the filtered results
                    }
                    _ => {}
                }
            }
            if self.mode == AppMode::Filtering && !ctx.input(|i| i.pointer.any_pressed()) {
                // Implicitly handled
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.mode = AppMode::Normal; self.command_buffer.clear(); self.apply_filter();
            }
            return;
        }
        
        // 2. Confirmation Modals
        if self.mode == AppMode::DeleteConfirm {
            if ctx.input(|i| i.key_pressed(egui::Key::Y) || i.key_pressed(egui::Key::Enter)) { self.perform_delete(); }
            if ctx.input(|i| i.key_pressed(egui::Key::N) || i.key_pressed(egui::Key::Escape)) { self.mode = AppMode::Normal; }
            return;
        }
        
        if self.mode == AppMode::Help {
             if ctx.input(|i| i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::Q) || i.key_pressed(egui::Key::Questionmark)) {
                 self.mode = AppMode::Normal;
             }
             return;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.mode = AppMode::Normal;
            self.command_buffer.clear();
            self.multi_selection.clear();
            self.apply_filter();
            return;
        }

        // 3. Global History keys
        if ctx.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::ArrowLeft)) { self.navigate_back(); return; }
        if ctx.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::ArrowRight)) { self.navigate_forward(); return; }

        // 4. Normal Mode Triggers
        if ctx.input(|i| i.key_pressed(egui::Key::Colon)) {
            self.mode = AppMode::Command; self.focus_input = true; self.command_buffer.clear(); return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Slash)) {
            self.mode = AppMode::Filtering; self.focus_input = true; self.command_buffer.clear(); return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Period)) {
            self.show_hidden = !self.show_hidden; self.request_refresh(); return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Questionmark)) {
            self.mode = AppMode::Help; return;
        }
        if self.mode == AppMode::Normal && ctx.input(|i| i.key_pressed(egui::Key::V)) {
            self.mode = AppMode::Visual;
            if let Some(idx) = self.selected_index {
                if let Some(entry) = self.visible_entries.get(idx) { self.multi_selection.insert(entry.path.clone()); }
            }
            return;
        }

        // 5. File Operation Triggers (Phase 6)
        if ctx.input(|i| i.key_pressed(egui::Key::Y)) { self.yank_selection(ClipboardOp::Copy); }
        if ctx.input(|i| i.key_pressed(egui::Key::X)) { self.yank_selection(ClipboardOp::Cut); }
        if ctx.input(|i| i.key_pressed(egui::Key::P)) { self.paste_clipboard(); }
        if ctx.input(|i| i.key_pressed(egui::Key::D)) { self.mode = AppMode::DeleteConfirm; }
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
             if ctx.input(|i| i.key_pressed(egui::Key::Backspace) || i.key_pressed(egui::Key::H)) { self.navigate_up(); }
            return;
        }

        let mut changed = false;
        let max_idx = self.visible_entries.len() - 1;
        let current = self.selected_index.unwrap_or(0);
        let mut new_index = current;

        if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::J)) {
            new_index = (current + 1).min(max_idx); changed = true;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::K)) {
            new_index = current.saturating_sub(1); changed = true;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Backspace) || i.key_pressed(egui::Key::H)) { self.navigate_up(); }
        if ctx.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::L)) {
            if let Some(idx) = self.selected_index {
                if let Some(entry) = self.visible_entries.get(idx) {
                    let path = entry.path.clone(); self.navigate_to(path);
                }
            }
        }

        if ctx.input(|i| i.key_pressed(egui::Key::G) && i.modifiers.shift) { new_index = max_idx; changed = true; }
        if ctx.input(|i| i.key_pressed(egui::Key::G) && !i.modifiers.shift) {
            let now = Instant::now();
            if let Some(last) = self.last_g_press {
                if now.duration_since(last) < Duration::from_millis(500) { new_index = 0; self.last_g_press = None; changed = true; } 
                else { self.last_g_press = Some(now); }
            } else { self.last_g_press = Some(now); }
        }
        if let Some(last) = self.last_g_press {
            if Instant::now().duration_since(last) > Duration::from_millis(500) { self.last_g_press = None; }
        }

        if changed {
            self.selected_index = Some(new_index);
            self.last_selection_change = Instant::now();
            if self.mode == AppMode::Visual {
                if let Some(entry) = self.visible_entries.get(new_index) { self.multi_selection.insert(entry.path.clone()); }
            }
        }
    }

    fn render_preview(&self, ui: &mut egui::Ui, next_navigation: &std::cell::RefCell<Option<PathBuf>>) {
        let idx = match self.selected_index {
            Some(i) => i, None => { ui.centered_and_justified(|ui| { ui.label("No file selected"); }); return; }
        };
        let entry = match self.visible_entries.get(idx) {
            Some(e) => e, None => return,
        };

        ui.heading(format!("{} {}", entry.get_icon(), entry.name));
        ui.add_space(5.0);
        ui.label(format!("Size: {}", bytesize::ByteSize(entry.size)));
        let datetime: DateTime<Local> = entry.modified.into();
        ui.label(format!("Modified: {}", datetime.format("%Y-%m-%d %H:%M")));
        ui.separator();

        if entry.is_dir {
            // Show directory contents in preview pane
            if self.last_selection_change.elapsed() <= Duration::from_millis(200) {
                ui.centered_and_justified(|ui| { ui.spinner(); });
                return;
            }

            match read_directory(&entry.path, self.show_hidden) {
                Ok(entries) => {
                    egui::ScrollArea::vertical().id_salt("preview_dir").show(ui, |ui| {
                        use egui_extras::{TableBuilder, Column};
                        TableBuilder::new(ui).striped(true).resizable(false)
                            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                            .column(Column::auto().at_least(30.0))
                            .column(Column::remainder())
                            .body(|body| {
                                body.rows(24.0, entries.len(), |mut row| {
                                    let preview_entry = &entries[row.index()];
                                    row.col(|ui| {
                                        ui.label(egui::RichText::new(preview_entry.get_icon()).size(14.0));
                                    });
                                    row.col(|ui| {
                                        if ui.selectable_label(false, &preview_entry.name).clicked() {
                                            *next_navigation.borrow_mut() = Some(preview_entry.path.clone());
                                        }
                                    });
                                });
                            });
                    });
                }
                Err(e) => {
                    ui.centered_and_justified(|ui| {
                        ui.colored_label(egui::Color32::RED, format!("Cannot read directory: {}", e));
                    });
                }
            }
            return;
        }
        if matches!(entry.extension.as_str(), "pdf") {
            ui.centered_and_justified(|ui| { ui.label("📕 PDF Preview Not Supported"); }); return;
        }

        if self.last_selection_change.elapsed() <= Duration::from_millis(200) {
            ui.centered_and_justified(|ui| { ui.spinner(); }); return; 
        }

        if matches!(entry.extension.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp") {
            let uri = format!("file://{}", entry.path.display());
            egui::ScrollArea::vertical().id_salt("preview_img").show(ui, |ui| {
                ui.add(egui::Image::new(uri).max_width(ui.available_width()));
            });
            return;
        }

        match fs::File::open(&entry.path) {
            Ok(mut file) => {
                let mut buffer = [0u8; 2048]; 
                match file.read(&mut buffer) {
                    Ok(n) if n > 0 => {
                        match std::str::from_utf8(&buffer[..n]) {
                            Ok(text) => {
                                egui::ScrollArea::vertical().id_salt("preview_text").show(ui, |ui| {
                                    // Poor man's syntax highlighting
                                    let is_code = matches!(entry.extension.as_str(), "rs" | "py" | "js" | "ts" | "toml" | "json");
                                    if is_code {
                                        for line in text.lines() {
                                            let trimmed = line.trim();
                                            if trimmed.starts_with("//") || trimmed.starts_with('#') {
                                                 ui.label(egui::RichText::new(line).color(egui::Color32::DARK_GREEN));
                                            } else if trimmed.contains("fn ") || trimmed.contains("struct ") || trimmed.contains("def ") || trimmed.contains("class ") {
                                                 ui.label(egui::RichText::new(line).color(egui::Color32::LIGHT_BLUE));
                                            } else {
                                                 ui.monospace(line);
                                            }
                                        }
                                    } else {
                                        ui.monospace(text);
                                    }
                                    
                                    if n == 2048 { ui.colored_label(egui::Color32::YELLOW, "\n--- Preview Truncated ---"); }
                                });
                            }
                            Err(_) => { ui.centered_and_justified(|ui| { ui.label("binary content"); }); }
                        }
                    }
                    Ok(_) => { ui.label("Empty File"); }
                    Err(e) => { ui.colored_label(egui::Color32::RED, format!("Read error: {}", e)); }
                }
            }
            Err(e) => { ui.colored_label(egui::Color32::RED, format!("Open error: {}", e)); }
        }
    }
}

impl eframe::App for Heike {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply theme
        match self.theme {
            Theme::Light => ctx.set_visuals(egui::Visuals::light()),
            Theme::Dark => ctx.set_visuals(egui::Visuals::dark()),
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
            if self.visible_entries.len() != old_len { self.last_selection_change = Instant::now(); }
        }

        let next_navigation = std::cell::RefCell::new(None);
        let next_selection = std::cell::RefCell::new(None);
        let context_action = std::cell::RefCell::new(None::<Box<dyn FnOnce(&mut Self)>>);

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                // History Controls
                if ui.button("⬅").on_hover_text("Back (Alt+Left)").clicked() { self.navigate_back(); }
                if ui.button("➡").on_hover_text("Forward (Alt+Right)").clicked() { self.navigate_forward(); }
                if ui.button("⬆").on_hover_text("Up (Backspace)").clicked() { self.navigate_up(); }
                ui.add_space(10.0);
                
                // Breadcrumbs
                let components: Vec<_> = self.current_path.components().collect();
                let mut path_acc = PathBuf::new();
                for component in components {
                    path_acc.push(component);
                    let name = component.as_os_str().to_string_lossy();
                    let label = if name.is_empty() { "/" } else { &name };
                    if ui.button(label).clicked() { *next_navigation.borrow_mut() = Some(path_acc.clone()); }
                    ui.label(">");
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.checkbox(&mut self.show_hidden, "Hidden (.)").changed() { self.request_refresh(); }

                    // Theme toggle
                    let theme_icon = match self.theme {
                        Theme::Light => "🌙",
                        Theme::Dark => "☀",
                    };
                    if ui.button(theme_icon).on_hover_text("Toggle theme").clicked() {
                        self.theme = match self.theme {
                            Theme::Light => Theme::Dark,
                            Theme::Dark => Theme::Light,
                        };
                    }

                    if ui.button("?").clicked() { self.mode = AppMode::Help; }

                    // Mode Indicator
                    match self.mode {
                        AppMode::Normal => { ui.label("NORMAL"); },
                        AppMode::Visual => { ui.colored_label(egui::Color32::LIGHT_BLUE, "VISUAL"); },
                        AppMode::Filtering => { ui.colored_label(egui::Color32::YELLOW, "FILTER"); },
                        AppMode::Command => { ui.colored_label(egui::Color32::RED, "COMMAND"); },
                        AppMode::Help => { ui.colored_label(egui::Color32::GREEN, "HELP"); },
                        AppMode::Rename => { ui.colored_label(egui::Color32::ORANGE, "RENAME"); },
                        AppMode::DeleteConfirm => { ui.colored_label(egui::Color32::RED, "CONFIRM DELETE?"); },
                    }
                });
            });
            ui.add_space(4.0);
        });

        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("{}/{} items", self.visible_entries.len(), self.all_entries.len()));
                if self.is_loading { ui.spinner(); }
                
                if let Some(msg) = &self.info_message { ui.colored_label(egui::Color32::GREEN, msg); }
                if let Some(err) = &self.error_message { ui.colored_label(egui::Color32::RED, format!(" | {}", err)); }
                
                if !self.multi_selection.is_empty() {
                    ui.separator();
                    ui.colored_label(egui::Color32::LIGHT_BLUE, format!("{} selected", self.multi_selection.len()));
                }
            });
        });

        egui::SidePanel::left("parent_panel").resizable(true).default_width(200.0).show(ctx, |ui| {
            ui.add_space(4.0);
            ui.vertical_centered(|ui| { ui.heading("Parent"); });
            ui.separator();
            egui::ScrollArea::vertical().id_salt("parent_scroll").show(ui, |ui| {
                use egui_extras::{TableBuilder, Column};
                TableBuilder::new(ui).striped(true).resizable(false)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::auto().at_least(30.0))
                    .column(Column::remainder())
                    .body(|body| {
                        body.rows(24.0, self.parent_entries.len(), |mut row| {
                            let entry = &self.parent_entries[row.index()];
                            let is_active = entry.path == self.current_path;

                            if is_active { row.set_selected(true); }

                            row.col(|ui| {
                                ui.label(egui::RichText::new(entry.get_icon()).size(14.0));
                            });
                            row.col(|ui| {
                                let text_color = if is_active { egui::Color32::from_rgb(100, 200, 255) } else { ui.visuals().text_color() };
                                if ui.selectable_label(is_active, egui::RichText::new(&entry.name).color(text_color)).clicked() {
                                    // If clicking the active directory in parent pane, navigate UP to parent
                                    // Otherwise, navigate to the clicked sibling directory
                                    if is_active {
                                        // Navigate to the parent of current (go up one level)
                                        if let Some(parent_path) = self.current_path.parent() {
                                            *next_navigation.borrow_mut() = Some(parent_path.to_path_buf());
                                        }
                                    } else {
                                        *next_navigation.borrow_mut() = Some(entry.path.clone());
                                    }
                                }
                            });
                        });
                    });
            });
        });

        egui::SidePanel::right("preview_panel").resizable(true).default_width(350.0).show(ctx, |ui| {
            ui.add_space(4.0);
            ui.vertical_centered(|ui| { ui.heading("Preview"); });
            ui.separator();
            self.render_preview(ui, &next_navigation);
        });

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
                        .color(egui::Color32::from_rgb(100, 200, 255))
                );
            }
            // Help Modal
            if self.mode == AppMode::Help {
                 egui::Window::new("Help")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.heading("Key Bindings");
                        ui.separator();
                        egui::Grid::new("help_grid").striped(true).show(ui, |ui| {
                            ui.label("j / Down"); ui.label("Next Item"); ui.end_row();
                            ui.label("k / Up"); ui.label("Previous Item"); ui.end_row();
                            ui.label("h / Backspace"); ui.label("Go to Parent"); ui.end_row();
                            ui.label("l / Enter"); ui.label("Open / Enter Dir"); ui.end_row();
                            ui.label("gg / G"); ui.label("Top / Bottom"); ui.end_row();
                            ui.label("Alt + Arrows"); ui.label("History"); ui.end_row();
                            ui.label("."); ui.label("Toggle Hidden"); ui.end_row();
                            ui.label("/"); ui.label("Filter Mode"); ui.end_row();
                            ui.label(":"); ui.label("Command Mode"); ui.end_row();
                            ui.label("v"); ui.label("Visual Select Mode"); ui.end_row();
                            ui.label("y / x / p"); ui.label("Copy / Cut / Paste"); ui.end_row();
                            ui.label("d / r"); ui.label("Delete / Rename"); ui.end_row();
                            ui.label("?"); ui.label("Toggle Help"); ui.end_row();
                        });
                        ui.add_space(10.0);
                        if ui.button("Close").clicked() { self.mode = AppMode::Normal; }
                    });
            }

            // Command/Filter/Rename Input Modal
            if matches!(self.mode, AppMode::Command | AppMode::Filtering | AppMode::Rename) {
                egui::Area::new("input_popup".into()).anchor(egui::Align2::CENTER_TOP, [0.0, 50.0]).order(egui::Order::Foreground).show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.set_min_width(400.0);
                        let prefix = match self.mode { AppMode::Rename => "Rename:", AppMode::Filtering => "/", _ => ":" };
                        ui.horizontal(|ui| {
                            ui.label(prefix);
                            let response = ui.text_edit_singleline(&mut self.command_buffer);
                            if self.focus_input { response.request_focus(); self.focus_input = false; }
                        });
                    });
                });
            }

            egui::ScrollArea::vertical().id_salt("current_scroll").auto_shrink([false, false]).show(ui, |ui| {
                use egui_extras::{TableBuilder, Column};
                let mut table = TableBuilder::new(ui).striped(true).resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::auto().at_least(30.0))
                    .column(Column::remainder());

                // Scroll to selected row if there is one
                if let Some(idx) = self.selected_index {
                    table = table.scroll_to_row(idx, None);
                }

                table.header(20.0, |mut header| { header.col(|ui| { ui.label(""); }); header.col(|ui| { ui.label("Name"); }); })
                    .body(|body| {
                        body.rows(24.0, self.visible_entries.len(), |mut row| {
                            let row_index = row.index();
                            let entry = &self.visible_entries[row_index];
                            let is_focused = self.selected_index == Some(row_index);
                            let is_multi_selected = self.multi_selection.contains(&entry.path);
                            let is_cut = self.clipboard_op == Some(ClipboardOp::Cut) && self.clipboard.contains(&entry.path);

                            if is_multi_selected || is_focused { row.set_selected(true); }

                            // Icon column
                            row.col(|ui| {
                                ui.label(egui::RichText::new(entry.get_icon()).size(14.0));
                            });

                            // Name column with context menu
                            row.col(|ui| {
                                let mut text = egui::RichText::new(&entry.name);
                                if is_multi_selected { text = text.color(egui::Color32::LIGHT_BLUE); }
                                if is_cut { text = text.color(egui::Color32::from_white_alpha(100)); } // Dimmed

                                let response = ui.selectable_label(is_focused, text);

                                if response.clicked() {
                                    *next_selection.borrow_mut() = Some(row_index);
                                    if entry.is_dir { *next_navigation.borrow_mut() = Some(entry.path.clone()); }
                                }

                                // Context menu on right-click
                                let entry_clone = entry.clone();
                                response.context_menu(|ui| {
                                    if ui.button("📂 Open").clicked() {
                                        if entry_clone.is_dir {
                                            *next_navigation.borrow_mut() = Some(entry_clone.path.clone());
                                        } else {
                                            let _ = open::that(&entry_clone.path);
                                        }
                                        ui.close();
                                    }

                                    ui.separator();

                                    if ui.button("📋 Copy (y)").clicked() {
                                        let path = entry_clone.path.clone();
                                        *context_action.borrow_mut() = Some(Box::new(move |app: &mut Self| {
                                            app.clipboard.clear();
                                            app.clipboard.insert(path);
                                            app.clipboard_op = Some(ClipboardOp::Copy);
                                            app.info_message = Some("Copied 1 file".into());
                                        }));
                                        ui.close();
                                    }

                                    if ui.button("✂️ Cut (x)").clicked() {
                                        let path = entry_clone.path.clone();
                                        *context_action.borrow_mut() = Some(Box::new(move |app: &mut Self| {
                                            app.clipboard.clear();
                                            app.clipboard.insert(path);
                                            app.clipboard_op = Some(ClipboardOp::Cut);
                                            app.info_message = Some("Cut 1 file".into());
                                        }));
                                        ui.close();
                                    }

                                    if ui.button("📥 Paste (p)").clicked() {
                                        *context_action.borrow_mut() = Some(Box::new(|app: &mut Self| {
                                            app.paste_clipboard();
                                        }));
                                        ui.close();
                                    }

                                    ui.separator();

                                    if ui.button("✏️ Rename (r)").clicked() {
                                        *next_selection.borrow_mut() = Some(row_index);
                                        let name = entry_clone.name.clone();
                                        *context_action.borrow_mut() = Some(Box::new(move |app: &mut Self| {
                                            app.command_buffer = name;
                                            app.mode = AppMode::Rename;
                                            app.focus_input = true;
                                        }));
                                        ui.close();
                                    }

                                    if ui.button("🗑️ Delete (d)").clicked() {
                                        *next_selection.borrow_mut() = Some(row_index);
                                        *context_action.borrow_mut() = Some(Box::new(|app: &mut Self| {
                                            app.mode = AppMode::DeleteConfirm;
                                        }));
                                        ui.close();
                                    }

                                    ui.separator();

                                    if ui.button("ℹ️ Properties").clicked() {
                                        let size = entry_clone.size;
                                        let modified = entry_clone.modified;
                                        let is_dir = entry_clone.is_dir;
                                        *context_action.borrow_mut() = Some(Box::new(move |app: &mut Self| {
                                            app.info_message = Some(format!(
                                                "{} | {} | Modified: {}",
                                                if is_dir { "Directory" } else { "File" },
                                                bytesize::ByteSize(size),
                                                chrono::DateTime::<chrono::Local>::from(modified)
                                                    .format("%Y-%m-%d %H:%M")
                                            ));
                                        }));
                                        ui.close();
                                    }
                                });
                            });
                        });
                    });
            });
        });

        if let Some(idx) = next_selection.into_inner() { self.selected_index = Some(idx); }
        if let Some(path) = next_navigation.into_inner() { self.navigate_to(path); }
        if let Some(action) = context_action.into_inner() { action(self); }
    }
}

fn main() -> eframe::Result<()> {
    // Load the app icon
    let icon_bytes = include_bytes!("../heike_icon.png");
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
        "Heike", options,
        Box::new(|cc| { egui_extras::install_image_loaders(&cc.egui_ctx); Ok(Box::new(Heike::new(cc.egui_ctx.clone()))) }),
    )
}