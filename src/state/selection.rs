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
}
