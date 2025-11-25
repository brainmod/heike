use std::path::PathBuf;

#[derive(Clone, Debug, Default)]
pub struct Clipboard {
    pub paths: Vec<PathBuf>,
    pub operation: Option<ClipboardOp>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ClipboardOp {
    Copy,
    Cut,
}

impl Clipboard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    pub fn clear(&mut self) {
        self.paths.clear();
        self.operation = None;
    }

    pub fn set_copy(&mut self, paths: Vec<PathBuf>) {
        self.paths = paths;
        self.operation = Some(ClipboardOp::Copy);
    }

    pub fn set_cut(&mut self, paths: Vec<PathBuf>) {
        self.paths = paths;
        self.operation = Some(ClipboardOp::Cut);
    }

    pub fn is_cut(&self) -> bool {
        matches!(self.operation, Some(ClipboardOp::Cut))
    }
}
