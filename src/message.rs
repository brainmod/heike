use crate::model::{FileEntry, Mode};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum Message {
    // Keyboard
    KeyPressed(iced::keyboard::Key, iced::keyboard::Modifiers),

    // Navigation
    Navigate(PathBuf),
    NavigateUp,
    NavigateBack,
    NavigateForward,
    Select(usize),
    SelectDelta(i32), // +1/-1 for j/k
    SelectFirst,
    SelectLast,

    // Modes
    SetMode(Mode),
    InputChanged(String),
    InputSubmit,
    CancelInput,

    // File operations
    Yank,
    Cut,
    Paste,
    Delete,
    ConfirmDelete,
    #[allow(dead_code)]
    Rename(String),
    #[allow(dead_code)]
    CreateDirectory(String),
    #[allow(dead_code)]
    CreateFile(String),

    // Multi-select
    ToggleMultiSelect,
    ClearMultiSelect,

    // Async results
    DirectoryLoaded(Result<Vec<FileEntry>, String>),
    ParentDirectoryLoaded(Result<Vec<FileEntry>, String>),
    PreviewDirectoryLoaded(Result<Vec<FileEntry>, String>),
    #[allow(dead_code)]
    SearchComplete(Vec<crate::model::SearchResult>),
    FileWatcherEvent(PathBuf),
    FileOperationComplete(Result<String, String>),
    PreviewLoaded(Result<PreviewContent, String>),

    // UI
    ToggleHidden,
    #[allow(dead_code)]
    ToggleTheme,
    #[allow(dead_code)]
    OpenFile(PathBuf),
    OpenInSystem(PathBuf),

    // Search navigation
    NextSearchResult,
    PrevSearchResult,

    // Pane interaction
    #[allow(dead_code)]
    PaneResized(f32),

    // Error handling
    ShowError(String),
    ShowInfo(String),
    FontLoaded(Result<iced::font::Family, iced::font::Error>),
}

#[derive(Debug, Clone)]
pub enum PreviewContent {
    Text(String),
    #[allow(dead_code)]
    Image(PathBuf),
    #[allow(dead_code)]
    Directory(Vec<FileEntry>),
    Loading,
    Error(String),
}

