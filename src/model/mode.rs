use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    Normal,
    #[allow(dead_code)]
    Visual,
    Filter,
    Command,
    Rename,
    Search,
    #[allow(dead_code)]
    SearchResults(Vec<SearchResult>),
    Confirm(ConfirmAction),
    GPrefix, // For 'gg' sequence
}

impl Default for Mode {
    fn default() -> Self {
        Self::Normal
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchResult {
    pub file_path: PathBuf,
    pub file_name: String,
    pub line_number: usize,
    pub line_content: String,
    pub match_start: usize,
    pub match_end: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ConfirmAction {
    Delete,
    #[allow(dead_code)]
    Overwrite,
}
