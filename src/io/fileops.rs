use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Reject names that would escape the parent directory or are otherwise invalid
pub fn validate_file_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Empty filename not allowed".into());
    }
    if name == "." || name == ".." {
        return Err(format!("Invalid filename: {}", name));
    }
    if name.contains('/') || name.contains('\\') || name.contains('\0') {
        return Err(format!("Filename cannot contain path separators: {}", name));
    }
    Ok(())
}

/// Returns `path` if free, otherwise the first free `name (N).ext` sibling
pub fn unique_destination(path: &Path) -> PathBuf {
    if fs::symlink_metadata(path).is_err() {
        return path.to_path_buf();
    }
    let parent = path.parent().unwrap_or(Path::new(""));
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    (1..)
        .map(|n| parent.join(format!("{} ({}){}", stem, n, ext)))
        .find(|p| fs::symlink_metadata(p).is_err())
        .expect("unbounded range always yields a free name")
}

/// True if `dest` is `src` or lies inside it (copying/moving a dir into itself)
pub fn is_inside(src: &Path, dest: &Path) -> bool {
    let src = src.canonicalize().unwrap_or_else(|_| src.to_path_buf());
    let dest_parent = dest
        .parent()
        .and_then(|p| p.canonicalize().ok())
        .unwrap_or_else(|| dest.to_path_buf());
    let dest = match dest.file_name() {
        Some(name) => dest_parent.join(name),
        None => dest_parent,
    };
    dest.starts_with(&src)
}

/// Copy a file, symlink or directory tree. `dest` must not exist.
pub fn copy_recursive(src: &Path, dest: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(src)?;
    if meta.file_type().is_symlink() {
        copy_symlink(src, dest)
    } else if meta.is_dir() {
        fs::create_dir(dest)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            copy_recursive(&entry.path(), &dest.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        fs::copy(src, dest).map(|_| ())
    }
}

#[cfg(unix)]
fn copy_symlink(src: &Path, dest: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(fs::read_link(src)?, dest)
}

#[cfg(not(unix))]
fn copy_symlink(src: &Path, dest: &Path) -> io::Result<()> {
    fs::copy(src, dest).map(|_| ())
}

/// Move `src` to `dest`, falling back to copy + delete across filesystems.
/// `dest` must not exist.
pub fn move_path(src: &Path, dest: &Path) -> io::Result<()> {
    match fs::rename(src, dest) {
        Err(e) if e.kind() == io::ErrorKind::CrossesDevices => {
            copy_recursive(src, dest)?;
            if fs::symlink_metadata(src)?.is_dir() {
                fs::remove_dir_all(src)
            } else {
                fs::remove_file(src)
            }
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("heike_fileops_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn rejects_traversal_names() {
        assert!(validate_file_name("ok.txt").is_ok());
        assert!(validate_file_name("").is_err());
        assert!(validate_file_name("..").is_err());
        assert!(validate_file_name("../x").is_err());
        assert!(validate_file_name("a/b").is_err());
    }

    #[test]
    fn unique_destination_never_overwrites() {
        let dir = temp_dir("unique");
        let file = dir.join("a.txt");
        assert_eq!(unique_destination(&file), file);
        fs::write(&file, "x").unwrap();
        assert_eq!(unique_destination(&file), dir.join("a (1).txt"));
        fs::write(dir.join("a (1).txt"), "x").unwrap();
        assert_eq!(unique_destination(&file), dir.join("a (2).txt"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn copy_recursive_copies_tree() {
        let dir = temp_dir("copy");
        fs::create_dir_all(dir.join("src/sub")).unwrap();
        fs::write(dir.join("src/sub/f.txt"), "hello").unwrap();
        copy_recursive(&dir.join("src"), &dir.join("dst")).unwrap();
        assert_eq!(
            fs::read_to_string(dir.join("dst/sub/f.txt")).unwrap(),
            "hello"
        );
        assert!(dir.join("src/sub/f.txt").exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn detects_dir_inside_itself() {
        let dir = temp_dir("inside");
        fs::create_dir_all(dir.join("a")).unwrap();
        assert!(is_inside(&dir.join("a"), &dir.join("a/a")));
        assert!(!is_inside(&dir.join("a"), &dir.join("b")));
        fs::remove_dir_all(dir).unwrap();
    }
}
