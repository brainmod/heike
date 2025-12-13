use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Application configuration
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    pub theme: ThemeConfig,
    pub panel: PanelConfig,
    pub font: FontConfig,
    pub ui: UiConfig,
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

impl Default for Config {
    fn default() -> Self {
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
