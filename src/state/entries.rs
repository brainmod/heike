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
}
