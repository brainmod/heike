use eframe::egui;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, Instant, Duration};
use chrono::{DateTime, Local};
use std::env;

// --- Data Structures ---

#[derive(Clone)]
struct FileEntry {
    path: PathBuf,
    name: String,
    is_dir: bool,
    size: u64,
    modified: SystemTime,
}

impl FileEntry {
    fn from_path(path: PathBuf) -> Option<Self> {
        let metadata = fs::metadata(&path).ok()?;
        let name = path.file_name()?.to_string_lossy().to_string();
        
        Some(Self {
            path,
            name,
            is_dir: metadata.is_dir(),
            size: metadata.len(),
            modified: metadata.modified().unwrap_or(SystemTime::now()),
        })
    }
}

struct RustyYazi {
    current_path: PathBuf,
    entries: Vec<FileEntry>,
    parent_entries: Vec<FileEntry>,
    selected_index: Option<usize>,
    search_query: String,
    error_message: Option<String>,
    show_hidden: bool,

    // Phase 2: Vim & Command State
    last_g_press: Option<Instant>,
    show_command_palette: bool,
    command_buffer: String,
    focus_command_input: bool, // Signal to focus the text box
}

impl Default for RustyYazi {
    fn default() -> Self {
        let start_path = directories::UserDirs::new()
            .map(|ud| ud.home_dir().to_path_buf())
            .unwrap_or_else(|| env::current_dir().unwrap_or_default());

        let mut app = Self {
            current_path: start_path.clone(),
            entries: Vec::new(),
            parent_entries: Vec::new(),
            selected_index: Some(0),
            search_query: String::new(),
            error_message: None,
            show_hidden: false,
            last_g_press: None,
            show_command_palette: false,
            command_buffer: String::new(),
            focus_command_input: false,
        };
        
        app.refresh_entries();
        app
    }
}

impl RustyYazi {
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

    fn refresh_entries(&mut self) {
        self.error_message = None;

        match Self::read_directory(&self.current_path, self.show_hidden) {
            Ok(entries) => {
                self.entries = entries;
                if !self.entries.is_empty() {
                    if self.selected_index.is_none() {
                         self.selected_index = Some(0);
                    }
                } else {
                    self.selected_index = None;
                }
            }
            Err(e) => {
                self.entries.clear();
                self.error_message = Some(format!("Error reading current: {}", e));
            }
        }

        if let Some(parent) = self.current_path.parent() {
            match Self::read_directory(parent, self.show_hidden) {
                Ok(entries) => self.parent_entries = entries,
                Err(_) => self.parent_entries.clear(),
            }
        } else {
            self.parent_entries.clear();
        }
    }

    fn navigate_to(&mut self, path: PathBuf) {
        if path.is_dir() {
            self.current_path = path;
            self.search_query.clear();
            self.selected_index = Some(0);
            self.refresh_entries();
        } else {
            if let Err(e) = open::that(&path) {
                self.error_message = Some(format!("Could not open file: {}", e));
            }
        }
    }

    fn navigate_up(&mut self) {
        if let Some(parent) = self.current_path.parent() {
            let old_current = self.current_path.clone();
            self.navigate_to(parent.to_path_buf());
            if let Some(idx) = self.entries.iter().position(|e| e.path == old_current) {
                self.selected_index = Some(idx);
            }
        }
    }

    fn execute_command(&mut self, ctx: &egui::Context) {
        let cmd = self.command_buffer.trim().to_string();
        self.show_command_palette = false;
        self.command_buffer.clear();

        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.is_empty() { return; }

        match parts[0] {
            "q" | "quit" => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            },
            "mkdir" => {
                if parts.len() > 1 {
                    let new_dir = self.current_path.join(parts[1]);
                    if let Err(e) = fs::create_dir(&new_dir) {
                        self.error_message = Some(format!("mkdir failed: {}", e));
                    } else {
                        self.refresh_entries();
                        // Auto-select new dir
                        if let Some(idx) = self.entries.iter().position(|e| e.path == new_dir) {
                            self.selected_index = Some(idx);
                        }
                    }
                } else {
                    self.error_message = Some("Usage: mkdir <name>".into());
                }
            },
            "touch" => {
                 if parts.len() > 1 {
                    let new_file = self.current_path.join(parts[1]);
                    if let Err(e) = fs::File::create(&new_file) {
                        self.error_message = Some(format!("touch failed: {}", e));
                    } else {
                        self.refresh_entries();
                        if let Some(idx) = self.entries.iter().position(|e| e.path == new_file) {
                            self.selected_index = Some(idx);
                        }
                    }
                } else {
                    self.error_message = Some("Usage: touch <name>".into());
                }
            }
            _ => {
                self.error_message = Some(format!("Unknown command: {}", parts[0]));
            }
        }
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        // 1. Global Toggle for Command Palette
        if !self.show_command_palette && ctx.input(|i| i.key_pressed(egui::Key::Colon)) {
            self.show_command_palette = true;
            self.focus_command_input = true;
            return;
        }

        // 2. If Command Palette is open, don't do file navigation
        if self.show_command_palette {
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.show_command_palette = false;
                self.command_buffer.clear();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.execute_command(ctx);
            }
            return; 
        }

        // 3. Navigation Logic (Vim + Arrows)
        if self.entries.is_empty() {
             if ctx.input(|i| i.key_pressed(egui::Key::Backspace) || i.key_pressed(egui::Key::H)) {
                self.navigate_up();
            }
            return;
        }

        let mut changed = false;
        let max_idx = self.entries.len() - 1;
        let current = self.selected_index.unwrap_or(0);

        // DOWN: j or ArrowDown
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::J)) {
            self.selected_index = Some((current + 1).min(max_idx));
            changed = true;
        }

        // UP: k or ArrowUp
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::K)) {
            self.selected_index = Some(current.saturating_sub(1));
            changed = true;
        }

        // PARENT: h or Backspace
        if ctx.input(|i| i.key_pressed(egui::Key::Backspace) || i.key_pressed(egui::Key::H)) {
            self.navigate_up();
        }

        // ENTER/CHILD: l or Enter
        if ctx.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::L)) {
            if let Some(idx) = self.selected_index {
                if let Some(entry) = self.entries.get(idx) {
                    let path = entry.path.clone();
                    self.navigate_to(path);
                }
            }
        }

        // BOTTOM: G (Shift+G)
        if ctx.input(|i| i.key_pressed(egui::Key::G) && i.modifiers.shift) {
            self.selected_index = Some(max_idx);
            changed = true;
        }

        // TOP: gg (double tap g)
        if ctx.input(|i| i.key_pressed(egui::Key::G) && !i.modifiers.shift) {
            let now = Instant::now();
            if let Some(last) = self.last_g_press {
                if now.duration_since(last) < Duration::from_millis(500) {
                    // Double tap detected
                    self.selected_index = Some(0);
                    self.last_g_press = None; // Reset
                    changed = true;
                } else {
                    self.last_g_press = Some(now);
                }
            } else {
                self.last_g_press = Some(now);
            }
        }

        // Reset g timer if too much time passes
        if let Some(last) = self.last_g_press {
            if Instant::now().duration_since(last) > Duration::from_millis(500) {
                self.last_g_press = None;
            }
        }
    }
}

impl eframe::App for RustyYazi {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_input(ctx);

        // Deferred actions
        let next_navigation = std::cell::RefCell::new(None);
        let next_selection = std::cell::RefCell::new(None);

        // --- Top Bar ---
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("⬆").clicked() {
                    self.navigate_up();
                }
                
                // Breadcrumbs
                ui.add_space(10.0);
                let components: Vec<_> = self.current_path.components().collect();
                let mut path_acc = PathBuf::new();

                for component in components {
                    path_acc.push(component);
                    let name = component.as_os_str().to_string_lossy();
                    // Special case for root on windows/unix
                    let label = if name.is_empty() { "/" } else { &name };
                    
                    if ui.button(label).clicked() {
                         *next_navigation.borrow_mut() = Some(path_acc.clone());
                    }
                    ui.label(">");
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.checkbox(&mut self.show_hidden, "Hidden").changed() {
                        self.refresh_entries();
                    }
                });
            });
            ui.add_space(4.0);
        });

        // --- Bottom Bar ---
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("{} items", self.entries.len()));
                if let Some(err) = &self.error_message {
                    ui.colored_label(egui::Color32::RED, format!(" | {}", err));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("?: Help | : Command Palette");
                });
            });
        });

        // --- 3-Pane Layout ---
        egui::SidePanel::left("parent_panel").resizable(true).default_width(200.0).show(ctx, |ui| {
            ui.add_space(4.0);
            ui.vertical_centered(|ui| { ui.heading("Parent"); });
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                for entry in &self.parent_entries {
                    let is_active = entry.path == self.current_path;
                    let text = if entry.is_dir { format!("📁 {}", entry.name) } else { format!("📄 {}", entry.name) };
                    let text_color = if is_active { egui::Color32::WHITE } else { egui::Color32::GRAY };
                    if ui.add(egui::Label::new(egui::RichText::new(text).color(text_color)).sense(egui::Sense::click())).clicked() {
                         *next_navigation.borrow_mut() = Some(entry.path.clone());
                    }
                }
            });
        });

        egui::SidePanel::right("preview_panel").resizable(true).default_width(300.0).show(ctx, |ui| {
            ui.add_space(4.0);
            ui.vertical_centered(|ui| { ui.heading("Preview"); });
            ui.separator();
            if let Some(idx) = self.selected_index {
                if let Some(entry) = self.entries.get(idx) {
                    ui.label(egui::RichText::new(&entry.name).strong().size(20.0));
                    ui.add_space(10.0);
                    ui.label(format!("Size: {}", bytesize::ByteSize(entry.size)));
                    let datetime: DateTime<Local> = entry.modified.into();
                    ui.label(format!("Modified: {}", datetime.format("%Y-%m-%d %H:%M")));
                    ui.add_space(20.0);
                    if entry.is_dir {
                        ui.label("📁 Directory");
                    } else {
                        ui.label("📄 File Content (TODO)");
                    }
                }
            } else {
                ui.centered_and_justified(|ui| { ui.label("No file selected"); });
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
             // Command Palette Overlay
            if self.show_command_palette {
                let area = egui::Area::new("command_palette".into())
                    .anchor(egui::Align2::CENTER_TOP, [0.0, 50.0])
                    .order(egui::Order::Foreground);
                
                area.show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.set_min_width(400.0);
                        ui.horizontal(|ui| {
                            ui.label(":");
                            let response = ui.text_edit_singleline(&mut self.command_buffer);
                            if self.focus_command_input {
                                response.request_focus();
                                self.focus_command_input = false;
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
                        body.rows(24.0, self.entries.len(), |mut row| {
                            let row_index = row.index();
                            let entry = &self.entries[row_index];
                            let is_selected = self.selected_index == Some(row_index);
                            row.set_selected(is_selected);
                            row.col(|ui| { ui.label(if entry.is_dir { "📁" } else { "📄" }); });
                            row.col(|ui| {
                                if ui.selectable_label(is_selected, &entry.name).clicked() {
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
            .with_inner_size([1000.0, 600.0])
            .with_title("Rusty Yazi"),
        ..Default::default()
    };
    eframe::run_native("Rusty Yazi", options, Box::new(|_cc| Ok(Box::new(RustyYazi::default()))))
}