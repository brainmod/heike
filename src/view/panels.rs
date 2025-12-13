// Panel rendering for Heike
// Miller columns layout rendering

use crate::app::Heike;
use crate::state::{AppMode, ClipboardOp};
use crate::style;
use eframe::egui;
use std::path::PathBuf;
use std::time::Instant;

impl Heike {
    pub(crate) fn render_divider(&mut self, ui: &mut egui::Ui, index: usize) {
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
                        (self.panel_widths[0] + delta).clamp(style::PARENT_MIN, style::PARENT_MAX)
                }
                1 => {
                    self.panel_widths[1] = (self.panel_widths[1] - delta)
                        .clamp(style::PREVIEW_MIN, style::PREVIEW_MAX)
                }
                _ => {}
            }
        }
    }

    pub(crate) fn render_parent_pane(
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
                                let response = style::truncated_label_with_sense(
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

    pub(crate) fn render_current_pane(
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

                                let response = style::truncated_label_with_sense(
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
                                        let perms = entry_clone.get_permissions_string();
                                        *context_action.borrow_mut() =
                                            Some(Box::new(move |app: &mut Self| {
                                                app.info_message =
                                                    Some((
                                                        format!(
                                            "{} | {} | {} | Modified: {}",
                                            if is_dir { "Directory" } else { "File" },
                                            bytesize::ByteSize(size),
                                            perms,
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
}
