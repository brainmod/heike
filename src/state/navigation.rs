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

    pub fn push_history(&mut self, path: PathBuf) {
        // Remove any forward history when navigating to a new path
        self.history.truncate(self.history_index + 1);
        self.history.push(path.clone());
        self.history_index += 1;
        self.current_path = path;
    }

    pub fn go_back(&mut self) -> Option<PathBuf> {
        if self.history_index > 0 {
            self.history_index -= 1;
            self.current_path = self.history[self.history_index].clone();
            Some(self.current_path.clone())
        } else {
            None
        }
    }

    pub fn go_forward(&mut self) -> Option<PathBuf> {
        if self.history_index < self.history.len() - 1 {
            self.history_index += 1;
            self.current_path = self.history[self.history_index].clone();
            Some(self.current_path.clone())
        } else {
            None
        }
    }
}
