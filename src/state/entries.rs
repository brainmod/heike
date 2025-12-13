// Entry state - holds file entries for different panes
use crate::entry::FileEntry;

pub struct EntryState {
    pub all_entries: Vec<FileEntry>,
    pub visible_entries: Vec<FileEntry>,
    pub parent_entries: Vec<FileEntry>,
}

impl EntryState {
    pub fn new() -> Self {
        Self {
            all_entries: Vec::new(),
            visible_entries: Vec::new(),
            parent_entries: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.all_entries.clear();
        self.visible_entries.clear();
        self.parent_entries.clear();
    }

    pub fn set_all_entries(&mut self, entries: Vec<FileEntry>) {
        self.all_entries = entries;
    }

    pub fn set_visible_entries(&mut self, entries: Vec<FileEntry>) {
        self.visible_entries = entries;
    }

    pub fn set_parent_entries(&mut self, entries: Vec<FileEntry>) {
        self.parent_entries = entries;
    }
}
