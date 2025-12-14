use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Application configuration
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    pub theme: ThemeConfig,
    pub panel: PanelConfig,
    pub font: FontConfig,
    pub ui: UiConfig,
    #[serde(default)]
    pub bookmarks: BookmarksConfig,
    #[serde(default)]
    pub previews: PreviewConfig,
}

/// Theme configuration
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ThemeConfig {
    /// "dark" or "light"
    pub mode: String,
}

/// Panel layout configuration
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PanelConfig {
    /// Width of parent directory pane (in pixels)
    pub parent_width: f32,
    /// Width of preview pane (in pixels)
    pub preview_width: f32,
}

/// Font and text rendering configuration
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FontConfig {
    /// Size of the main interface font (in points)
    pub font_size: f32,
    /// Size of icons (in points)
    pub icon_size: f32,
}

/// UI behavior configuration
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UiConfig {
    /// Show hidden files by default
    pub show_hidden: bool,
    /// Default sort field: "name", "size", "modified", "extension"
    pub sort_by: String,
    /// Sort order: "asc" or "desc"
    pub sort_order: String,
    /// Show directories first in sorting
    pub dirs_first: bool,
}

/// Bookmarks configuration - map of single character to directory path
/// Example: {"d" = "~/Downloads", "h" = "~", "p" = "~/Projects"}
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct BookmarksConfig {
    pub shortcuts: HashMap<String, String>,
}

/// Preview configuration - control which preview handlers are enabled
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PreviewConfig {
    /// List of enabled preview handlers
    /// Available: "directory", "image", "markdown", "archive", "pdf", "office", "audio", "text", "binary"
    pub enabled: Vec<String>,
}

impl Default for PreviewConfig {
    fn default() -> Self {
        PreviewConfig {
            enabled: vec![
                "directory".to_string(),
                "image".to_string(),
                "markdown".to_string(),
                "archive".to_string(),
                "pdf".to_string(),
                "office".to_string(),
                "audio".to_string(),
                "text".to_string(),
                "binary".to_string(),
            ],
        }
    }
}

impl BookmarksConfig {
    /// Resolve a bookmark path, expanding ~ to home directory
    pub fn resolve_path(&self, key: &str) -> Option<PathBuf> {
        self.shortcuts.get(key).map(|path_str| {
            if path_str.starts_with('~') {
                if let Some(home_dir) = directories::UserDirs::new().map(|ud| ud.home_dir().to_path_buf()) {
                    let rest = &path_str[1..];
                    home_dir.join(rest)
                } else {
                    PathBuf::from(path_str)
                }
            } else {
                PathBuf::from(path_str)
            }
        })
    }

    /// Get all available bookmark keys
    pub fn keys(&self) -> Vec<String> {
        self.shortcuts.keys().cloned().collect()
    }
}

impl Default for Config {
    fn default() -> Self {
        let mut shortcuts = HashMap::new();
        // Add default bookmarks
        shortcuts.insert("h".to_string(), "~".to_string());
        shortcuts.insert("r".to_string(), "/".to_string());
        shortcuts.insert("d".to_string(), "~/Downloads".to_string());
        shortcuts.insert("p".to_string(), "~/Projects".to_string());
        shortcuts.insert("t".to_string(), "/tmp".to_string());

        Config {
            theme: ThemeConfig {
                mode: "dark".to_string(),
            },
            panel: PanelConfig {
                parent_width: 200.0,
                preview_width: 350.0,
            },
            font: FontConfig {
                font_size: 12.0,
                icon_size: 14.0,
            },
            ui: UiConfig {
                show_hidden: false,
                sort_by: "name".to_string(),
                sort_order: "asc".to_string(),
                dirs_first: true,
            },
            bookmarks: BookmarksConfig { shortcuts },
            previews: PreviewConfig::default(),
        }
    }
}

impl Config {
    /// Get the path to the config file
    pub fn config_path() -> Option<PathBuf> {
        // Use directories crate to find config directory
        if let Some(proj_dirs) = directories::ProjectDirs::from("", "", "heike") {
            let config_dir = proj_dirs.config_dir();
            return Some(config_dir.join("config.toml"));
        }
        None
    }

    /// Load configuration from file, or return defaults if file doesn't exist
    pub fn load() -> Self {
        if let Some(path) = Self::config_path() {
            if path.exists() {
                match fs::read_to_string(&path) {
                    Ok(contents) => {
                        match toml::from_str::<Config>(&contents) {
                            Ok(config) => return config,
                            Err(e) => {
                                eprintln!("Failed to parse config file: {}", e);
                                eprintln!("Using default configuration");
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to read config file: {}", e);
                        eprintln!("Using default configuration");
                    }
                }
            }
        }
        Config::default()
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(path) = Self::config_path() {
            // Create config directory if it doesn't exist
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }

            let contents = toml::to_string_pretty(self)?;
            fs::write(&path, contents)?;
            return Ok(());
        }

        Err("Could not determine config directory".into())
    }

    /// Create a default config file if it doesn't exist
    pub fn create_default() -> Result<(), Box<dyn std::error::Error>> {
        if let Some(path) = Self::config_path() {
            if !path.exists() {
                let config = Config::default();
                config.save()?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.theme.mode, "dark");
        assert_eq!(config.panel.parent_width, 200.0);
        assert_eq!(config.panel.preview_width, 350.0);
        assert_eq!(config.font.font_size, 12.0);
        assert_eq!(config.font.icon_size, 14.0);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).expect("Failed to serialize");
        let deserialized: Config = toml::from_str(&toml_str).expect("Failed to deserialize");
        assert_eq!(config.theme.mode, deserialized.theme.mode);
    }
}
