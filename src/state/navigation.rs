// Navigation state - history and current location
use std::path::PathBuf;

pub struct NavigationState {
    pub current_path: PathBuf,
    pub history: Vec<PathBuf>,
    pub history_index: usize,
    pub pending_selection_path: Option<PathBuf>,
}

impl NavigationState {
    pub fn new(start_path: PathBuf) -> Self {
        Self {
            current_path: start_path.clone(),
            history: vec![start_path],
            history_index: 0,
            pending_selection_path: None,
        }
    }
}
