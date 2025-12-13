// Sort options for file listing

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortBy {
    Name,
    Size,
    Modified,
    Extension,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

#[derive(Clone, Copy, Debug)]
pub struct SortOptions {
    pub sort_by: SortBy,
    pub sort_order: SortOrder,
    pub dirs_first: bool,
}

impl Default for SortOptions {
    fn default() -> Self {
        Self {
            sort_by: SortBy::Name,
            sort_order: SortOrder::Ascending,
            dirs_first: true,
        }
    }
}

impl SortOptions {
    pub fn cycle_sort_by(&mut self) {
        self.sort_by = match self.sort_by {
            SortBy::Name => SortBy::Size,
            SortBy::Size => SortBy::Modified,
            SortBy::Modified => SortBy::Extension,
            SortBy::Extension => SortBy::Name,
        };
    }

    pub fn toggle_order(&mut self) {
        self.sort_order = match self.sort_order {
            SortOrder::Ascending => SortOrder::Descending,
            SortOrder::Descending => SortOrder::Ascending,
        };
    }

    pub fn toggle_dirs_first(&mut self) {
        self.dirs_first = !self.dirs_first;
    }
}
