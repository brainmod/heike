use std::fs;
use std::path::{Path, PathBuf};

pub async fn copy_files(sources: Vec<PathBuf>, dest_dir: PathBuf) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        for src in &sources {
            let file_name = src
                .file_name()
                .ok_or("Invalid source file")?
                .to_string_lossy();
            let dest = dest_dir.join(file_name.as_ref());

            if src.is_dir() {
                copy_dir_recursive(src, &dest)?;
            } else {
                fs::copy(src, &dest).map_err(|e| format!("Copy failed: {}", e))?;
            }
        }
        Ok(format!("Copied {} item(s)", sources.len()))
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn move_files(sources: Vec<PathBuf>, dest_dir: PathBuf) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        for src in &sources {
            let file_name = src
                .file_name()
                .ok_or("Invalid source file")?
                .to_string_lossy();
            let dest = dest_dir.join(file_name.as_ref());

            fs::rename(src, &dest).map_err(|e| format!("Move failed: {}", e))?;
        }
        Ok(format!("Moved {} item(s)", sources.len()))
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn delete_files(paths: Vec<PathBuf>) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        for path in &paths {
            if path.is_dir() {
                fs::remove_dir_all(path).map_err(|e| format!("Delete failed: {}", e))?;
            } else {
                fs::remove_file(path).map_err(|e| format!("Delete failed: {}", e))?;
            }
        }
        Ok(format!("Deleted {} item(s)", paths.len()))
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn rename_file(old_path: PathBuf, new_name: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let parent = old_path
            .parent()
            .ok_or("Cannot get parent directory")?
            .to_path_buf();
        let new_path = parent.join(&new_name);

        fs::rename(&old_path, &new_path).map_err(|e| format!("Rename failed: {}", e))?;
        Ok(format!("Renamed to {}", new_name))
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn create_directory(path: PathBuf, name: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let new_dir = path.join(&name);
        fs::create_dir(&new_dir).map_err(|e| format!("Create directory failed: {}", e))?;
        Ok(format!("Created directory: {}", name))
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn create_file(path: PathBuf, name: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let new_file = path.join(&name);
        fs::File::create(&new_file).map_err(|e| format!("Create file failed: {}", e))?;
        Ok(format!("Created file: {}", name))
    })
    .await
    .map_err(|e| e.to_string())?
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| format!("Failed to create directory: {}", e))?;

    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if path.is_dir() {
            copy_dir_recursive(&path, &dest_path)?;
        } else {
            fs::copy(&path, &dest_path).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}
