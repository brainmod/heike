use eframe::egui;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, Instant, Duration};
use chrono::{DateTime, Local};
use std::env;
use std::io::Read;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::collections::HashSet;

// --- Data Structures ---

#[derive(Clone, Debug)]
struct FileEntry {
    path: PathBuf,
    name: String,
    is_dir: bool,
    size: u64,
    modified: SystemTime,
    extension: String, // Cached for icons
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
        if self.is_dir {
            return "📁";
        }
        match self.extension.as_str() {
            "rs" => "🦀",
            "toml" => "⚙️",
            "md" => "📝",
            "txt" => "📄",
            "png" | "jpg" | "jpeg" | "gif" | "webp" => "🖼️",
            "mp4" | "mkv" | "mov" => "🎬",
            "mp3" | "wav" | "flac" => "🎵",
            "zip" | "tar" | "gz" | "7z" | "rar" => "📦",
            "py" => "🐍",
            "js" | "ts" | "jsx" | "tsx" => "📜",
            "html" | "css" => "🌐",
            "json" | "yaml" | "yml" | "xml" => "📋",
            "pdf" => "📕",
            "exe" | "msi" | "bat" | "sh" => "🚀",
            _ => "📄",
        }
    }
}

// --- Modes ---

#[derive(Debug, PartialEq, Clone, Copy)]
enum AppMode {
    Normal,
    Visual,   // Multi-select
    Filtering, // Fuzzy search
    Command,  // : command
}

// --- Async Architecture ---

enum IoCommand {
    LoadDirectory(PathBuf, bool), 
    LoadParent(PathBuf, bool),   
}

enum IoResult {
    DirectoryLoaded(Vec<FileEntry>),
    ParentLoaded(Vec<FileEntry>),
    Error(String),
}

fn read_directory(path: &Path, show_hidden: bool) -> Result<Vec<FileEntry>, std::io::Error> {
    let mut entries = Vec::new();
    let read_dir = fs::read_dir(path)?;

    for entry_result in read_dir {
        if let Ok(entry) = entry_result {
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
    }

    entries.sort_by(|a, b| {
        if a.is_dir != b.is_dir {
            return b.is_dir.cmp(&a.is_dir);
        }
        a.name.to_lowercase().cmp(&b.name.to_lowercase())
    });

    Ok(entries)
}

// Simple fuzzy subsequence match
fn fuzzy_match(text: &str, query: &str) -> bool {
    if query.is_empty() { return true; }
    
    let mut q_chars = query.chars();
    let mut q_char = match q_chars.next() {
        Some(c) => c,
        None => return true,
    };
    
    for t_char in text.chars() {
        if t_char.to_ascii_lowercase() == q_char.to_ascii_lowercase() {
            q_char = match q_chars.next() {
                Some(c) => c,
                None => return true,
            };
        }
    }
    false
}

// --- Main App Struct ---

struct RustyYazi {
    // Core State
    current_path: PathBuf,
    all_entries: Vec<FileEntry>,      // Source of truth (raw)
    visible_entries: Vec<FileEntry>,  // Filtered view
    parent_entries: Vec<FileEntry>,
    
    // Navigation State
    selected_index: Option<usize>,
    multi_selection: HashSet<PathBuf>, // For Visual mode
    
    // Mode State
    mode: AppMode,
    command_buffer: String, // Shared for Command(:) and Filter(/)
    focus_input: bool,
    
    // UI State
    error_message: Option<String>,
    show_hidden: bool,
    is_loading: bool,
    last_g_press: Option<Instant>,
    last_selection_change: Instant,

    // Async Communication
    command_tx: Sender<IoCommand>,
    result_rx: Receiver<IoResult>,
}

impl RustyYazi {
    fn new(ctx: egui::Context) -> Self {
        let start_path = directories::UserDirs::new()
            .map(|ud| ud.home_dir().to_path_buf())
            .unwrap_or_else(|| env::current_dir().unwrap_or_default());

        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
        let (res_tx, res_rx) = std::sync::mpsc::channel();

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
            all_entries: Vec::new(),
            visible_entries: Vec::new(),
            parent_entries: Vec::new(),
            selected_index: Some(0),
            multi_selection: HashSet::new(),
            mode: AppMode::Normal,
            command_buffer: String::new(),
            focus_input: false,
            error_message: None,
            show_hidden: false,
            is_loading: false,
            last_g_press: None,
            last_selection_change: Instant::now(),
            command_tx: cmd_tx,
            result_rx: res_rx,
        };
        
        app.request_refresh();
        app
    }

    fn request_refresh(&mut self) {
        self.is_loading = true;
        self.error_message = None;
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
        
        // Reset selection if it went out of bounds
        if self.visible_entries.is_empty() {
            self.selected_index = None;
        } else {
            self.selected_index = Some(0);
        }
    }

    fn process_async_results(&mut self) {
        while let Ok(result) = self.result_rx.try_recv() {
            match result {
                IoResult::DirectoryLoaded(entries) => {
                    self.all_entries = entries;
                    self.is_loading = false;
                    self.apply_filter(); // Re-apply filter on new data
                }
                IoResult::ParentLoaded(entries) => {
                    self.parent_entries = entries;
                }
                IoResult::Error(msg) => {
                    self.is_loading = false;
                    self.error_message = Some(msg);
                    self.all_entries.clear();
                    self.visible_entries.clear();
                }
            }
        }
    }

    fn navigate_to(&mut self, path: PathBuf) {
        if path.is_dir() {
            self.current_path = path;
            self.command_buffer.clear();
            self.mode = AppMode::Normal; // Exit filter/visual on nav
            self.multi_selection.clear();
            self.selected_index = Some(0);
            self.request_refresh();
        } else {
            if let Err(e) = open::that(&path) {
                self.error_message = Some(format!("Could not open file: {}", e));
            }
        }
    }

    fn navigate_up(&mut self) {
        if let Some(parent) = self.current_path.parent() {
            self.current_path = parent.to_path_buf();
            self.request_refresh();
            self.selected_index = Some(0);
            self.mode = AppMode::Normal;
            self.multi_selection.clear();
        }
    }

    fn execute_command(&mut self, ctx: &egui::Context) {
        let cmd = self.command_buffer.trim().to_string();
        // Clear buffer and exit mode *before* execution logic
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
        // --- Mode Switching ---
        
        // Escape: Always return to Normal
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.mode = AppMode::Normal;
            self.command_buffer.clear();
            self.multi_selection.clear();
            self.apply_filter(); // clear filter
            return;
        }

        // If in Command or Filtering, Handle Text Input
        if matches!(self.mode, AppMode::Command | AppMode::Filtering) {
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                if self.mode == AppMode::Command {
                    self.execute_command(ctx);
                } else {
                    // For filtering, Enter just keeps the filter active but gives focus back to list
                    // Optional: You could make Enter "select" the top item.
                    // For now, let's just switch mode to Normal but KEEP the buffer (so filter persists)
                    // Actually, standard TUI behavior: Enter on filter usually selects the file.
                    // Let's keep it simple: Escape cancels filter, user must nav manually.
                }
            }
            // While filtering, update the list live
            if self.mode == AppMode::Filtering && ctx.input(|i| i.pointer.any_pressed()) == false {
                // Hack: we let the TextEdit handle input, but we need to trigger updates
                // The TextEdit updates `self.command_buffer` automatically.
                // We call `apply_filter` in `update` loop.
            }
            return;
        }

        // --- Normal & Visual Mode Inputs ---

        // Mode Triggers
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
        if self.mode == AppMode::Normal && ctx.input(|i| i.key_pressed(egui::Key::V)) {
            self.mode = AppMode::Visual;
            // Start selection at current
            if let Some(idx) = self.selected_index {
                if let Some(entry) = self.visible_entries.get(idx) {
                    self.multi_selection.insert(entry.path.clone());
                }
            }
            return;
        }

        // Navigation
        if self.visible_entries.is_empty() {
             if ctx.input(|i| i.key_pressed(egui::Key::Backspace) || i.key_pressed(egui::Key::H)) {
                self.navigate_up();
            }
            return;
        }

        let mut changed = false;
        let max_idx = self.visible_entries.len() - 1;
        let current = self.selected_index.unwrap_or(0);
        let mut new_index = current;

        if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::J)) {
            new_index = (current + 1).min(max_idx);
            changed = true;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::K)) {
            new_index = current.saturating_sub(1);
            changed = true;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Backspace) || i.key_pressed(egui::Key::H)) {
            self.navigate_up();
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::L)) {
            if let Some(idx) = self.selected_index {
                if let Some(entry) = self.visible_entries.get(idx) {
                    let path = entry.path.clone();
                    self.navigate_to(path);
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

            // Visual Mode Logic: Expand selection to new focus
            if self.mode == AppMode::Visual {
                if let Some(entry) = self.visible_entries.get(new_index) {
                    if self.multi_selection.contains(&entry.path) {
                        // Deselect if backtracking? No, usually just adds. 
                        // For simplicity: "v" mode toggles, movement adds.
                        self.multi_selection.insert(entry.path.clone());
                    } else {
                        self.multi_selection.insert(entry.path.clone());
                    }
                }
            }
        }
    }

    fn render_preview(&self, ui: &mut egui::Ui) {
        let idx = match self.selected_index {
            Some(i) => i,
            None => {
                ui.centered_and_justified(|ui| { ui.label("No file selected"); });
                return;
            }
        };

        let entry = match self.visible_entries.get(idx) {
            Some(e) => e,
            None => return,
        };

        ui.heading(format!("{} {}", entry.get_icon(), entry.name));
        ui.add_space(5.0);
        ui.label(format!("Size: {}", bytesize::ByteSize(entry.size)));
        let datetime: DateTime<Local> = entry.modified.into();
        ui.label(format!("Modified: {}", datetime.format("%Y-%m-%d %H:%M")));
        ui.separator();

        if entry.is_dir {
            ui.centered_and_justified(|ui| { ui.label("📁 Directory"); });
            return;
        }

        let is_settled = self.last_selection_change.elapsed() > Duration::from_millis(200);
        if !is_settled {
            ui.centered_and_justified(|ui| { ui.spinner(); });
            return; 
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
                                    ui.monospace(text);
                                    if n == 2048 {
                                        ui.colored_label(egui::Color32::YELLOW, "\n--- Preview Truncated ---");
                                    }
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

impl eframe::App for RustyYazi {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_async_results();
        self.handle_input(ctx);
        
        // Reactive Filter Update
        if self.mode == AppMode::Filtering {
            let old_len = self.visible_entries.len();
            self.apply_filter();
            if self.visible_entries.len() != old_len {
                self.last_selection_change = Instant::now(); // Trigger debounce
            }
        }

        let next_navigation = std::cell::RefCell::new(None);
        let next_selection = std::cell::RefCell::new(None);

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("⬆").clicked() { self.navigate_up(); }
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
                    if ui.checkbox(&mut self.show_hidden, "Hidden").changed() { self.request_refresh(); }
                    // Mode indicator
                    match self.mode {
                        AppMode::Normal => { ui.label("NORMAL"); },
                        AppMode::Visual => { ui.colored_label(egui::Color32::LIGHT_BLUE, "VISUAL"); },
                        AppMode::Filtering => { ui.colored_label(egui::Color32::YELLOW, "FILTER"); },
                        AppMode::Command => { ui.colored_label(egui::Color32::RED, "COMMAND"); },
                    }
                });
            });
            ui.add_space(4.0);
        });

        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("{}/{} items", self.visible_entries.len(), self.all_entries.len()));
                if self.is_loading { ui.spinner(); }
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
            egui::ScrollArea::vertical().show(ui, |ui| {
                for entry in &self.parent_entries {
                    let is_active = entry.path == self.current_path;
                    let text = format!("{} {}", entry.get_icon(), entry.name);
                    let text_color = if is_active { egui::Color32::WHITE } else { egui::Color32::GRAY };
                    if ui.add(egui::Label::new(egui::RichText::new(text).color(text_color)).sense(egui::Sense::click())).clicked() {
                         *next_navigation.borrow_mut() = Some(entry.path.clone());
                    }
                }
            });
        });

        egui::SidePanel::right("preview_panel").resizable(true).default_width(350.0).show(ctx, |ui| {
            ui.add_space(4.0);
            ui.vertical_centered(|ui| { ui.heading("Preview"); });
            ui.separator();
            self.render_preview(ui);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // Popup for Command or Filter input
            if matches!(self.mode, AppMode::Command | AppMode::Filtering) {
                let area = egui::Area::new("input_popup".into())
                    .anchor(egui::Align2::CENTER_TOP, [0.0, 50.0])
                    .order(egui::Order::Foreground);
                
                area.show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.set_min_width(400.0);
                        ui.horizontal(|ui| {
                            ui.label(if self.mode == AppMode::Command { ":" } else { "/" });
                            let response = ui.text_edit_singleline(&mut self.command_buffer);
                            if self.focus_input {
                                response.request_focus();
                                self.focus_input = false;
                            }
                        });
                    });
                });
            }

            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                use egui_extras::{TableBuilder, Column};
                TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::auto().at_least(30.0))
                    .column(Column::remainder())
                    .header(20.0, |mut header| {
                        header.col(|ui| { ui.label(""); });
                        header.col(|ui| { ui.label("Name"); });
                    })
                    .body(|body| {
                        body.rows(24.0, self.visible_entries.len(), |mut row| {
                            let row_index = row.index();
                            let entry = &self.visible_entries[row_index];
                            
                            // Selection Logic
                            let is_focused = self.selected_index == Some(row_index);
                            let is_multi_selected = self.multi_selection.contains(&entry.path);
                            
                            // Custom color for Visual selection
                            if is_multi_selected {
                                row.set_selected(true); // Highlights row
                            } else if is_focused {
                                row.set_selected(true); // Standard highlight
                            }

                            row.col(|ui| { ui.label(entry.get_icon()); });
                            row.col(|ui| {
                                let mut text = egui::RichText::new(&entry.name);
                                if is_multi_selected {
                                    text = text.color(egui::Color32::LIGHT_BLUE);
                                }
                                if ui.selectable_label(is_focused, text).clicked() {
                                    *next_selection.borrow_mut() = Some(row_index);
                                    if entry.is_dir { *next_navigation.borrow_mut() = Some(entry.path.clone()); }
                                }
                            });
                        });
                    });
            });
        });

        if let Some(idx) = next_selection.into_inner() { self.selected_index = Some(idx); }
        if let Some(path) = next_navigation.into_inner() { self.navigate_to(path); }
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 700.0])
            .with_title("Rusty Yazi"),
        ..Default::default()
    };
    eframe::run_native(
        "Rusty Yazi",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(RustyYazi::new(cc.egui_ctx.clone())))
        }),
    )
}