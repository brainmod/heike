use eframe::egui;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;
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
    selected_index: Option<usize>,
    search_query: String,
    error_message: Option<String>,
    show_hidden: bool,
}

impl Default for RustyYazi {
    fn default() -> Self {
        // Try to start at Home, fallback to current directory
        let start_path = directories::UserDirs::new()
            .map(|ud| ud.home_dir().to_path_buf())
            .unwrap_or_else(|| env::current_dir().unwrap_or_default());

        let mut app = Self {
            current_path: start_path.clone(),
            entries: Vec::new(),
            selected_index: Some(0),
            search_query: String::new(),
            error_message: None,
            show_hidden: false,
        };
        
        app.refresh_entries();
        app
    }
}

impl RustyYazi {
    /// Reloads the file list for the current directory
    fn refresh_entries(&mut self) {
        self.entries.clear();
        self.error_message = None;

        match fs::read_dir(&self.current_path) {
            Ok(read_dir) => {
                for entry_result in read_dir {
                    if let Ok(entry) = entry_result {
                        let path = entry.path();
                        
                        // Filter hidden files if toggle is off
                        if !self.show_hidden {
                            if let Some(name) = path.file_name() {
                                if name.to_string_lossy().starts_with('.') {
                                    continue;
                                }
                            }
                        }

                        if let Some(file_entry) = FileEntry::from_path(path) {
                            self.entries.push(file_entry);
                        }
                    }
                }

                // Sort: Directories first, then alphabetical
                self.entries.sort_by(|a, b| {
                    if a.is_dir != b.is_dir {
                        return b.is_dir.cmp(&a.is_dir);
                    }
                    a.name.to_lowercase().cmp(&b.name.to_lowercase())
                });

                // Reset selection safety
                if !self.entries.is_empty() {
                    self.selected_index = Some(0);
                } else {
                    self.selected_index = None;
                }
            }
            Err(e) => {
                self.error_message = Some(format!("Access denied or error: {}", e));
            }
        }
    }

    fn navigate_to(&mut self, path: PathBuf) {
        if path.is_dir() {
            self.current_path = path;
            self.search_query.clear();
            self.refresh_entries();
        } else {
            // If it's a file, try to open it with the system default
            if let Err(e) = open::that(&path) {
                self.error_message = Some(format!("Could not open file: {}", e));
            }
        }
    }

    fn navigate_up(&mut self) {
        if let Some(parent) = self.current_path.parent() {
            self.navigate_to(parent.to_path_buf());
        }
    }

    /// Handling keyboard navigation (Up, Down, Enter, Backspace)
    fn handle_input(&mut self, ctx: &egui::Context) {
        if self.entries.is_empty() {
            return;
        }

        let mut changed = false;
        let max_idx = self.entries.len() - 1;
        let current = self.selected_index.unwrap_or(0);

        if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
            self.selected_index = Some((current + 1).min(max_idx));
            changed = true;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
            self.selected_index = Some(current.saturating_sub(1));
            changed = true;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            if let Some(idx) = self.selected_index {
                if let Some(entry) = self.entries.get(idx) {
                    let path = entry.path.clone();
                    self.navigate_to(path);
                }
            }
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Backspace)) {
            self.navigate_up();
        }

        if changed {
            // Scroll to selection logic would go here in a more complex implementation
            // For now, eframe's ScrollArea handles visibility reasonably well if we auto-scroll
        }
    }
}

impl eframe::App for RustyYazi {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Global keyboard shortcuts
        self.handle_input(ctx);

        // Deferred action storage to satisfy borrow checker
        let next_navigation = std::cell::RefCell::new(None);
        let next_selection = std::cell::RefCell::new(None);

        // --- Top Bar (Navigation & Options) ---
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("⬆ Up").clicked() {
                    self.navigate_up();
                }
                
                if ui.button("⟳").on_hover_text("Refresh").clicked() {
                    self.refresh_entries();
                }

                // Editable path bar
                let mut path_str = self.current_path.to_string_lossy().to_string();
                let response = ui.add_sized(
                    ui.available_size() - egui::vec2(120.0, 0.0),
                    egui::TextEdit::singleline(&mut path_str)
                );
                
                // Navigate if user manually edits path and presses enter
                if response.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let new_path = PathBuf::from(&path_str);
                    if new_path.exists() {
                        self.navigate_to(new_path);
                    } else {
                        self.error_message = Some("Path does not exist".to_string());
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.checkbox(&mut self.show_hidden, "Hidden").changed() {
                        self.refresh_entries();
                    }
                });
            });
            ui.add_space(4.0);
        });

        // --- Bottom Bar (Status) ---
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let count = self.entries.len();
                ui.label(format!("{} items", count));
                
                if let Some(err) = &self.error_message {
                    ui.colored_label(egui::Color32::RED, format!(" | Error: {}", err));
                }
                
                if let Some(idx) = self.selected_index {
                    if let Some(entry) = self.entries.get(idx) {
                         ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let size_str = bytesize::ByteSize(entry.size).to_string();
                            ui.label(format!("Size: {}", size_str));
                        });
                    }
                }
            });
        });

        // --- Central Panel (File List) ---
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                
                // Setup Table
                use egui_extras::{TableBuilder, Column};
                
                TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::auto().at_least(40.0)) // Icon
                    .column(Column::initial(300.0).resizable(true)) // Name
                    .column(Column::initial(100.0)) // Size
                    .column(Column::remainder()) // Date
                    .header(20.0, |mut header| {
                        header.col(|ui| { ui.label("Type"); });
                        header.col(|ui| { ui.label("Name"); });
                        header.col(|ui| { ui.label("Size"); });
                        header.col(|ui| { ui.label("Modified"); });
                    })
                    .body(|body| {
                        let row_height = 24.0;
                        let num_rows = self.entries.len();

                        body.rows(row_height, num_rows, |mut row| {
                            let row_index = row.index();
                            let entry = &self.entries[row_index];
                            
                            // Highlight selected row
                            let is_selected = self.selected_index == Some(row_index);
                            row.set_selected(is_selected);

                            // 1. Icon Column
                            row.col(|ui| {
                                let icon = if entry.is_dir { "📁" } else { "📄" };
                                if ui.selectable_label(is_selected, icon).clicked() {
                                    *next_selection.borrow_mut() = Some(row_index);
                                    *next_navigation.borrow_mut() = Some(entry.path.clone());
                                }
                            });

                            // 2. Name Column
                            row.col(|ui| {
                                if ui.selectable_label(is_selected, &entry.name).clicked() {
                                    *next_selection.borrow_mut() = Some(row_index);
                                    *next_navigation.borrow_mut() = Some(entry.path.clone());
                                }
                            });

                            // 3. Size Column
                            row.col(|ui| {
                                let size_text = if entry.is_dir {
                                    "--".to_string()
                                } else {
                                    bytesize::ByteSize(entry.size).to_string()
                                };
                                ui.label(size_text);
                            });

                            // 4. Date Column
                            row.col(|ui| {
                                let datetime: DateTime<Local> = entry.modified.into();
                                ui.label(datetime.format("%Y-%m-%d %H:%M").to_string());
                            });
                        });
                    });
            });
        });

        // Apply deferred actions
        if let Some(idx) = next_selection.into_inner() {
            self.selected_index = Some(idx);
        }
        if let Some(path) = next_navigation.into_inner() {
            self.navigate_to(path);
        }
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("Rusty Yazi"),
        ..Default::default()
    };
    
    eframe::run_native(
        "Rusty Yazi",
        options,
        Box::new(|_cc| Ok(Box::new(RustyYazi::default()))),
    )
}