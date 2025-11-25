use crate::io;
use crate::message::{Message, PreviewContent};
use crate::model::{Clipboard, ClipboardOp, ConfirmAction, FileEntry, Mode, SearchResult};
use crate::style::AppTheme;
use crate::subscription::{file_watcher, handle_key, keyboard_subscription};
use iced::keyboard;
use iced::widget::{column, container, pane_grid, row, scrollable, text, text_input, Column};
use iced::{Element, Length, Size, Subscription, Task, Theme};
use std::collections::{HashMap, HashSet};
use std::env;
use std::path::PathBuf;

pub struct Heike {
    // Navigation
    pub current_path: PathBuf,
    pub history: Vec<PathBuf>,
    pub history_idx: usize,

    // Entries
    pub entries: Vec<FileEntry>,
    pub parent_entries: Vec<FileEntry>,
    pub selected: Option<usize>,
    pub multi_select: HashSet<PathBuf>,

    // Mode & Input
    pub mode: Mode,
    pub input_buffer: String,

    // Clipboard
    pub clipboard: Clipboard,

    // Search
    pub search_results: Vec<SearchResult>,
    pub search_index: usize,

    // UI State
    pub show_hidden: bool,
    pub theme: AppTheme,
    pub message: Option<String>,
    pub error: Option<String>,

    // Preview
    pub preview_content: PreviewContent,

    // Loading states
    pub loading: bool,
}

impl Default for Heike {
    fn default() -> Self {
        let current_path = env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));

        Self {
            current_path,
            history: Vec::new(),
            history_idx: 0,
            entries: Vec::new(),
            parent_entries: Vec::new(),
            selected: Some(0),
            multi_select: HashSet::new(),
            mode: Mode::default(),
            input_buffer: String::new(),
            clipboard: Clipboard::new(),
            search_results: Vec::new(),
            search_index: 0,
            show_hidden: false,
            theme: AppTheme::default(),
            message: None,
            error: None,
            preview_content: PreviewContent::Loading,
            loading: false,
        }
    }
}

impl Heike {
    pub fn new() -> (Self, Task<Message>) {
        let app = Self::default();
        let path = app.current_path.clone();
        let show_hidden = app.show_hidden;

        (
            app,
            Task::perform(
                io::load_directory(path, show_hidden),
                Message::DirectoryLoaded,
            ),
        )
    }

    pub fn title(&self) -> String {
        format!("Heike - {}", self.current_path.display())
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::KeyPressed(key, modifiers) => {
                if let Some(msg) = handle_key(key, modifiers, &self.mode) {
                    return self.update(msg);
                }
                Task::none()
            }
            Message::Navigate(path) => self.navigate(path),
            Message::NavigateUp => self.navigate_up(),
            Message::NavigateBack => self.navigate_back(),
            Message::NavigateForward => self.navigate_forward(),
            Message::Select(index) => self.select(index),
            Message::SelectDelta(delta) => self.select_delta(delta),
            Message::SelectFirst => self.select_first(),
            Message::SelectLast => self.select_last(),
            Message::SetMode(mode) => self.set_mode(mode),
            Message::InputChanged(value) => self.input_changed(value),
            Message::InputSubmit => self.input_submit(),
            Message::CancelInput => self.cancel_input(),
            Message::Yank => self.yank(),
            Message::Cut => self.cut(),
            Message::Paste => self.paste(),
            Message::Delete => self.delete(),
            Message::ConfirmDelete => self.confirm_delete(),
            Message::Rename(new_name) => self.rename(new_name),
            Message::CreateDirectory(name) => self.create_directory(name),
            Message::CreateFile(name) => self.create_file(name),
            Message::ToggleMultiSelect => self.toggle_multi_select(),
            Message::ClearMultiSelect => self.clear_multi_select(),
            Message::DirectoryLoaded(result) => self.directory_loaded(result),
            Message::FileOperationComplete(result) => self.file_operation_complete(result),
            Message::ToggleHidden => self.toggle_hidden(),
            Message::ToggleTheme => self.toggle_theme(),
            Message::OpenFile(path) => self.open_file(path),
            Message::OpenInSystem(path) => self.open_in_system(path),
            Message::FileWatcherEvent(path) => self.file_watcher_event(path),
            Message::ShowError(msg) => self.show_error(msg),
            Message::ShowInfo(msg) => self.show_info(msg),
            _ => Task::none(),
        }
    }

    fn navigate(&mut self, path: PathBuf) -> Task<Message> {
        if path.is_dir() {
            self.current_path = path.clone();
            self.add_to_history(path.clone());
            self.selected = Some(0);
            self.loading = true;

            Task::perform(
                io::load_directory(path, self.show_hidden),
                Message::DirectoryLoaded,
            )
        } else {
            Task::none()
        }
    }

    fn navigate_up(&mut self) -> Task<Message> {
        if let Some(parent) = self.current_path.parent() {
            let parent = parent.to_path_buf();
            self.navigate(parent)
        } else {
            Task::none()
        }
    }

    fn navigate_back(&mut self) -> Task<Message> {
        if self.history_idx > 0 {
            self.history_idx -= 1;
            let path = self.history[self.history_idx].clone();
            Task::perform(
                io::load_directory(path, self.show_hidden),
                Message::DirectoryLoaded,
            )
        } else {
            Task::none()
        }
    }

    fn navigate_forward(&mut self) -> Task<Message> {
        if self.history_idx < self.history.len() - 1 {
            self.history_idx += 1;
            let path = self.history[self.history_idx].clone();
            Task::perform(
                io::load_directory(path, self.show_hidden),
                Message::DirectoryLoaded,
            )
        } else {
            Task::none()
        }
    }

    fn add_to_history(&mut self, path: PathBuf) {
        // Remove any history after current index
        self.history.truncate(self.history_idx + 1);
        self.history.push(path);
        self.history_idx = self.history.len() - 1;
    }

    fn select(&mut self, index: usize) -> Task<Message> {
        if index < self.entries.len() {
            self.selected = Some(index);
        }
        Task::none()
    }

    fn select_delta(&mut self, delta: i32) -> Task<Message> {
        if self.entries.is_empty() {
            return Task::none();
        }

        let new_index = if let Some(current) = self.selected {
            let new = current as i32 + delta;
            if new < 0 {
                self.entries.len() - 1 // Wrap to bottom
            } else if new >= self.entries.len() as i32 {
                0 // Wrap to top
            } else {
                new as usize
            }
        } else {
            0
        };

        self.selected = Some(new_index);
        Task::none()
    }

    fn select_first(&mut self) -> Task<Message> {
        if !self.entries.is_empty() {
            self.selected = Some(0);
        }
        Task::none()
    }

    fn select_last(&mut self) -> Task<Message> {
        if !self.entries.is_empty() {
            self.selected = Some(self.entries.len() - 1);
        }
        Task::none()
    }

    fn set_mode(&mut self, mode: Mode) -> Task<Message> {
        self.mode = mode;
        self.input_buffer.clear();
        Task::none()
    }

    fn input_changed(&mut self, value: String) -> Task<Message> {
        self.input_buffer = value;
        Task::none()
    }

    fn input_submit(&mut self) -> Task<Message> {
        match &self.mode {
            Mode::Normal | Mode::Visual => {
                // Enter directory or open file
                if let Some(idx) = self.selected {
                    if let Some(entry) = self.entries.get(idx) {
                        if entry.is_dir {
                            return self.navigate(entry.path.clone());
                        } else {
                            return self.open_in_system(entry.path.clone());
                        }
                    }
                }
                Task::none()
            }
            Mode::Command => self.execute_command(),
            Mode::Rename => {
                let new_name = self.input_buffer.clone();
                self.mode = Mode::Normal;
                self.rename(new_name)
            }
            Mode::Filter => {
                self.mode = Mode::Normal;
                Task::none()
            }
            Mode::Search => {
                // TODO: Implement search
                self.mode = Mode::Normal;
                Task::none()
            }
            _ => Task::none(),
        }
    }

    fn cancel_input(&mut self) -> Task<Message> {
        self.mode = Mode::Normal;
        self.input_buffer.clear();
        Task::none()
    }

    fn execute_command(&mut self) -> Task<Message> {
        let cmd = self.input_buffer.trim().to_string();
        self.mode = Mode::Normal;
        self.input_buffer.clear();

        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.is_empty() {
            return Task::none();
        }

        match parts[0] {
            "q" | "quit" => {
                // TODO: Exit application
                Task::none()
            }
            "mkdir" => {
                if parts.len() > 1 {
                    let name = parts[1..].join(" ");
                    self.create_directory(name)
                } else {
                    self.show_error("Usage: mkdir <name>".to_string())
                }
            }
            "touch" => {
                if parts.len() > 1 {
                    let name = parts[1..].join(" ");
                    self.create_file(name)
                } else {
                    self.show_error("Usage: touch <name>".to_string())
                }
            }
            _ => self.show_error(format!("Unknown command: {}", parts[0])),
        }
    }

    fn yank(&mut self) -> Task<Message> {
        let paths = self.get_selected_paths();
        if !paths.is_empty() {
            self.clipboard.set_copy(paths);
            self.show_info(format!("Yanked {} item(s)", self.clipboard.paths.len()))
        } else {
            Task::none()
        }
    }

    fn cut(&mut self) -> Task<Message> {
        let paths = self.get_selected_paths();
        if !paths.is_empty() {
            self.clipboard.set_cut(paths);
            self.show_info(format!("Cut {} item(s)", self.clipboard.paths.len()))
        } else {
            Task::none()
        }
    }

    fn paste(&mut self) -> Task<Message> {
        if self.clipboard.is_empty() {
            return Task::none();
        }

        let sources = self.clipboard.paths.clone();
        let dest = self.current_path.clone();
        let is_cut = self.clipboard.is_cut();

        if is_cut {
            self.clipboard.clear();
            Task::perform(
                io::move_files(sources, dest),
                Message::FileOperationComplete,
            )
        } else {
            Task::perform(
                io::copy_files(sources, dest),
                Message::FileOperationComplete,
            )
        }
    }

    fn delete(&mut self) -> Task<Message> {
        self.mode = Mode::Confirm(ConfirmAction::Delete);
        Task::none()
    }

    fn confirm_delete(&mut self) -> Task<Message> {
        self.mode = Mode::Normal;
        let paths = self.get_selected_paths();

        if !paths.is_empty() {
            Task::perform(io::delete_files(paths), Message::FileOperationComplete)
        } else {
            Task::none()
        }
    }

    fn rename(&mut self, new_name: String) -> Task<Message> {
        if let Some(idx) = self.selected {
            if let Some(entry) = self.entries.get(idx) {
                let old_path = entry.path.clone();
                return Task::perform(
                    io::rename_file(old_path, new_name),
                    Message::FileOperationComplete,
                );
            }
        }
        Task::none()
    }

    fn create_directory(&mut self, name: String) -> Task<Message> {
        let path = self.current_path.clone();
        Task::perform(
            io::create_directory(path, name),
            Message::FileOperationComplete,
        )
    }

    fn create_file(&mut self, name: String) -> Task<Message> {
        let path = self.current_path.clone();
        Task::perform(io::create_file(path, name), Message::FileOperationComplete)
    }

    fn toggle_multi_select(&mut self) -> Task<Message> {
        if let Some(idx) = self.selected {
            if let Some(entry) = self.entries.get(idx) {
                let path = entry.path.clone();
                if self.multi_select.contains(&path) {
                    self.multi_select.remove(&path);
                } else {
                    self.multi_select.insert(path);
                }
            }
        }
        Task::none()
    }

    fn clear_multi_select(&mut self) -> Task<Message> {
        self.multi_select.clear();
        Task::none()
    }

    fn get_selected_paths(&self) -> Vec<PathBuf> {
        if !self.multi_select.is_empty() {
            self.multi_select.iter().cloned().collect()
        } else if let Some(idx) = self.selected {
            if let Some(entry) = self.entries.get(idx) {
                vec![entry.path.clone()]
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    }

    fn directory_loaded(&mut self, result: Result<Vec<FileEntry>, String>) -> Task<Message> {
        self.loading = false;

        match result {
            Ok(entries) => {
                self.entries = entries;

                // Load parent directory entries
                if let Some(parent) = self.current_path.parent() {
                    let parent_path = parent.to_path_buf();
                    let show_hidden = self.show_hidden;

                    // Also spawn task to load parent entries
                    return Task::perform(
                        io::load_directory(parent_path, show_hidden),
                        |_| Message::ShowInfo("".to_string()), // Ignore result for now
                    );
                }

                Task::none()
            }
            Err(err) => self.show_error(err),
        }
    }

    fn file_operation_complete(&mut self, result: Result<String, String>) -> Task<Message> {
        match result {
            Ok(msg) => {
                // Reload directory after operation
                let path = self.current_path.clone();
                let show_hidden = self.show_hidden;

                Task::batch([
                    Task::perform(async move { Message::ShowInfo(msg) }, |m| m),
                    Task::perform(
                        io::load_directory(path, show_hidden),
                        Message::DirectoryLoaded,
                    ),
                ])
            }
            Err(err) => self.show_error(err),
        }
    }

    fn toggle_hidden(&mut self) -> Task<Message> {
        self.show_hidden = !self.show_hidden;
        let path = self.current_path.clone();
        Task::perform(
            io::load_directory(path, self.show_hidden),
            Message::DirectoryLoaded,
        )
    }

    fn toggle_theme(&mut self) -> Task<Message> {
        self.theme = self.theme.toggle();
        Task::none()
    }

    fn open_file(&mut self, path: PathBuf) -> Task<Message> {
        // TODO: Implement file opening in preview
        Task::none()
    }

    fn open_in_system(&mut self, path: PathBuf) -> Task<Message> {
        Task::perform(
            async move {
                match open::that(&path) {
                    Ok(_) => Message::ShowInfo(format!("Opened {}", path.display())),
                    Err(e) => Message::ShowError(format!("Failed to open: {}", e)),
                }
            },
            |m| m,
        )
    }

    fn file_watcher_event(&mut self, path: PathBuf) -> Task<Message> {
        if path == self.current_path {
            Task::perform(
                io::load_directory(path, self.show_hidden),
                Message::DirectoryLoaded,
            )
        } else {
            Task::none()
        }
    }

    fn show_error(&mut self, msg: String) -> Task<Message> {
        self.error = Some(msg);
        self.message = None;
        Task::none()
    }

    fn show_info(&mut self, msg: String) -> Task<Message> {
        if !msg.is_empty() {
            self.message = Some(msg);
            self.error = None;
        }
        Task::none()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            keyboard_subscription(self.mode.clone()),
            file_watcher(self.current_path.clone()),
        ])
    }

    pub fn view(&self) -> Element<Message> {
        let content = column![
            text("Heike - iced version (WIP)").size(24),
            text(format!("Path: {}", self.current_path.display())),
            text(format!("Mode: {:?}", self.mode)),
            text(format!("Entries: {}", self.entries.len())),
        ]
        .spacing(10)
        .padding(20);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub fn theme(&self) -> Theme {
        self.theme.to_iced_theme()
    }
}
