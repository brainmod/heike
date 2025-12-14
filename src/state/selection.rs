// Selection state - cursor position and multi-selection tracking
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

pub struct SelectionState {
    pub selected_index: Option<usize>,
    pub multi_selection: HashSet<PathBuf>,
    pub directory_selections: HashMap<PathBuf, usize>,
    pub last_selection_change: Instant,
    pub disable_autoscroll: bool,
    pub last_g_press: Option<Instant>,
}

impl SelectionState {
    pub fn new() -> Self {
        Self {
            // Start with None - selection is set when entries load
            selected_index: None,
            multi_selection: HashSet::new(),
            directory_selections: HashMap::new(),
            last_selection_change: Instant::now(),
            disable_autoscroll: false,
            last_g_press: None,
        }
    }

    pub fn save_selection(&mut self, path: PathBuf) {
        if let Some(idx) = self.selected_index {
            self.directory_selections.insert(path, idx);
        }
    }

    pub fn restore_selection(&mut self, path: &PathBuf) -> Option<usize> {
        self.directory_selections.get(path).copied()
    }

    pub fn clear_multi_selection(&mut self) {
        self.multi_selection.clear();
    }

    pub fn toggle_selection(&mut self, path: PathBuf) {
        if self.multi_selection.contains(&path) {
            self.multi_selection.remove(&path);
        } else {
            self.multi_selection.insert(path);
        }
    }

    pub fn update_selection_time(&mut self) {
        self.last_selection_change = Instant::now();
    }
}
