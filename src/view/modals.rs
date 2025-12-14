// Modal rendering functions (Help, Search Input, Command/Filter/Rename Input)
// Extracted from app.rs for better code organization

use crate::app::Heike;
use crate::io::worker::IoCommand;
use crate::state::AppMode;
use crate::style;
use eframe::egui;

impl Heike {
    pub(crate) fn render_help_modal(&mut self, ctx: &egui::Context) {
        if self.mode.mode == AppMode::Help {
            egui::Window::new("Help")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .default_width(style::modal_width(ctx))
                .show(ctx, |ui| {
                    ui.set_max_height(style::modal_max_height(ctx));
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
                            ui.label("h / Left Arrow / Backspace / -");
                            ui.label("Go to Parent");
                            ui.end_row();
                            ui.label("l / Right Arrow / e");
                            ui.label("Enter Directory / Open File");
                            ui.end_row();
                            ui.label("Enter");
                            ui.label("Open File / Enter Dir");
                            ui.end_row();
                            ui.label("Shift+E");
                            ui.label("Archive Info");
                            ui.end_row();
                            ui.label("gg / G");
                            ui.label("Top / Bottom");
                            ui.end_row();
                            ui.label("Ctrl+D / Ctrl+U");
                            ui.label("Half-Page Down / Up");
                            ui.end_row();
                            ui.label("Ctrl+F / Ctrl+B");
                            ui.label("Full-Page Down / Up");
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
                            ui.label("Ctrl+R");
                            ui.label("Invert Selection");
                            ui.end_row();
                            ui.label("y / x / p");
                            ui.label("Copy / Cut / Paste");
                            ui.end_row();
                            ui.label("d / r");
                            ui.label("Delete / Rename");
                            ui.end_row();
                            ui.label("R (Shift+r)");
                            ui.label("Bulk Rename (vidir-style)");
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
                            ui.label("g + key");
                            ui.label("Jump to Bookmark");
                            ui.end_row();
                        });
                        ui.add_space(10.0);
                        ui.heading("Tab Management");
                        ui.separator();
                        egui::Grid::new("tab_grid").striped(true).show(ui, |ui| {
                            ui.label("Ctrl+T");
                            ui.label("New Tab");
                            ui.end_row();
                            ui.label("Ctrl+W");
                            ui.label("Close Tab");
                            ui.end_row();
                            ui.label("Ctrl+Tab");
                            ui.label("Next Tab");
                            ui.end_row();
                            ui.label("Ctrl+Shift+Tab");
                            ui.label("Previous Tab");
                            ui.end_row();
                            ui.label("Alt+1...9");
                            ui.label("Switch to Tab 1-9");
                            ui.end_row();
                        });
                        ui.add_space(10.0);
                        ui.heading("Sort Options");
                        ui.separator();
                        egui::Grid::new("sort_grid").striped(true).show(ui, |ui| {
                            ui.label("Shift+O");
                            ui.label("Cycle Sort (Name → Size → Modified → Ext)");
                            ui.end_row();
                            ui.label("Alt+O");
                            ui.label("Toggle Order (Ascending ↔ Descending)");
                            ui.end_row();
                            ui.label("Ctrl+O");
                            ui.label("Toggle Directories First");
                            ui.end_row();
                        });
                        ui.add_space(10.0);
                        ui.heading("Available Bookmarks");
                        ui.separator();
                        for key in self.bookmarks.keys() {
                            if let Some(path) = self.bookmarks.resolve_path(&key) {
                                ui.label(format!("g{} → {}", key, path.display()));
                            }
                        }
                        ui.add_space(10.0);
                        if ui.button("Close").clicked() {
                            self.mode.set_mode(AppMode::Normal);
                        }
                    });
                });
        }
    }

    pub(crate) fn render_search_input_modal(&mut self, ctx: &egui::Context) {
        if self.mode.mode == AppMode::SearchInput {
            egui::Window::new("Content Search")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .default_width(style::modal_width(ctx))
                .show(ctx, |ui| {
                    ui.set_max_height(style::modal_max_height(ctx));
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.label("Search for content in files:");
                        ui.add_space(5.0);

                        let response = ui.text_edit_singleline(&mut self.ui.search_query);
                        if self.mode.focus_input {
                            response.request_focus();
                            self.mode.focus_input = false;
                        }

                        ui.add_space(10.0);
                        ui.label("Options:");
                        ui.checkbox(
                            &mut self.ui.search_options.case_sensitive,
                            "Case sensitive",
                        );
                        ui.checkbox(&mut self.ui.search_options.use_regex, "Use regex");
                        ui.checkbox(
                            &mut self.ui.search_options.search_hidden,
                            "Search hidden files",
                        );
                        ui.checkbox(&mut self.ui.search_options.search_pdfs, "Search PDFs");
                        ui.checkbox(
                            &mut self.ui.search_options.search_archives,
                            "Search archives",
                        );

                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            if ui.button("Search").clicked()
                                && !self.ui.search_query.is_empty()
                            {
                                self.ui.search_in_progress = true;
                                // Reset all search statistics
                                self.ui.search_file_count = 0;
                                self.ui.search_files_skipped = 0;
                                self.ui.search_errors = 0;
                                let _ = self.command_tx.send(IoCommand::SearchContent {
                                    query: self.ui.search_query.clone(),
                                    root_path: self.navigation.current_path.clone(),
                                    options: self.ui.search_options.clone(),
                                });
                                self.mode.set_mode(AppMode::Normal);
                            }
                            if ui.button("Cancel").clicked() {
                                self.mode.set_mode(AppMode::Normal);
                            }
                        });

                        if self.ui.search_in_progress {
                            ui.add_space(10.0);
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(format!(
                                    "Searching... ({} searched, {} skipped, {} errors)",
                                    self.ui.search_file_count,
                                    self.ui.search_files_skipped,
                                    self.ui.search_errors
                                ));
                            });
                        }
                    });
                });
        }
    }

    pub(crate) fn render_input_modal(&mut self, ctx: &egui::Context) {
        if matches!(
            self.mode.mode,
            AppMode::Command | AppMode::Filtering | AppMode::Rename
        ) {
            egui::Area::new("input_popup".into())
                .anchor(egui::Align2::CENTER_TOP, [0.0, 50.0])
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.set_min_width(400.0);
                        let prefix = match self.mode.mode {
                            AppMode::Rename => "Rename:",
                            AppMode::Filtering => "/",
                            _ => ":",
                        };
                        ui.horizontal(|ui| {
                            ui.label(prefix);
                            let response =
                                ui.text_edit_singleline(&mut self.mode.command_buffer);
                            if self.mode.focus_input {
                                response.request_focus();
                                self.mode.focus_input = false;
                            }
                        });
                    });
                });
        }
    }

    pub(crate) fn render_bulk_rename_modal(&mut self, ctx: &egui::Context) {
        // Extract the data we need before entering the closure
        let is_bulk_rename = matches!(self.mode.mode, AppMode::BulkRename { .. });
        if !is_bulk_rename {
            return;
        }

        let (file_count, focus_input) = if let AppMode::BulkRename {
            original_paths, ..
        } = &self.mode.mode
        {
            (original_paths.len(), self.mode.focus_input)
        } else {
            return;
        };

        egui::Window::new("Bulk Rename")
            .collapsible(false)
            .resizable(true)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .default_width(style::modal_width(ctx) * 1.2)
            .default_height(style::modal_max_height(ctx) * 0.8)
            .show(ctx, |ui| {
                ui.label(format!("Editing {} files (one per line):", file_count));
                ui.label(
                    egui::RichText::new("Press Ctrl+Enter to apply, Escape to cancel")
                        .weak()
                        .italics(),
                );
                ui.separator();

                // Get mutable reference to edit_buffer
                if let AppMode::BulkRename { edit_buffer, .. } = &mut self.mode.mode {
                    // Multi-line text editor
                    let response = ui.add_sized(
                        [ui.available_width(), ui.available_height() - 60.0],
                        egui::TextEdit::multiline(edit_buffer)
                            .font(egui::TextStyle::Monospace)
                            .code_editor()
                            .desired_width(f32::INFINITY),
                    );

                    if focus_input {
                        response.request_focus();
                        self.mode.focus_input = false;
                    }
                }

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Apply (Ctrl+Enter)").clicked() {
                        self.apply_bulk_rename();
                    }
                    if ui.button("Cancel (Esc)").clicked() {
                        self.mode.set_mode(AppMode::Normal);
                    }
                });
            });
    }
}
