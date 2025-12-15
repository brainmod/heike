use crate::entry::{FileEntry, GitStatus};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn read_directory(path: &Path, show_hidden: bool) -> Result<Vec<FileEntry>, std::io::Error> {
    let mut entries = Vec::new();
    let read_dir = fs::read_dir(path)?;

    // Fetch git statuses for the directory
    let git_statuses = get_git_statuses(path);

    for entry in read_dir.flatten() {
        let path = entry.path();
        if !show_hidden {
            if let Some(name) = path.file_name() {
                if name.to_string_lossy().starts_with('.') {
                    continue;
                }
            }
        }
        if let Some(mut file_entry) = FileEntry::from_path(path) {
            if let Some(status) = git_statuses.get(&file_entry.name) {
                file_entry.git_status = Some(status.clone());
            }
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

fn get_git_statuses(dir_path: &Path) -> HashMap<String, GitStatus> {
    let mut statuses = HashMap::new();

    // 1. Get prefix (relative path of current dir from repo root)
    let prefix = match Command::new("git")
        .arg("rev-parse")
        .arg("--show-prefix")
        .current_dir(dir_path)
        .output()
    {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        _ => return statuses, // Not a git repo or git not found
    };

    // 2. Get status of files in current dir (and subdirs)
    let output = match Command::new("git")
        .arg("status")
        .arg("--porcelain")
        .arg("--ignored")
        .arg(".")
        .current_dir(dir_path)
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return statuses,
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.len() < 4 {
            continue;
        }
        let status_code = &line[..2];
        let raw_path = line[3..].trim();
        // Handle basic quoting
        let raw_path = raw_path.trim_matches('"');

        if let Some(local_path) = raw_path.strip_prefix(&prefix) {
            if local_path.is_empty() {
                continue;
            }

            // Get the immediate child name in current dir
            let component = local_path.split('/').next().unwrap_or(local_path);

            let status = match status_code {
                "??" => GitStatus::Untracked,
                "!!" => GitStatus::Ignored,
                s if s.contains('U') => GitStatus::Conflict,
                s if s.contains('M') => GitStatus::Modified,
                s if s.contains('A') => GitStatus::Staged,
                s if s.contains('D') => GitStatus::Modified,
                _ => continue,
            };

            statuses
                .entry(component.to_string())
                .and_modify(|e| *e = prioritize_status(e, &status))
                .or_insert(status);
        }
    }

    statuses
}

fn prioritize_status(current: &GitStatus, new: &GitStatus) -> GitStatus {
    use GitStatus::*;
    match (current, new) {
        (Conflict, _) => Conflict,
        (_, Conflict) => Conflict,
        (Modified, _) => Modified,
        (_, Modified) => Modified,
        (Staged, _) => Staged,
        (_, Staged) => Staged,
        (Untracked, _) => Untracked,
        (_, Untracked) => Untracked,
        (Ignored, _) => Ignored,
    }
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
