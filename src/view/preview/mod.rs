// Modular preview system for Heike file manager
//
// This module provides an extensible preview system based on the PreviewHandler trait.
// Individual preview handlers can be enabled/disabled via configuration, and new handlers
// can be added without modifying the core preview system.

mod handler;
mod handlers;
mod registry;

pub use handler::PreviewContext;
pub use handlers::*;
pub use registry::PreviewRegistry;

use crate::entry::FileEntry;
use crate::style::{self, Theme};
use chrono::{DateTime, Local};
use eframe::egui;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime};
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

/// Cached preview content with metadata for invalidation
#[derive(Clone)]
pub struct CachedPreview {
    pub content: Arc<str>,
    pub modified_time: SystemTime,
    pub cached_at: Instant,
}

/// Extracts preview content from a file; runs on a background thread
pub type PreviewLoadFn = fn(&FileEntry) -> Result<String, String>;

/// Background loads shared between the UI thread and loader threads
#[derive(Default)]
struct BackgroundLoads {
    in_flight: HashSet<PathBuf>,
    done: HashMap<PathBuf, (SystemTime, Result<String, String>)>,
}

/// Preview cache to avoid re-reading and re-parsing identical files
pub struct PreviewCache {
    cache: HashMap<PathBuf, CachedPreview>,
    errors: HashMap<PathBuf, (SystemTime, String)>,
    loads: Arc<Mutex<BackgroundLoads>>,
    max_entries: usize,
}

impl PreviewCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            errors: HashMap::new(),
            loads: Arc::new(Mutex::new(BackgroundLoads::default())),
            max_entries: 100, // Cache up to 100 file previews
        }
    }

    /// Get cached preview if valid (not modified since caching)
    pub fn get(&self, path: &Path, current_mtime: SystemTime) -> Option<Arc<str>> {
        self.cache
            .get(path)
            .filter(|cached| cached.modified_time == current_mtime)
            .map(|cached| cached.content.clone())
    }

    /// Store preview in cache
    pub fn insert(&mut self, path: PathBuf, content: impl Into<Arc<str>>, mtime: SystemTime) {
        // Simple LRU: remove oldest entry if cache is full
        if self.cache.len() >= self.max_entries {
            if let Some(oldest_key) = self
                .cache
                .iter()
                .min_by_key(|(_, v)| v.cached_at)
                .map(|(k, _)| k.clone())
            {
                self.cache.remove(&oldest_key);
            }
        }

        self.cache.insert(
            path,
            CachedPreview {
                content: content.into(),
                modified_time: mtime,
                cached_at: Instant::now(),
            },
        );
    }

    /// Cached content for `entry`, loading it with `load` on a background thread
    /// on a miss. Returns `None` while the load is in progress (show a spinner);
    /// the UI is repainted when it finishes.
    pub fn load(
        &mut self,
        ctx: &egui::Context,
        entry: &FileEntry,
        load: PreviewLoadFn,
    ) -> Option<Result<Arc<str>, String>> {
        if let Some(content) = self.get(&entry.path, entry.modified) {
            return Some(Ok(content));
        }
        if let Some((mtime, err)) = self.errors.get(&entry.path) {
            if *mtime == entry.modified {
                return Some(Err(err.clone()));
            }
        }

        let mut loads = self.loads.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((mtime, result)) = loads.done.remove(&entry.path) {
            if mtime == entry.modified {
                drop(loads);
                return Some(match result {
                    Ok(content) => {
                        let content: Arc<str> = content.into();
                        self.insert(entry.path.clone(), content.clone(), mtime);
                        Ok(content)
                    }
                    Err(err) => {
                        if self.errors.len() >= self.max_entries {
                            self.errors.clear();
                        }
                        self.errors.insert(entry.path.clone(), (mtime, err.clone()));
                        Err(err)
                    }
                });
            }
            // Stale result (file changed while loading) - fall through and reload
        }

        if loads.in_flight.insert(entry.path.clone()) {
            let entry = entry.clone();
            let loads = Arc::clone(&self.loads);
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let result = load(&entry);
                let mut loads = loads.lock().unwrap_or_else(|e| e.into_inner());
                loads.in_flight.remove(&entry.path);
                loads.done.insert(entry.path, (entry.modified, result));
                ctx.request_repaint();
            });
        }
        None
    }
}

impl Default for PreviewCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a default preview registry with all standard handlers
pub fn create_default_registry() -> PreviewRegistry {
    let mut registry = PreviewRegistry::new();

    // Register all default handlers (ordered by priority)
    registry.register(Arc::new(DirectoryPreviewHandler::new()));
    registry.register(Arc::new(ImagePreviewHandler::new()));
    registry.register(Arc::new(MarkdownPreviewHandler::new()));
    registry.register(Arc::new(ArchivePreviewHandler::new()));
    registry.register(Arc::new(PdfPreviewHandler::new()));
    registry.register(Arc::new(OfficePreviewHandler::new()));
    registry.register(Arc::new(AudioPreviewHandler::new()));
    registry.register(Arc::new(TextPreviewHandler::new()));
    registry.register(Arc::new(BinaryPreviewHandler::new())); // Fallback

    registry
}

/// Render preview pane header with file metadata
pub fn render_preview_header(ui: &mut egui::Ui, entry: &FileEntry) {
    style::truncated_label(
        ui,
        egui::RichText::new(format!("{} {}", entry.get_icon(), entry.display_name())).heading(),
    );
    ui.add_space(5.0);
    ui.label(format!("Type: {}", entry.get_file_type()));
    style::truncated_label(ui, format!("Size: {}", bytesize::ByteSize(entry.size)));
    let datetime: DateTime<Local> = entry.modified.into();
    ui.label(format!("Modified: {}", datetime.format("%Y-%m-%d %H:%M")));
    ui.label(format!("Permissions: {}", entry.get_permissions_string()));
    ui.separator();
}

/// Main preview dispatcher using the handler registry
///
/// This is the public API for rendering file previews.
pub fn render_preview(
    ui: &mut egui::Ui,
    entry: &FileEntry,
    registry: &PreviewRegistry,
    show_hidden: bool,
    last_selection_change: Instant,
    directory_selections: &HashMap<PathBuf, usize>,
    syntax_set: &SyntaxSet,
    theme_set: &ThemeSet,
    theme: Theme,
    next_navigation: &std::cell::RefCell<Option<PathBuf>>,
    pending_selection: &std::cell::RefCell<Option<PathBuf>>,
    preview_cache: &std::cell::RefCell<PreviewCache>,
) {
    // Render file metadata header
    render_preview_header(ui, entry);

    // Debounce for initial file selection change
    if last_selection_change.elapsed() <= std::time::Duration::from_millis(200) {
        ui.centered_and_justified(|ui| {
            ui.spinner();
        });
        return;
    }

    // Create preview context
    let context = PreviewContext {
        syntax_set,
        theme_set,
        theme,
        show_hidden,
        last_selection_change,
        directory_selections,
        next_navigation,
        pending_selection,
        preview_cache,
    };

    // Try to render using registry
    if !registry.render_preview(ui, entry, &context) {
        // No handler found - show fallback message
        ui.centered_and_justified(|ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.label(egui::RichText::new("❓ Unknown File Type").size(18.0));
                ui.add_space(10.0);
                ui.label("No preview handler available");
            });
        });
    }
}
