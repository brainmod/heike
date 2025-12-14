// Tabs state management for multiple directory views
use crate::entry::FileEntry;
use std::collections::HashMap;
use std::path::PathBuf;

/// State for a single tab (directory view)
#[derive(Clone)]
pub struct TabState {
    pub label: String,
    pub current_path: PathBuf,
    pub history: Vec<PathBuf>,
    pub history_index: usize,
    pub all_entries: Vec<FileEntry>,
    pub visible_entries: Vec<FileEntry>,
    pub parent_entries: Vec<FileEntry>,
    pub selected_index: Option<usize>,
    pub directory_selections: HashMap<PathBuf, usize>,
    pub pending_selection_path: Option<PathBuf>,
}

impl TabState {
    pub fn new(path: PathBuf) -> Self {
        let label = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("/")
            .to_string();

        Self {
            label,
            current_path: path.clone(),
            history: vec![path],
            history_index: 0,
            all_entries: Vec::new(),
            visible_entries: Vec::new(),
            parent_entries: Vec::new(),
            selected_index: None,
            directory_selections: HashMap::new(),
            pending_selection_path: None,
        }
    }

    pub fn update_label(&mut self) {
        self.label = self
            .current_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("/")
            .to_string();
    }
}

/// Manages multiple tabs
pub struct TabsManager {
    pub tabs: Vec<TabState>,
    pub active_tab: usize,
}

impl TabsManager {
    pub fn new(initial_path: PathBuf) -> Self {
        Self {
            tabs: vec![TabState::new(initial_path)],
            active_tab: 0,
        }
    }

    pub fn get_active(&self) -> Option<&TabState> {
        self.tabs.get(self.active_tab)
    }

    pub fn get_active_mut(&mut self) -> Option<&mut TabState> {
        self.tabs.get_mut(self.active_tab)
    }

    pub fn new_tab(&mut self, path: PathBuf) {
        self.tabs.push(TabState::new(path));
        self.active_tab = self.tabs.len() - 1;
    }

    pub fn close_tab(&mut self, index: usize) -> bool {
        if self.tabs.len() <= 1 {
            return false; // Can't close the last tab
        }

        self.tabs.remove(index);

        // Adjust active tab index
        if self.active_tab >= index && self.active_tab > 0 {
            self.active_tab -= 1;
        }

        true
    }

    pub fn close_current_tab(&mut self) -> bool {
        self.close_tab(self.active_tab)
    }

    pub fn switch_to_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active_tab = index;
        }
    }

    pub fn next_tab(&mut self) {
        self.active_tab = (self.active_tab + 1) % self.tabs.len();
    }

    pub fn prev_tab(&mut self) {
        if self.active_tab == 0 {
            self.active_tab = self.tabs.len() - 1;
        } else {
            self.active_tab -= 1;
        }
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }
}
