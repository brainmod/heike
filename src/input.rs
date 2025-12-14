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
                let dest = self.navigation.current_path.join(path.file_name().unwrap_or_default());

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
            self.ui.error_message = Some((errors.join(" | "), Instant::now()));
        } else if count > 0 {
            self.ui.info_message = Some((format!("Copied {} file(s)", count), Instant::now()));
        }

        if count > 0 {
            self.request_refresh();
        }
    }

    pub fn handle_input(&mut self, ctx: &egui::Context) {
        // 1. Bulk Rename Mode
        if matches!(self.mode.mode, AppMode::BulkRename { .. }) {
            if ctx.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.ctrl) {
                self.apply_bulk_rename();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.mode.set_mode(AppMode::Normal);
            }
            return;
        }

        // 2. Modal Inputs (Command, Filter, Rename, SearchInput)
        if matches!(
            self.mode.mode,
            AppMode::Command | AppMode::Filtering | AppMode::Rename | AppMode::SearchInput
        ) {
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                match self.mode.mode {
                    AppMode::Rename => self.perform_rename(),
                    AppMode::Command => self.execute_command(ctx),
                    AppMode::Filtering => {
                        // Finalize search and allow navigation in filtered results
                        self.mode.set_mode(AppMode::Normal);
                        // Keep the filtered results
                    }
                    AppMode::SearchInput => {
                        // Start search
                        if !self.ui.search_query.is_empty() {
                            self.ui.search_in_progress = true;
                            self.ui.search_file_count = 0;
                            let _ = self.command_tx.send(IoCommand::SearchContent {
                                query: self.ui.search_query.clone(),
                                root_path: self.navigation.current_path.clone(),
                                options: self.ui.search_options.clone(),
                            });
                        }
                        self.mode.set_mode(AppMode::Normal);
                    }
                    _ => {}
                }
            }
            if self.mode.mode == AppMode::Filtering && !ctx.input(|i| i.pointer.any_pressed()) {
                // Implicitly handled
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.mode.set_mode(AppMode::Normal);
                self.mode.command_buffer.clear();
                self.apply_filter();
            }
            return;
        }

        // 3. Confirmation Modals
        if self.mode.mode == AppMode::DeleteConfirm {
            if ctx.input(|i| i.key_pressed(egui::Key::Y) || i.key_pressed(egui::Key::Enter)) {
                self.perform_delete();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::N) || i.key_pressed(egui::Key::Escape)) {
                self.mode.set_mode(AppMode::Normal);
            }
            return;
        }

        if self.mode.mode == AppMode::Help {
            if ctx.input(|i| {
                i.key_pressed(egui::Key::Escape)
                    || i.key_pressed(egui::Key::Q)
                    || i.key_pressed(egui::Key::Questionmark)
            }) {
                self.mode.set_mode(AppMode::Normal);
            }
            return;
        }

        // Handle SearchResults mode navigation
        if let AppMode::SearchResults {
            query: ref current_query,
            ref results,
            ref mut selected_index,
        } = self.mode.mode
        {
            if ctx.input(|i| i.key_pressed(egui::Key::S) && i.modifiers.shift) {
                self.ui.search_query = current_query.clone();
                self.ui.search_in_progress = false;
                self.ui.search_file_count = 0;
                self.mode.set_mode(AppMode::SearchInput);
                self.mode.focus_input = true;
                return;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.mode.set_mode(AppMode::Normal);
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
            self.mode.set_mode(AppMode::Normal);
            self.mode.command_buffer.clear();
            self.selection.multi_selection.clear();
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
            self.mode.set_mode(AppMode::Command);
            self.mode.focus_input = true;
            self.mode.command_buffer.clear();
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Slash)) {
            self.mode.set_mode(AppMode::Filtering);
            self.mode.focus_input = true;
            self.mode.command_buffer.clear();
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Period)) {
            self.ui.show_hidden = !self.ui.show_hidden;
            self.request_refresh();
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::O) && i.modifiers.shift) {
            self.ui.sort_options.cycle_sort_by();
            self.apply_filter();
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::O) && i.modifiers.alt) {
            self.ui.sort_options.toggle_order();
            self.apply_filter();
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::O) && i.modifiers.ctrl) {
            self.ui.sort_options.toggle_dirs_first();
            self.apply_filter();
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Questionmark)) {
            self.mode.set_mode(AppMode::Help);
            return;
        }

        // --- Tab Management ---
        if ctx.input(|i| i.key_pressed(egui::Key::T) && i.modifiers.ctrl) {
            // Ctrl+T: New tab in current directory
            self.new_tab(None);
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::W) && i.modifiers.ctrl) {
            // Ctrl+W: Close current tab
            self.close_current_tab();
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Tab) && i.modifiers.ctrl && !i.modifiers.shift) {
            // Ctrl+Tab: Next tab
            self.next_tab();
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Tab) && i.modifiers.ctrl && i.modifiers.shift) {
            // Ctrl+Shift+Tab: Previous tab
            self.prev_tab();
            return;
        }
        // Alt+1 through Alt+9 to switch tabs
        for i in 1..=9 {
            let key_name = i.to_string();
            if let Some(key) = egui::Key::from_name(&key_name) {
                if ctx.input(|input| input.modifiers.alt && input.key_pressed(key)) {
                    self.switch_to_tab(i - 1);
                    return;
                }
            }
        }

        if ctx.input(|i| i.key_pressed(egui::Key::V) && !i.modifiers.shift) {
            if self.mode.mode == AppMode::Normal {
                // Enter visual mode
                self.mode.set_mode(AppMode::Visual);
                if let Some(idx) = self.selection.selected_index {
                    if let Some(entry) = self.entries.visible_entries.get(idx) {
                        self.selection.multi_selection.insert(entry.path.clone());
                    }
                }
            } else if self.mode.mode == AppMode::Visual {
                // Exit visual mode (unset)
                self.mode.set_mode(AppMode::Normal);
                self.selection.multi_selection.clear();
            }
            return;
        }
        if self.mode.mode == AppMode::Normal
            && ctx.input(|i| i.key_pressed(egui::Key::V) && i.modifiers.shift)
        {
            // Shift+V: Enter visual mode and select all
            self.mode.set_mode(AppMode::Visual);
            self.selection.multi_selection.clear();
            for entry in &self.entries.visible_entries {
                self.selection.multi_selection.insert(entry.path.clone());
            }
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::A) && i.modifiers.ctrl) {
            // Ctrl+A: Select all
            if self.mode.mode != AppMode::Visual {
                self.mode.set_mode(AppMode::Visual);
            }
            self.selection.multi_selection.clear();
            for entry in &self.entries.visible_entries {
                self.selection.multi_selection.insert(entry.path.clone());
            }
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
            // Space: Toggle selection of current item
            if let Some(idx) = self.selection.selected_index {
                if let Some(entry) = self.entries.visible_entries.get(idx) {
                    if self.selection.multi_selection.contains(&entry.path) {
                        self.selection.multi_selection.remove(&entry.path);
                    } else {
                        if self.mode.mode != AppMode::Visual {
                            self.mode.set_mode(AppMode::Visual);
                        }
                        self.selection.multi_selection.insert(entry.path.clone());
                    }
                }
            }
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::R) && i.modifiers.ctrl) {
            // Ctrl+R: Invert selection (select unselected, deselect selected)
            let unselected: Vec<_> = self
                .entries.visible_entries
                .iter()
                .filter(|e| !self.selection.multi_selection.contains(&e.path))
                .map(|e| e.path.clone())
                .collect();

            self.selection.multi_selection.clear();
            for path in unselected {
                self.selection.multi_selection.insert(path);
            }

            // Enter visual mode if we have selections
            if !self.selection.multi_selection.is_empty() {
                self.mode.set_mode(AppMode::Visual);
            }
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::S) && i.modifiers.shift) {
            self.ui.search_in_progress = false;
            self.ui.search_file_count = 0;
            self.mode.set_mode(AppMode::SearchInput);
            self.mode.focus_input = true;
            return;
        }

        // 5. File Operation Triggers (Phase 6)
        // Check if we're waiting for a bookmark key - if so, skip file operations
        let waiting_for_bookmark = if let Some(last) = self.selection.last_g_press {
            Instant::now().duration_since(last) < Duration::from_millis(500)
        } else {
            false
        };

        if !waiting_for_bookmark && ctx.input(|i| i.key_pressed(egui::Key::Y)) {
            self.yank_selection(ClipboardOp::Copy);
        }
        if !waiting_for_bookmark && ctx.input(|i| i.key_pressed(egui::Key::X)) {
            self.yank_selection(ClipboardOp::Cut);
        }
        if !waiting_for_bookmark && ctx.input(|i| i.key_pressed(egui::Key::P)) {
            self.paste_clipboard();
        }
        if !waiting_for_bookmark && ctx.input(|i| i.key_pressed(egui::Key::D) && !i.modifiers.ctrl) {
            self.mode.set_mode(AppMode::DeleteConfirm);
        }
        if !waiting_for_bookmark && ctx.input(|i| i.key_pressed(egui::Key::R) && i.modifiers.shift) {
            // Shift+R: Bulk rename - rename multiple files at once
            self.enter_bulk_rename_mode();
        }
        if !waiting_for_bookmark && ctx.input(|i| i.key_pressed(egui::Key::R) && !i.modifiers.shift) {
            if let Some(idx) = self.selection.selected_index {
                if let Some(entry) = self.entries.visible_entries.get(idx) {
                    self.mode.command_buffer = entry.name.clone();
                    self.mode.set_mode(AppMode::Rename);
                    self.mode.focus_input = true;
                }
            }
        }
        if !waiting_for_bookmark && ctx.input(|i| i.key_pressed(egui::Key::E)) {
            // 'e' key: open file with default app
            if let Some(idx) = self.selection.selected_index {
                if let Some(entry) = self.entries.visible_entries.get(idx) {
                    // For directories, enter them (same as 'l')
                    if entry.is_dir {
                        let path = entry.path.clone();
                        self.navigate_to(path);
                    } else {
                        // For files, open with default app
                        let _ = open::that(&entry.path);
                    }
                }
            }
        }
        if !waiting_for_bookmark && ctx.input(|i| i.key_pressed(egui::Key::E) && i.modifiers.shift) {
            // Shift+E: open command mode for extraction (user can use ':' commands)
            // For now, just show a message since extraction requires special handling
            if let Some(idx) = self.selection.selected_index {
                if let Some(entry) = self.entries.visible_entries.get(idx) {
                    if matches!(entry.extension.as_str(), "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz") {
                        self.ui.info_message = Some((
                            "Use ':extract <path>' command to extract this archive".into(),
                            Instant::now()
                        ));
                    } else {
                        self.ui.error_message = Some((
                            "Selected file is not an archive".into(),
                            Instant::now()
                        ));
                    }
                }
            }
        }

        // 6. Navigation (j/k/arrows)
        if self.entries.visible_entries.is_empty() {
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
        let max_idx = self.entries.visible_entries.len() - 1;
        let current = self.selection.selected_index.unwrap_or(0);
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
                || i.key_pressed(egui::Key::Minus)  // '-' for parent (vim standard)
        }) {
            self.navigate_up();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            if let Some(idx) = self.selection.selected_index {
                if let Some(entry) = self.entries.visible_entries.get(idx) {
                    let path = entry.path.clone();
                    self.navigate_to(path);
                }
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::L) || i.key_pressed(egui::Key::ArrowRight)) {
            if let Some(idx) = self.selection.selected_index {
                if let Some(entry) = self.entries.visible_entries.get(idx) {
                    if entry.is_dir {
                        let path = entry.path.clone();
                        self.navigate_to(path);
                    }
                }
            }
        }

        // Page-down / half-page navigation (vim style)
        if ctx.input(|i| i.key_pressed(egui::Key::D) && i.modifiers.ctrl) {
            // Ctrl-D: half-page down
            let page_size = (self.entries.visible_entries.len() / 2).max(1);
            new_index = (current + page_size).min(max_idx);
            changed = true;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::U) && i.modifiers.ctrl) {
            // Ctrl-U: half-page up
            let page_size = (self.entries.visible_entries.len() / 2).max(1);
            new_index = if current >= page_size { current - page_size } else { 0 };
            changed = true;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F) && i.modifiers.ctrl) {
            // Ctrl-F: full page down
            let page_size = self.entries.visible_entries.len().max(1);
            new_index = (current + page_size).min(max_idx);
            changed = true;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::B) && i.modifiers.ctrl) {
            // Ctrl-B: full page up
            let page_size = self.entries.visible_entries.len().max(1);
            new_index = if current >= page_size { current - page_size } else { 0 };
            changed = true;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::G) && i.modifiers.shift) {
            new_index = max_idx;
            changed = true;
        }
        // Handle 'g' key for navigation (gg=top, gX=bookmark)
        if ctx.input(|i| i.key_pressed(egui::Key::G) && !i.modifiers.shift) {
            let now = Instant::now();
            if let Some(last) = self.selection.last_g_press {
                if now.duration_since(last) < Duration::from_millis(500) {
                    // Double 'g' press - jump to top
                    new_index = 0;
                    self.selection.last_g_press = None;
                    changed = true;
                } else {
                    // Single 'g' press after timeout - start new sequence
                    self.selection.last_g_press = Some(now);
                }
            } else {
                // First 'g' press - start sequence
                self.selection.last_g_press = Some(now);
            }
        }

        // Check for bookmark navigation (g + key)
        if let Some(last) = self.selection.last_g_press {
            let elapsed = Instant::now().duration_since(last);
            if elapsed > Duration::from_millis(500) {
                // Timeout - clear the 'g' press
                self.selection.last_g_press = None;
            } else if elapsed > Duration::from_millis(10) {
                // Short delay to allow keyboard input processing
                // Check for any single-character key press for bookmarks
                let bookmark_key = ctx.input(|i| {
                    for key in &[
                        egui::Key::A, egui::Key::B, egui::Key::C, egui::Key::D, egui::Key::E, egui::Key::F,
                        egui::Key::H, egui::Key::I, egui::Key::J, egui::Key::K, egui::Key::L, egui::Key::M,
                        egui::Key::N, egui::Key::O, egui::Key::P, egui::Key::Q, egui::Key::R, egui::Key::S,
                        egui::Key::T, egui::Key::U, egui::Key::V, egui::Key::W, egui::Key::X, egui::Key::Y, egui::Key::Z,
                        egui::Key::Num0, egui::Key::Num1, egui::Key::Num2, egui::Key::Num3, egui::Key::Num4,
                        egui::Key::Num5, egui::Key::Num6, egui::Key::Num7, egui::Key::Num8, egui::Key::Num9,
                    ] {
                        if i.key_pressed(*key) {
                            return Some(key.name().to_lowercase());
                        }
                    }
                    None
                });

                if let Some(key) = bookmark_key {
                    if let Some(path) = self.bookmarks.resolve_path(&key) {
                        if path.is_dir() {
                            self.navigate_to(path);
                        } else {
                            self.ui.error_message = Some((
                                format!("Bookmark '{}' does not exist or is not a directory", key),
                                Instant::now()
                            ));
                        }
                    } else {
                        self.ui.info_message = Some((
                            format!("No bookmark '{}' defined", key),
                            Instant::now()
                        ));
                    }
                    self.selection.last_g_press = None;
                }
            }
        }

        if changed {
            self.selection.selected_index = Some(new_index);
            self.selection.last_selection_change = Instant::now();
            self.selection.disable_autoscroll = false; // Re-enable autoscroll on keyboard navigation
            if self.mode.mode == AppMode::Visual {
                if let Some(entry) = self.entries.visible_entries.get(new_index) {
                    self.selection.multi_selection.insert(entry.path.clone());
                }
            }
        }
    }
}
