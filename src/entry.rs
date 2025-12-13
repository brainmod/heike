use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[derive(Clone, Debug)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub modified: SystemTime,
    pub extension: String,
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

        Some(Self {
            path,
            name,
            is_dir,
            is_symlink,
            size,
            modified,
            extension,
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
        #[cfg(unix)]
        {
            match fs::metadata(&self.path) {
                Ok(metadata) => {
                    let mode = metadata.permissions().mode();
                    let owner = format_perms((mode >> 6) & 0o7);
                    let group = format_perms((mode >> 3) & 0o7);
                    let others = format_perms(mode & 0o7);
                    format!("{}{}{}", owner, group, others)
                }
                Err(_) => "unknown".to_string(),
            }
        }

        #[cfg(not(unix))]
        {
            match fs::metadata(&self.path) {
                Ok(metadata) => {
                    if metadata.permissions().readonly() {
                        "read-only".to_string()
                    } else {
                        "read-write".to_string()
                    }
                }
                Err(_) => "unknown".to_string(),
            }
        }
    }
}

#[cfg(unix)]
fn format_perms(mode: u32) -> String {
    let r = if mode & 0o4 != 0 { "r" } else { "-" };
    let w = if mode & 0o2 != 0 { "w" } else { "-" };
    let x = if mode & 0o1 != 0 { "x" } else { "-" };
    format!("{}{}{}", r, w, x)
}
