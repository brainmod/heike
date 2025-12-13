// Modal rendering functions (Help, Search Input, Command/Filter/Rename Input)
// Extracted from app.rs for better code organization

use crate::app::Heike;
use crate::io::worker::IoCommand;
use crate::state::AppMode;
use crate::style;
use eframe::egui;

impl Heike {
    pub(crate) fn render_help_modal(&mut self, ctx: &egui::Context) {
        if self.mode == AppMode::Help {
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
    }

    pub(crate) fn render_search_input_modal(&mut self, ctx: &egui::Context) {
        if self.mode == AppMode::SearchInput {
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
    }

    pub(crate) fn render_input_modal(&mut self, ctx: &egui::Context) {
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
    }
}
