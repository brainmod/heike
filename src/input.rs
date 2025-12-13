// Input handling for Heike
// Keyboard and mouse input processing

use crate::app::Heike;
use crate::state::ClipboardOp;
use crate::io::worker::IoCommand;
use crate::state::AppMode;
use eframe::egui;
use std::fs;
use std::time::{Duration, Instant};

impl Heike {
    pub fn handle_dropped_files(&mut self, dropped_files: &[egui::DroppedFile]) {
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
            self.error_message = Some((errors.join(" | "), Instant::now()));
        } else if count > 0 {
            self.info_message = Some((format!("Copied {} file(s)", count), Instant::now()));
        }

        if count > 0 {
            self.request_refresh();
        }
    }

    pub fn handle_input(&mut self, ctx: &egui::Context) {
        // 1. Modal Inputs (Command, Filter, Rename, SearchInput)
        if matches!(
            self.mode,
            AppMode::Command | AppMode::Filtering | AppMode::Rename | AppMode::SearchInput
        ) {
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                match self.mode {
                    AppMode::Rename => self.perform_rename(),
                    AppMode::Command => self.execute_command(ctx),
                    AppMode::Filtering => {
                        // Finalize search and allow navigation in filtered results
                        self.mode = AppMode::Normal;
                        // Keep the filtered results
                    }
                    AppMode::SearchInput => {
                        // Start search
                        if !self.search_query.is_empty() {
                            self.search_in_progress = true;
                            self.search_file_count = 0;
                            let _ = self.command_tx.send(IoCommand::SearchContent {
                                query: self.search_query.clone(),
                                root_path: self.current_path.clone(),
                                options: self.search_options.clone(),
                            });
                        }
                        self.mode = AppMode::Normal;
                    }
                    _ => {}
                }
            }
            if self.mode == AppMode::Filtering && !ctx.input(|i| i.pointer.any_pressed()) {
                // Implicitly handled
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.mode = AppMode::Normal;
                self.command_buffer.clear();
                self.apply_filter();
            }
            return;
        }

        // 2. Confirmation Modals
        if self.mode == AppMode::DeleteConfirm {
            if ctx.input(|i| i.key_pressed(egui::Key::Y) || i.key_pressed(egui::Key::Enter)) {
                self.perform_delete();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::N) || i.key_pressed(egui::Key::Escape)) {
                self.mode = AppMode::Normal;
            }
            return;
        }

        if self.mode == AppMode::Help {
            if ctx.input(|i| {
                i.key_pressed(egui::Key::Escape)
                    || i.key_pressed(egui::Key::Q)
                    || i.key_pressed(egui::Key::Questionmark)
            }) {
                self.mode = AppMode::Normal;
            }
            return;
        }

        // Handle SearchResults mode navigation
        if let AppMode::SearchResults {
            query: ref current_query,
            ref results,
            ref mut selected_index,
        } = self.mode
        {
            if ctx.input(|i| i.key_pressed(egui::Key::S) && i.modifiers.shift) {
                self.search_query = current_query.clone();
                self.search_in_progress = false;
                self.search_file_count = 0;
                self.mode = AppMode::SearchInput;
                self.focus_input = true;
                return;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.mode = AppMode::Normal;
                return;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::N) && !i.modifiers.shift) {
                if !results.is_empty() {
                    *selected_index = (*selected_index + 1) % results.len();
                }
                return;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::N) && i.modifiers.shift) {
                if !results.is_empty() {
                    *selected_index = if *selected_index == 0 {
                        results.len() - 1
                    } else {
                        *selected_index - 1
                    };
                }
                return;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                // Open the file at the match location
                if let Some(result) = results.get(*selected_index) {
                    if result.file_path.is_file() {
                        let _ = open::that(&result.file_path);
                    }
                }
                return;
            }
            // Allow other navigation within search results
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::J)) {
                if !results.is_empty() {
                    *selected_index = (*selected_index + 1) % results.len();
                }
                return;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::K)) {
                if !results.is_empty() {
                    *selected_index = if *selected_index == 0 {
                        results.len() - 1
                    } else {
                        *selected_index - 1
                    };
                }
                return;
            }
            return; // Don't process other keys in search results mode
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.mode = AppMode::Normal;
            self.command_buffer.clear();
            self.multi_selection.clear();
            self.apply_filter();
            return;
        }

        // 3. Global History keys
        if ctx.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::ArrowLeft)) {
            self.navigate_back();
            return;
        }
        if ctx.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::ArrowRight)) {
            self.navigate_forward();
            return;
        }

        // 4. Normal Mode Triggers
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
        if ctx.input(|i| i.key_pressed(egui::Key::Period)) {
            self.show_hidden = !self.show_hidden;
            self.request_refresh();
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::O) && i.modifiers.shift) {
            self.sort_options.cycle_sort_by();
            self.apply_filter();
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::O) && i.modifiers.alt) {
            self.sort_options.toggle_order();
            self.apply_filter();
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::O) && i.modifiers.ctrl) {
            self.sort_options.toggle_dirs_first();
            self.apply_filter();
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Questionmark)) {
            self.mode = AppMode::Help;
            return;
        }
        if self.mode == AppMode::Normal
            && ctx.input(|i| i.key_pressed(egui::Key::V) && !i.modifiers.shift)
        {
            self.mode = AppMode::Visual;
            if let Some(idx) = self.selected_index {
                if let Some(entry) = self.visible_entries.get(idx) {
                    self.multi_selection.insert(entry.path.clone());
                }
            }
            return;
        }
        if self.mode == AppMode::Normal
            && ctx.input(|i| i.key_pressed(egui::Key::V) && i.modifiers.shift)
        {
            // Shift+V: Enter visual mode and select all
            self.mode = AppMode::Visual;
            self.multi_selection.clear();
            for entry in &self.visible_entries {
                self.multi_selection.insert(entry.path.clone());
            }
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::A) && i.modifiers.ctrl) {
            // Ctrl+A: Select all
            if self.mode != AppMode::Visual {
                self.mode = AppMode::Visual;
            }
            self.multi_selection.clear();
            for entry in &self.visible_entries {
                self.multi_selection.insert(entry.path.clone());
            }
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
            // Space: Toggle selection of current item
            if let Some(idx) = self.selected_index {
                if let Some(entry) = self.visible_entries.get(idx) {
                    if self.multi_selection.contains(&entry.path) {
                        self.multi_selection.remove(&entry.path);
                    } else {
                        if self.mode != AppMode::Visual {
                            self.mode = AppMode::Visual;
                        }
                        self.multi_selection.insert(entry.path.clone());
                    }
                }
            }
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::S) && i.modifiers.shift) {
            self.search_in_progress = false;
            self.search_file_count = 0;
            self.mode = AppMode::SearchInput;
            self.focus_input = true;
            return;
        }

        // 5. File Operation Triggers (Phase 6)
        if ctx.input(|i| i.key_pressed(egui::Key::Y)) {
            self.yank_selection(ClipboardOp::Copy);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::X)) {
            self.yank_selection(ClipboardOp::Cut);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::P)) {
            self.paste_clipboard();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::D)) {
            self.mode = AppMode::DeleteConfirm;
        }
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
            if ctx.input(|i| {
                i.key_pressed(egui::Key::Backspace)
                    || i.key_pressed(egui::Key::H)
                    || i.key_pressed(egui::Key::ArrowLeft)
            }) {
                self.navigate_up();
            }
            return;
        }

        let mut changed = false;
        let max_idx = self.visible_entries.len() - 1;
        let current = self.selected_index.unwrap_or(0);
        let mut new_index = current;

        if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::J)) {
            new_index = if current >= max_idx { 0 } else { current + 1 };
            changed = true;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::K)) {
            new_index = if current == 0 { max_idx } else { current - 1 };
            changed = true;
        }
        if ctx.input(|i| {
            i.key_pressed(egui::Key::Backspace)
                || i.key_pressed(egui::Key::H)
                || i.key_pressed(egui::Key::ArrowLeft)
        }) {
            self.navigate_up();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            if let Some(idx) = self.selected_index {
                if let Some(entry) = self.visible_entries.get(idx) {
                    let path = entry.path.clone();
                    self.navigate_to(path);
                }
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::L) || i.key_pressed(egui::Key::ArrowRight)) {
            if let Some(idx) = self.selected_index {
                if let Some(entry) = self.visible_entries.get(idx) {
                    if entry.is_dir {
                        let path = entry.path.clone();
                        self.navigate_to(path);
                    }
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
            self.disable_autoscroll = false; // Re-enable autoscroll on keyboard navigation
            if self.mode == AppMode::Visual {
                if let Some(entry) = self.visible_entries.get(new_index) {
                    self.multi_selection.insert(entry.path.clone());
                }
            }
        }
    }
}
