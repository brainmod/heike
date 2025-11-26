use crate::entry::FileEntry;
use std::fs;
use std::path::Path;

pub fn read_directory(path: &Path, show_hidden: bool) -> Result<Vec<FileEntry>, std::io::Error> {
    let mut entries = Vec::new();
    let read_dir = fs::read_dir(path)?;
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !show_hidden {
            if let Some(name) = path.file_name() {
                if name.to_string_lossy().starts_with('.') {
                    continue;
                }
            }
        }
        if let Some(file_entry) = FileEntry::from_path(path) {
            entries.push(file_entry);
        }
    }
    entries.sort_by(|a, b| {
        if a.is_dir != b.is_dir {
            return b.is_dir.cmp(&a.is_dir);
        }
        a.name.to_lowercase().cmp(&b.name.to_lowercase())
    });
    Ok(entries)
}

pub fn fuzzy_match(text: &str, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let mut q_chars = query.chars();
    let mut q_char = match q_chars.next() {
        Some(c) => c,
        None => return true,
    };
    for t_char in text.chars() {
        if t_char.eq_ignore_ascii_case(&q_char) {
            q_char = match q_chars.next() {
                Some(c) => c,
                None => return true,
            };
        }
    }
    false
}

pub fn is_likely_binary(path: &Path) -> bool {
    let mut buf = [0u8; 8192];
    if let Ok(mut f) = fs::File::open(path) {
        if let Ok(n) = std::io::Read::read(&mut f, &mut buf) {
            if n == 0 {
                return false;
            }
            let null_count = buf[..n].iter().filter(|&&b| b == 0).count();
            return null_count > (n / 100).max(1);
        }
    }
    false
}
