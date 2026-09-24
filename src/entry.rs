use std::cmp::Ordering;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[derive(Clone, Debug, PartialEq)]
pub enum GitStatus {
    Modified,
    Untracked,
    Ignored,
    Staged,
    Conflict,
}

#[derive(Clone, Debug)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub modified: SystemTime,
    pub extension: String,
    pub permissions: Option<fs::Permissions>,
    pub git_status: Option<GitStatus>,
}

impl FileEntry {
    pub fn from_path(path: PathBuf) -> Option<Self> {
        let symlink_meta = fs::symlink_metadata(&path).ok()?;
        let is_symlink = symlink_meta.is_symlink();

        let name = path.file_name()?.to_string_lossy().to_string();
        let extension = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        let metadata = fs::metadata(&path).ok();
        let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);
        let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
        let modified = metadata
            .as_ref()
            .and_then(|m| m.modified().ok())
            .or_else(|| symlink_meta.modified().ok())
            .unwrap_or(SystemTime::now());
        let permissions = metadata.as_ref().map(|m| m.permissions());

        Some(Self {
            path,
            name,
            is_dir,
            is_symlink,
            size,
            modified,
            extension,
            permissions,
            git_status: None,
        })
    }

    pub fn get_icon(&self) -> &str {
        if self.is_dir {
            return "\u{f07b}";
        }
        match self.extension.as_str() {
            "rs" => "\u{e7a8}",
            "toml" => "\u{e615}",
            "md" => "\u{e73e}",
            "txt" => "\u{f15c}",
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" => "\u{f1c5}",
            "mp4" | "mkv" | "mov" | "avi" | "webm" => "\u{f03d}",
            "mp3" | "wav" | "flac" | "ogg" | "m4a" => "\u{f001}",
            "zip" | "tar" | "gz" | "7z" | "rar" | "xz" | "bz2" => "\u{f410}",
            "py" => "\u{e73c}",
            "pyc" => "\u{e73c}",
            "js" | "mjs" => "\u{e74e}",
            "ts" | "tsx" => "\u{e628}",
            "jsx" => "\u{e7ba}",
            "html" | "htm" => "\u{e736}",
            "css" | "scss" | "sass" => "\u{e749}",
            "json" => "\u{e60b}",
            "yaml" | "yml" => "\u{e615}",
            "xml" => "\u{e619}",
            "pdf" => "\u{f1c1}",
            "doc" | "docx" => "\u{f1c2}",
            "xls" | "xlsx" => "\u{f1c3}",
            "exe" | "msi" => "\u{f17a}",
            "bat" | "cmd" => "\u{e795}",
            "sh" | "bash" | "zsh" => "\u{f489}",
            "c" | "h" => "\u{e61e}",
            "cpp" | "cc" | "cxx" | "hpp" => "\u{e61d}",
            "java" => "\u{e738}",
            "class" | "jar" => "\u{e738}",
            "go" => "\u{e626}",
            "rb" => "\u{e739}",
            "php" => "\u{e73d}",
            "sql" | "db" | "sqlite" => "\u{f1c0}",
            "env" => "\u{f462}",
            "lock" => "\u{f023}",
            "log" => "\u{f18d}",
            "git" | "gitignore" => "\u{e725}",
            _ => "\u{f15b}",
        }
    }

    pub fn display_name(&self) -> String {
        if self.is_symlink {
            format!("{} \u{2192}", self.name)
        } else {
            self.name.clone()
        }
    }

    pub fn get_permissions_string(&self) -> String {
        let Some(perms) = &self.permissions else {
            return "unknown".to_string();
        };

        #[cfg(unix)]
        {
            let mode = perms.mode();
            format!(
                "{}{}{}",
                format_perms((mode >> 6) & 0o7),
                format_perms((mode >> 3) & 0o7),
                format_perms(mode & 0o7)
            )
        }

        #[cfg(not(unix))]
        {
            if perms.readonly() {
                "read-only".to_string()
            } else {
                "read-write".to_string()
            }
        }
    }

    pub fn get_file_type(&self) -> String {
        if self.is_symlink {
            return "Symbolic Link".to_string();
        }

        if self.is_dir {
            return "Directory".to_string();
        }

        match self.extension.as_str() {
            // Archives
            "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "7z" | "rar" => "Archive",
            // Images
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" | "ico" => "Image",
            // Videos
            "mp4" | "mkv" | "mov" | "avi" | "webm" | "flv" | "wmv" => "Video",
            // Audio
            "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "wma" => "Audio",
            // Documents
            "pdf" => "PDF Document",
            "doc" | "docx" => "Word Document",
            "xls" | "xlsx" => "Excel Spreadsheet",
            "ppt" | "pptx" => "PowerPoint Presentation",
            // Code
            "py" | "pyc" => "Python",
            "js" | "mjs" => "JavaScript",
            "ts" | "tsx" => "TypeScript",
            "jsx" => "JSX",
            "rs" => "Rust",
            "c" | "h" => "C",
            "cpp" | "cc" | "cxx" | "hpp" => "C++",
            "java" => "Java",
            "go" => "Go",
            "rb" => "Ruby",
            "php" => "PHP",
            "swift" => "Swift",
            "kt" => "Kotlin",
            // Data Formats
            "json" => "JSON",
            "yaml" | "yml" => "YAML",
            "xml" => "XML",
            "toml" => "TOML",
            "sql" | "db" | "sqlite" => "Database",
            // Markup
            "md" | "markdown" => "Markdown",
            "html" | "htm" => "HTML",
            "css" | "scss" | "sass" | "less" => "Stylesheet",
            // Shell
            "sh" | "bash" | "zsh" | "fish" => "Shell Script",
            "bat" | "cmd" | "ps1" => "Batch/PowerShell Script",
            // Config
            "conf" | "config" | "cfg" | "ini" | "env" => "Configuration",
            "gitignore" | "git" => "Git",
            // Text
            "txt" | "log" => "Text",
            // Executables
            "exe" | "msi" => "Executable",
            // Default
            "" => "File",
            _ => "File",
        }
        .to_string()
    }
}

#[cfg(unix)]
fn format_perms(mode: u32) -> String {
    let r = if mode & 0o4 != 0 { "r" } else { "-" };
    let w = if mode & 0o2 != 0 { "w" } else { "-" };
    let x = if mode & 0o1 != 0 { "x" } else { "-" };
    format!("{}{}{}", r, w, x)
}

/// Case-insensitive natural ordering: "file2" < "File10". Ties fall back to
/// a case-sensitive compare so the order is total and stable.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let mut ai = a.chars().peekable();
    let mut bi = b.chars().peekable();
    loop {
        match (ai.peek().copied(), bi.peek().copied()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(ca), Some(cb)) if ca.is_ascii_digit() && cb.is_ascii_digit() => {
                let na = take_digits(&mut ai);
                let nb = take_digits(&mut bi);
                let ord = na
                    .trim_start_matches('0')
                    .len()
                    .cmp(&nb.trim_start_matches('0').len())
                    .then_with(|| na.trim_start_matches('0').cmp(nb.trim_start_matches('0')))
                    .then_with(|| na.len().cmp(&nb.len()));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(ca), Some(cb)) => {
                let ord = ca.to_lowercase().cmp(cb.to_lowercase());
                if ord != Ordering::Equal {
                    return ord;
                }
                ai.next();
                bi.next();
            }
        }
    }
}

fn take_digits(it: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut s = String::new();
    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
        s.push(c);
        it.next();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted(names: &[&str]) -> Vec<String> {
        let mut v: Vec<String> = names.iter().map(|s| s.to_string()).collect();
        v.sort_by(|a, b| natural_cmp(a, b));
        v
    }

    #[test]
    fn natural_numbers() {
        assert_eq!(
            sorted(&["file10", "file2", "file1"]),
            ["file1", "file2", "file10"]
        );
        assert_eq!(sorted(&["a010", "a9", "a10"]), ["a9", "a10", "a010"]);
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(sorted(&["b", "A", "a", "B"]), ["A", "a", "B", "b"]);
        assert_eq!(sorted(&["Zeta", "alpha"]), ["alpha", "Zeta"]);
    }

    #[test]
    fn prefix_first() {
        assert_eq!(sorted(&["abc", "ab"]), ["ab", "abc"]);
        assert_eq!(natural_cmp("x", "x"), Ordering::Equal);
    }
}
