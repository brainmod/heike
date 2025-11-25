use crate::model::{ConfirmAction, FileEntry, Mode, SearchResult};
use iced::keyboard::{Key, Modifiers};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum Message {
    // Keyboard
    KeyPressed(Key, Modifiers),

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
    Rename(String),
    CreateDirectory(String),
    CreateFile(String),

    // Multi-select
    ToggleMultiSelect,
    ClearMultiSelect,

    // Async results
    DirectoryLoaded(Result<Vec<FileEntry>, String>),
    SearchComplete(Vec<SearchResult>),
    FileWatcherEvent(PathBuf),
    FileOperationComplete(Result<String, String>),
    PreviewLoaded(PreviewContent),

    // UI
    ToggleHidden,
    ToggleTheme,
    OpenFile(PathBuf),
    OpenInSystem(PathBuf),

    // Search navigation
    NextSearchResult,
    PrevSearchResult,

    // Pane interaction
    PaneResized(f32),

    // Error handling
    ShowError(String),
    ShowInfo(String),
}

#[derive(Debug, Clone)]
pub enum PreviewContent {
    Text(String),
    Image(PathBuf),
    Directory(Vec<FileEntry>),
    Loading,
    Error(String),
}
