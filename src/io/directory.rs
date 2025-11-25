use crate::model::FileEntry;
use std::fs;
use std::path::PathBuf;

pub async fn load_directory(path: PathBuf, show_hidden: bool) -> Result<Vec<FileEntry>, String> {
    tokio::task::spawn_blocking(move || read_directory(&path, show_hidden))
        .await
        .map_err(|e| e.to_string())?
}

fn read_directory(path: &PathBuf, show_hidden: bool) -> Result<Vec<FileEntry>, String> {
    let entries = fs::read_dir(path).map_err(|e| format!("Failed to read directory: {}", e))?;

    let mut file_entries: Vec<FileEntry> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();

            // Filter hidden files if needed
            if !show_hidden {
                if let Some(name) = path.file_name() {
                    if name.to_string_lossy().starts_with('.') {
                        return None;
                    }
                }
            }

            FileEntry::from_path(path)
        })
        .collect();

    // Sort: directories first, then by name
    file_entries.sort_by(|a, b| {
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });

    Ok(file_entries)
}
