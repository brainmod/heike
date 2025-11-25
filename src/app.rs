use crate::io;
use crate::message::{Message, PreviewContent};
use crate::model::{Clipboard, ClipboardOp, ConfirmAction, FileEntry, Mode, SearchResult};
use crate::style::AppTheme;
use crate::subscription::{file_watcher, handle_key, keyboard_subscription};
use iced::keyboard;
use iced::widget::{button, column, container, mouse_area, pane_grid, row, scrollable, stack, text, text_input, Column};
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
    pub preview_entries: Vec<FileEntry>,
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
            preview_entries: Vec::new(),
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
            Message::ParentDirectoryLoaded(result) => self.parent_directory_loaded(result),
            Message::PreviewDirectoryLoaded(result) => self.preview_directory_loaded(result),
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

            // Load preview if it's a directory
            if let Some(entry) = self.entries.get(index) {
                if entry.is_dir {
                    let path = entry.path.clone();
                    let show_hidden = self.show_hidden;
                    return Task::perform(
                        io::load_directory(path, show_hidden),
                        Message::PreviewDirectoryLoaded,
                    );
                } else {
                    self.preview_entries.clear();
                }
            }
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

                // Load parent and preview directories
                let mut tasks = vec![];

                // Load parent directory entries
                if let Some(parent) = self.current_path.parent() {
                    let parent_path = parent.to_path_buf();
                    let show_hidden = self.show_hidden;
                    tasks.push(Task::perform(
                        io::load_directory(parent_path, show_hidden),
                        Message::ParentDirectoryLoaded,
                    ));
                }

                // Load preview for selected entry if it's a directory
                if let Some(idx) = self.selected {
                    if let Some(entry) = self.entries.get(idx) {
                        if entry.is_dir {
                            let path = entry.path.clone();
                            let show_hidden = self.show_hidden;
                            tasks.push(Task::perform(
                                io::load_directory(path, show_hidden),
                                Message::PreviewDirectoryLoaded,
                            ));
                        }
                    }
                }

                if tasks.is_empty() {
                    Task::none()
                } else {
                    Task::batch(tasks)
                }
            }
            Err(err) => self.show_error(err),
        }
    }

    fn parent_directory_loaded(&mut self, result: Result<Vec<FileEntry>, String>) -> Task<Message> {
        if let Ok(entries) = result {
            self.parent_entries = entries;
        }
        Task::none()
    }

    fn preview_directory_loaded(&mut self, result: Result<Vec<FileEntry>, String>) -> Task<Message> {
        if let Ok(entries) = result {
            self.preview_entries = entries;
        }
        Task::none()
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

    fn get_filtered_entries(&self) -> Vec<(usize, &FileEntry)> {
        if self.mode == Mode::Filter && !self.input_buffer.is_empty() {
            let filter = self.input_buffer.to_lowercase();
            self.entries
                .iter()
                .enumerate()
                .filter(|(_, entry)| entry.name.to_lowercase().contains(&filter))
                .collect()
        } else {
            self.entries.iter().enumerate().collect()
        }
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
        let breadcrumb = self.view_breadcrumb();
        let columns = self.view_miller_columns();
        let status = self.view_status_bar();

        let main_content = column![breadcrumb, columns, status]
            .width(Length::Fill)
            .height(Length::Fill);

        // Add modal overlays for different modes
        match &self.mode {
            Mode::Command | Mode::Filter | Mode::Rename | Mode::Search => {
                self.view_with_input_modal(main_content)
            }
            Mode::Confirm(action) => self.view_with_confirm_dialog(main_content, action),
            _ => main_content.into(),
        }
    }

    fn view_with_input_modal<'a>(&'a self, background: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
        let (title, placeholder) = match &self.mode {
            Mode::Command => ("Command", ":"),
            Mode::Filter => ("Filter", "Type to filter..."),
            Mode::Rename => ("Rename", "New name..."),
            Mode::Search => ("Search", "Search pattern..."),
            _ => ("Input", ""),
        };

        let input = text_input(placeholder, &self.input_buffer)
            .on_input(Message::InputChanged)
            .on_submit(Message::InputSubmit)
            .padding(10)
            .size(16);

        let modal_content = column![
            text(title).size(18),
            input,
            text("Press Enter to submit, Esc to cancel").size(12),
        ]
        .spacing(10)
        .padding(20);

        let modal = container(modal_content)
            .width(Length::Fixed(500.0))
            .style(|theme: &Theme| container::Style {
                background: Some(theme.extended_palette().background.base.color.into()),
                text_color: None,
                border: iced::Border {
                    color: theme.extended_palette().primary.strong.color,
                    width: 2.0,
                    radius: 8.0.into(),
                },
                shadow: iced::Shadow {
                    color: iced::Color::BLACK,
                    offset: iced::Vector::new(0.0, 4.0),
                    blur_radius: 16.0,
                },
            });

        iced::widget::stack![
            background.into(),
            container(modal)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|_theme: &Theme| container::Style {
                    background: Some(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.5).into()),
                    ..Default::default()
                })
        ]
        .into()
    }

    fn view_with_confirm_dialog<'a>(
        &'a self,
        background: impl Into<Element<'a, Message>>,
        action: &ConfirmAction,
    ) -> Element<'a, Message> {
        let message_text = match action {
            ConfirmAction::Delete => {
                let count = if !self.multi_select.is_empty() {
                    self.multi_select.len()
                } else {
                    1
                };
                format!("Delete {} item(s)?", count)
            }
            ConfirmAction::Overwrite => "Overwrite existing file?".to_string(),
        };

        let dialog_content = column![
            text(message_text).size(16),
            text("Press 'y' or Enter to confirm, 'n' or Esc to cancel").size(12),
        ]
        .spacing(15)
        .padding(20);

        let dialog = container(dialog_content)
            .width(Length::Fixed(400.0))
            .style(|theme: &Theme| container::Style {
                background: Some(theme.extended_palette().background.base.color.into()),
                text_color: None,
                border: iced::Border {
                    color: theme.extended_palette().danger.strong.color,
                    width: 2.0,
                    radius: 8.0.into(),
                },
                shadow: iced::Shadow {
                    color: iced::Color::BLACK,
                    offset: iced::Vector::new(0.0, 4.0),
                    blur_radius: 16.0,
                },
            });

        iced::widget::stack![
            background.into(),
            container(dialog)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|_theme: &Theme| container::Style {
                    background: Some(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.5).into()),
                    ..Default::default()
                })
        ]
        .into()
    }

    fn view_breadcrumb(&self) -> Element<Message> {
        let path_text = text(format!("  {}", self.current_path.display()))
            .size(16);

        container(path_text)
            .width(Length::Fill)
            .padding(10)
            .style(|theme: &Theme| container::Style {
                background: Some(theme.extended_palette().background.strong.color.into()),
                ..Default::default()
            })
            .into()
    }

    fn view_miller_columns(&self) -> Element<Message> {
        let parent_column = self.view_parent_pane();
        let current_column = self.view_current_pane();
        let preview_column = self.view_preview_pane();

        row![parent_column, current_column, preview_column]
            .spacing(2)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_parent_pane(&self) -> Element<Message> {
        let entries = if self.parent_entries.is_empty() {
            column![text("").size(12)]
        } else {
            let items: Vec<Element<Message>> = self
                .parent_entries
                .iter()
                .map(|entry| {
                    let icon = text(entry.get_icon()).size(14);
                    let name = text(&entry.name).size(14);
                    row![icon, name].spacing(8).into()
                })
                .collect();

            column(items).spacing(2)
        };

        container(scrollable(entries))
            .width(Length::FillPortion(1))
            .height(Length::Fill)
            .padding(10)
            .style(|theme: &Theme| container::Style {
                background: Some(theme.extended_palette().background.base.color.into()),
                border: iced::Border {
                    color: theme.extended_palette().background.strong.color,
                    width: 1.0,
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    }

    fn view_current_pane(&self) -> Element<Message> {
        if self.loading {
            return container(text("Loading...").size(14))
                .width(Length::FillPortion(2))
                .height(Length::Fill)
                .padding(10)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into();
        }

        let filtered = self.get_filtered_entries();

        let entries = if filtered.is_empty() {
            column![text(if self.mode == Mode::Filter {
                "No matches"
            } else {
                "Empty directory"
            })
            .size(14)]
        } else {
            let items: Vec<Element<Message>> = filtered
                .into_iter()
                .map(|(idx, entry)| {
                    let is_selected = self.selected == Some(idx);
                    let is_cut = self.clipboard.is_cut()
                        && self.clipboard.paths.contains(&entry.path);
                    let is_multi = self.multi_select.contains(&entry.path);

                    let icon = text(entry.get_icon()).size(14);
                    let name = text(&entry.name).size(14);

                    let size_str = if entry.is_dir {
                        String::from("DIR")
                    } else {
                        bytesize::ByteSize::b(entry.size).to_string()
                    };
                    let size_label = text(size_str).size(12);

                    let row_content = row![icon, name, size_label]
                        .spacing(8)
                        .padding(4);

                    let styled_container = container(row_content)
                        .width(Length::Fill)
                        .style(move |theme: &Theme| {
                            let palette = theme.extended_palette();
                            container::Style {
                                background: if is_selected {
                                    Some(palette.primary.weak.color.into())
                                } else if is_multi {
                                    Some(palette.success.weak.color.into())
                                } else {
                                    None
                                },
                                text_color: if is_cut {
                                    Some(palette.background.strong.text)
                                } else {
                                    None
                                },
                                ..Default::default()
                            }
                        });

                    mouse_area(styled_container)
                        .on_press(Message::Select(idx))
                        .into()
                })
                .collect();

            column(items).spacing(1)
        };

        container(scrollable(entries))
            .width(Length::FillPortion(2))
            .height(Length::Fill)
            .padding(10)
            .style(|theme: &Theme| container::Style {
                background: Some(theme.extended_palette().background.base.color.into()),
                border: iced::Border {
                    color: theme.extended_palette().background.strong.color,
                    width: 1.0,
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    }

    fn view_preview_pane(&self) -> Element<Message> {
        let content = if let Some(idx) = self.selected {
            if let Some(entry) = self.entries.get(idx) {
                if entry.is_dir {
                    // Show directory contents
                    if self.preview_entries.is_empty() {
                        column![text("Empty directory").size(14)]
                    } else {
                        let items: Vec<Element<Message>> = self
                            .preview_entries
                            .iter()
                            .map(|entry| {
                                let icon = text(entry.get_icon()).size(14);
                                let name = text(&entry.name).size(14);
                                row![icon, name].spacing(8).into()
                            })
                            .collect();
                        column(items).spacing(2)
                    }
                } else {
                    // Show file info
                    column![text(format!(
                        "{}\n\nSize: {}\nType: {}\nModified: {:?}",
                        entry.name,
                        bytesize::ByteSize::b(entry.size),
                        entry.extension,
                        entry.modified
                    ))
                    .size(14)]
                }
            } else {
                column![text("No selection").size(14)]
            }
        } else {
            column![text("No selection").size(14)]
        };

        container(scrollable(content))
            .width(Length::FillPortion(2))
            .height(Length::Fill)
            .padding(10)
            .style(|theme: &Theme| container::Style {
                background: Some(theme.extended_palette().background.base.color.into()),
                border: iced::Border {
                    color: theme.extended_palette().background.strong.color,
                    width: 1.0,
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    }

    fn view_status_bar(&self) -> Element<Message> {
        let mode_text = match &self.mode {
            Mode::Normal => "NORMAL",
            Mode::Visual => "VISUAL",
            Mode::Filter => "FILTER",
            Mode::Command => "COMMAND",
            Mode::Rename => "RENAME",
            Mode::Search => "SEARCH",
            Mode::SearchResults(_) => "RESULTS",
            Mode::Confirm(_) => "CONFIRM",
            Mode::GPrefix => "G",
        };

        let item_text = if self.mode == Mode::Filter && !self.input_buffer.is_empty() {
            let filtered_count = self.get_filtered_entries().len();
            format!(
                "Items: {}/{} | Selected: {}",
                filtered_count,
                self.entries.len(),
                self.selected.map(|i| i + 1).unwrap_or(0)
            )
        } else {
            format!(
                "Items: {} | Selected: {}",
                self.entries.len(),
                self.selected.map(|i| i + 1).unwrap_or(0)
            )
        };

        let status_content = row![
            text(format!("  {} ", mode_text))
                .size(14)
                .style(|theme: &Theme| text::Style {
                    color: Some(theme.extended_palette().primary.strong.color),
                }),
            text(" | ").size(14),
            text(item_text).size(14),
        ]
        .spacing(5);

        let status = if let Some(msg) = &self.message {
            row![
                status_content,
                text(" | ").size(14),
                text(msg)
                    .size(14)
                    .style(|theme: &Theme| text::Style {
                        color: Some(theme.extended_palette().success.strong.color),
                    }),
            ]
            .spacing(5)
        } else if let Some(err) = &self.error {
            row![
                status_content,
                text(" | ").size(14),
                text(err)
                    .size(14)
                    .style(|theme: &Theme| text::Style {
                        color: Some(theme.extended_palette().danger.strong.color),
                    }),
            ]
            .spacing(5)
        } else {
            status_content
        };

        container(status)
            .width(Length::Fill)
            .padding(8)
            .style(|theme: &Theme| container::Style {
                background: Some(theme.extended_palette().background.strong.color.into()),
                ..Default::default()
            })
            .into()
    }

    pub fn theme(&self) -> Theme {
        self.theme.to_iced_theme()
    }
}
