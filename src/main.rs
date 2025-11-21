use eframe::egui;
use std::fs;
use std::path::{Path, PathBuf};
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
    parent_entries: Vec<FileEntry>, // New: Store parent context
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
            parent_entries: Vec::new(),
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
    /// Helper to read a directory and return sorted entries
    fn read_directory(path: &Path, show_hidden: bool) -> Result<Vec<FileEntry>, std::io::Error> {
        let mut entries = Vec::new();
        let read_dir = fs::read_dir(path)?;

        for entry_result in read_dir {
            if let Ok(entry) = entry_result {
                let path = entry.path();
                
                // Filter hidden files
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

        // Sort: Directories first, then alphabetical
        entries.sort_by(|a, b| {
            if a.is_dir != b.is_dir {
                return b.is_dir.cmp(&a.is_dir);
            }
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        });

        Ok(entries)
    }

    /// Reloads entries for current AND parent directory
    fn refresh_entries(&mut self) {
        self.error_message = None;

        // 1. Load Current Directory
        match Self::read_directory(&self.current_path, self.show_hidden) {
            Ok(entries) => {
                self.entries = entries;
                // Reset selection if out of bounds, or default to 0
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

        // 2. Load Parent Directory (Context)
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
            self.selected_index = Some(0); // Reset selection on nav
            self.refresh_entries();
        } else {
            if let Err(e) = open::that(&path) {
                self.error_message = Some(format!("Could not open file: {}", e));
            }
        }
    }

    fn navigate_up(&mut self) {
        if let Some(parent) = self.current_path.parent() {
            // When going up, try to select the directory we just came from
            let old_current = self.current_path.clone();
            
            self.navigate_to(parent.to_path_buf());
            
            // Find index of old_current in new entries to restore selection position
            if let Some(idx) = self.entries.iter().position(|e| e.path == old_current) {
                self.selected_index = Some(idx);
            }
        }
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        if self.entries.is_empty() {
            // Even if empty, allow going up
             if ctx.input(|i| i.key_pressed(egui::Key::Backspace)) {
                self.navigate_up();
            }
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

        // TODO: Add Vim keys j/k here later
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
                if ui.button("⬆ Up").clicked() {
                    self.navigate_up();
                }
                if ui.button("⟳").on_hover_text("Refresh").clicked() {
                    self.refresh_entries();
                }

                let mut path_str = self.current_path.to_string_lossy().to_string();
                let response = ui.add_sized(
                    ui.available_size() - egui::vec2(120.0, 0.0),
                    egui::TextEdit::singleline(&mut path_str)
                );
                
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

        // --- Bottom Bar ---
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("{} items", self.entries.len()));
                if let Some(err) = &self.error_message {
                    ui.colored_label(egui::Color32::RED, format!(" | {}", err));
                }
            });
        });

        // --- 3-Pane Layout ---
        
        // 1. Left Panel: Parent Context
        egui::SidePanel::left("parent_panel")
            .resizable(true)
            .default_width(200.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.vertical_centered(|ui| { ui.heading("Parent"); });
                ui.separator();
                
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for entry in &self.parent_entries {
                        // Highlight the entry that matches our current directory
                        let is_active = entry.path == self.current_path;
                        
                        let text = if entry.is_dir {
                            format!("📁 {}", entry.name)
                        } else {
                            format!("📄 {}", entry.name)
                        };

                        // Use a weaker color for parent entries to imply "context"
                        let text_color = if is_active {
                            egui::Color32::WHITE
                        } else {
                            egui::Color32::GRAY
                        };

                        if ui.add(egui::Label::new(
                            egui::RichText::new(text).color(text_color)
                        ).sense(egui::Sense::click())).clicked() {
                            // Clicking parent items allows jumping to siblings
                             *next_navigation.borrow_mut() = Some(entry.path.clone());
                        }
                    }
                });
            });

        // 2. Right Panel: Preview
        egui::SidePanel::right("preview_panel")
            .resizable(true)
            .default_width(300.0)
            .show(ctx, |ui| {
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
                            ui.label("📁 Directory Content Preview (TODO)");
                        } else {
                            ui.label("📄 File Content Preview (TODO)");
                        }
                    }
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.label("No file selected");
                    });
                }
            });

        // 3. Central Panel: Current Directory (The Active Pane)
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                use egui_extras::{TableBuilder, Column};
                
                TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::auto().at_least(30.0)) // Icon
                    .column(Column::remainder())           // Name
                    .header(20.0, |mut header| {
                        header.col(|ui| { ui.label(""); }); // Icon header empty
                        header.col(|ui| { ui.label("Name"); });
                    })
                    .body(|body| {
                        let row_height = 24.0;
                        let num_rows = self.entries.len();

                        body.rows(row_height, num_rows, |mut row| {
                            let row_index = row.index();
                            let entry = &self.entries[row_index];
                            let is_selected = self.selected_index == Some(row_index);
                            
                            row.set_selected(is_selected);

                            row.col(|ui| {
                                let icon = if entry.is_dir { "📁" } else { "📄" };
                                ui.label(icon); 
                            });

                            row.col(|ui| {
                                if ui.selectable_label(is_selected, &entry.name).clicked() {
                                    *next_selection.borrow_mut() = Some(row_index);
                                    if entry.is_dir {
                                        *next_navigation.borrow_mut() = Some(entry.path.clone());
                                    }
                                }
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
            .with_inner_size([1000.0, 600.0]) // Wider default for 3 columns
            .with_title("Rusty Yazi"),
        ..Default::default()
    };
    
    eframe::run_native(
        "Rusty Yazi",
        options,
        Box::new(|_cc| Ok(Box::new(RustyYazi::default()))),
    )
}