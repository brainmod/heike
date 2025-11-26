use super::search::SearchResult;

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
}
