use super::search::SearchResult;
use std::path::PathBuf;

#[derive(Debug, PartialEq, Clone)]
pub enum AppMode {
    Normal,
    Visual,
    Filtering,
    Command,
    Help,
    Rename,
    DeleteConfirm,
    SearchInput,
    SearchResults {
        query: String,
        results: Vec<SearchResult>,
        selected_index: usize,
    },
    BulkRename {
        // Original paths and names for the bulk rename operation
        original_paths: Vec<PathBuf>,
        // Editable text buffer with one filename per line
        edit_buffer: String,
        // Cursor position in the text editor
        cursor_line: usize,
    },
}
