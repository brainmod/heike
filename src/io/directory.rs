use crate::entry::{natural_cmp, FileEntry, GitStatus};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

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
        natural_cmp(&a.name, &b.name)
    });
    Ok(entries)
}

/// Git status of each immediate child of `dir_path`, keyed by file name.
/// Finds the repo root without spawning, so non-repo directories cost nothing;
/// inside a repo it runs a single `git status`.
pub fn git_statuses(dir_path: &Path) -> HashMap<String, GitStatus> {
    let Ok(dir) = dir_path.canonicalize() else {
        return HashMap::new();
    };
    let Some(root) = dir.ancestors().find(|a| a.join(".git").exists()) else {
        return HashMap::new();
    };
    let mut prefix = dir
        .strip_prefix(root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    if !prefix.is_empty() {
        prefix.push('/');
    }

    let output = match Command::new("git")
        .args([
            "status",
            "--porcelain=v1",
            "-z",
            "--ignored",
            "--untracked-files=normal",
            "--",
            ".",
        ])
        .current_dir(&dir)
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return HashMap::new(),
    };

    parse_porcelain_z(&String::from_utf8_lossy(&output.stdout), &prefix)
}

/// Parse `git status --porcelain=v1 -z` output (paths relative to the repo root)
/// into statuses for the immediate children of the directory at `prefix`.
fn parse_porcelain_z(output: &str, prefix: &str) -> HashMap<String, GitStatus> {
    let mut statuses = HashMap::new();
    let mut records = output.split('\0');

    while let Some(record) = records.next() {
        if record.len() < 4 || !record.is_char_boundary(3) {
            continue;
        }
        let status_code = &record[..2];
        // Renames/copies are followed by a record holding the original path
        if status_code.contains('R') || status_code.contains('C') {
            records.next();
        }

        let Some(local_path) = record[3..].strip_prefix(prefix) else {
            continue;
        };
        // Immediate child name in this directory
        let component = local_path.split('/').next().unwrap_or(local_path);
        if component.is_empty() {
            continue;
        }

        let status = match status_code {
            "??" => GitStatus::Untracked,
            "!!" => GitStatus::Ignored,
            s if s.contains('U') => GitStatus::Conflict,
            s if s.contains('M') || s.contains('D') || s.contains('T') => GitStatus::Modified,
            s if s.contains('A') || s.contains('R') || s.contains('C') => GitStatus::Staged,
            _ => continue,
        };

        statuses
            .entry(component.to_string())
            .and_modify(|e| *e = prioritize_status(e, &status))
            .or_insert(status);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_porcelain_z_for_immediate_children() {
        let out = "?? src/new.rs\0 M src/app.rs\0R  src/b.rs\0src/a.rs\0!! target/\0 M src/io/x.rs\0M  README.md\0";
        let s = parse_porcelain_z(out, "src/");
        assert_eq!(s.get("new.rs"), Some(&GitStatus::Untracked));
        assert_eq!(s.get("app.rs"), Some(&GitStatus::Modified));
        assert_eq!(s.get("b.rs"), Some(&GitStatus::Staged));
        assert_eq!(s.get("a.rs"), None); // rename source is skipped
        assert_eq!(s.get("io"), Some(&GitStatus::Modified));
        assert_eq!(s.len(), 4);

        let root = parse_porcelain_z(out, "");
        assert_eq!(root.get("src"), Some(&GitStatus::Modified));
        assert_eq!(root.get("target"), Some(&GitStatus::Ignored));
        assert_eq!(root.get("README.md"), Some(&GitStatus::Modified));
    }

    #[test]
    fn handles_spaces_in_names() {
        let s = parse_porcelain_z("?? my file.txt\0", "");
        assert_eq!(s.get("my file.txt"), Some(&GitStatus::Untracked));
    }
}
