use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct SearchResult {
    pub file_path: PathBuf,
    pub file_name: String,
    pub line_number: usize,
    pub line_content: String,
    pub match_start: usize,
    pub match_end: usize,
}

#[derive(Clone, Debug)]
pub struct SearchOptions {
    pub case_sensitive: bool,
    pub use_regex: bool,
    pub search_hidden: bool,
    pub search_pdfs: bool,
    pub search_archives: bool,
    pub max_results: usize,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            use_regex: false,
            search_hidden: false,
            search_pdfs: true,
            search_archives: true,
            max_results: 1000,
        }
    }
}
