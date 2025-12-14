# Preview Handler Architecture

## Overview

Heike's preview system uses a modular, trait-based architecture that allows for extensible file previews. Individual preview handlers can be enabled/disabled via configuration, and new handlers can be added without modifying the core preview system.

## Architecture Components

### 1. PreviewHandler Trait

The core abstraction for all preview handlers:

```rust
pub trait PreviewHandler: Send + Sync {
    /// Name of this handler (for configuration and debugging)
    fn name(&self) -> &str;

    /// Check if this handler can preview the given file
    fn can_preview(&self, entry: &FileEntry) -> bool;

    /// Render the preview for the given file
    fn render(
        &self,
        ui: &mut egui::Ui,
        entry: &FileEntry,
        context: &PreviewContext,
    ) -> Result<(), String>;

    /// Priority of this handler (lower = higher priority)
    fn priority(&self) -> i32 {
        100
    }

    /// Whether this handler is enabled by default
    fn enabled_by_default(&self) -> bool {
        true
    }
}
```

### 2. PreviewContext

Shared context passed to all preview handlers containing resources and state:

```rust
pub struct PreviewContext<'a> {
    pub syntax_set: &'a SyntaxSet,
    pub theme_set: &'a ThemeSet,
    pub theme: Theme,
    pub show_hidden: bool,
    pub last_selection_change: Instant,
    pub directory_selections: &'a HashMap<PathBuf, usize>,
    pub next_navigation: &'a std::cell::RefCell<Option<PathBuf>>,
    pub pending_selection: &'a std::cell::RefCell<Option<PathBuf>>,
}
```

### 3. PreviewRegistry

Manages and dispatches preview handlers:

```rust
pub struct PreviewRegistry {
    handlers: Vec<Arc<dyn PreviewHandler>>,
    enabled_handlers: HashSet<String>,
}
```

The registry:
- Stores all registered handlers
- Tracks which handlers are enabled
- Dispatches preview requests to the first matching enabled handler
- Automatically sorts handlers by priority

### 4. Built-in Handlers

Heike includes these built-in preview handlers (in priority order):

1. **DirectoryPreviewHandler** (priority 5) - Directory contents
2. **ImagePreviewHandler** (priority 10) - PNG, JPG, GIF, WebP, BMP, SVG, ICO
3. **MarkdownPreviewHandler** (priority 20) - Markdown files
4. **ArchivePreviewHandler** (priority 30) - ZIP, TAR, GZ, TGZ archives
5. **PdfPreviewHandler** (priority 40) - PDF metadata
6. **OfficePreviewHandler** (priority 50) - DOCX, XLSX files
7. **AudioPreviewHandler** (priority 60) - MP3 metadata
8. **TextPreviewHandler** (priority 90) - Text files with syntax highlighting
9. **BinaryPreviewHandler** (priority 1000) - Fallback for binary files

## Configuration

Users can enable/disable preview handlers in `~/.config/heike/config.toml`:

```toml
[previews]
enabled = [
    "directory",
    "image",
    "markdown",
    "archive",
    "pdf",
    "office",
    "audio",
    "text",
    "binary"
]
```

To disable specific previews (e.g., for performance):

```toml
[previews]
enabled = [
    "directory",
    "image",
    "text",
    "binary"
]
```

## Creating Custom Preview Handlers

### Step 1: Implement PreviewHandler

```rust
use crate::entry::FileEntry;
use crate::view::preview::handler::{PreviewContext, PreviewHandler};
use eframe::egui;

pub struct MyCustomHandler;

impl PreviewHandler for MyCustomHandler {
    fn name(&self) -> &str {
        "my_custom"
    }

    fn can_preview(&self, entry: &FileEntry) -> bool {
        // Check if this handler should handle the file
        entry.extension == "xyz"
    }

    fn render(
        &self,
        ui: &mut egui::Ui,
        entry: &FileEntry,
        context: &PreviewContext,
    ) -> Result<(), String> {
        // Render your custom preview
        ui.label("My custom preview!");
        Ok(())
    }

    fn priority(&self) -> i32 {
        25 // Adjust based on specificity
    }
}
```

### Step 2: Register Handler

In `src/view/preview/mod.rs`:

```rust
pub fn create_default_registry() -> PreviewRegistry {
    let mut registry = PreviewRegistry::new();

    // ... existing handlers ...
    registry.register(Arc::new(MyCustomHandler::new()));

    registry
}
```

### Step 3: Add to Configuration

Update `src/config.rs` to include the handler in the default configuration:

```rust
impl Default for PreviewConfig {
    fn default() -> Self {
        PreviewConfig {
            enabled: vec![
                // ... existing handlers ...
                "my_custom".to_string(),
            ],
        }
    }
}
```

## Handler Priority Guidelines

- **1-10**: Essential handlers (directories, system files)
- **11-50**: Specific file types (images, markdown, archives, PDFs)
- **51-90**: Medium specificity (office docs, audio metadata)
- **91-500**: Generic handlers (text files, syntax highlighting)
- **501-999**: Experimental or niche handlers
- **1000+**: Fallback handlers (binary files, unknown types)

## Future: Plugin System

The current architecture lays the foundation for a full plugin system. Future enhancements could include:

### Dynamic Loading

```rust
// Load preview handlers from external .so/.dll files
impl PreviewRegistry {
    pub fn load_plugin(&mut self, path: &Path) -> Result<(), PluginError> {
        // Use libloading or similar to dynamically load handlers
        let handler = unsafe { load_preview_handler(path)? };
        self.register(handler);
        Ok(())
    }
}
```

### Yazi Plugin Compatibility

Heike could support Yazi's Lua-based preview plugins:

```rust
pub struct YaziLuaPreviewHandler {
    lua_script: String,
    lua_runtime: mlua::Lua,
}

impl PreviewHandler for YaziLuaPreviewHandler {
    fn name(&self) -> &str {
        &self.lua_script
    }

    fn can_preview(&self, entry: &FileEntry) -> bool {
        // Call Lua function to check
        self.lua_runtime.call_function("can_preview", entry)
    }

    fn render(&self, ui, entry, context) -> Result<(), String> {
        // Execute Lua preview script
        let output = self.lua_runtime.call_function("preview", entry)?;
        ui.label(output);
        Ok(())
    }
}
```

### WASM Plugins

For sandboxed, cross-platform plugins:

```rust
pub struct WasmPreviewHandler {
    wasm_module: wasmtime::Module,
}

impl PreviewHandler for WasmPreviewHandler {
    // Similar pattern using wasmtime to execute WASM preview code
}
```

## API Stability

The `PreviewHandler` trait is considered **stable** and follows semantic versioning:

- **Major version changes**: Breaking changes to the trait
- **Minor version changes**: New optional methods with default implementations
- **Patch version changes**: Bug fixes, documentation updates

Current version: **0.8.x** (unstable - subject to change before 1.0)

## Performance Considerations

1. **Handler Priority**: Keep specific handlers at low priority (checked first)
2. **Lazy Loading**: Handlers should defer expensive operations until `render()` is called
3. **Size Limits**: Check `entry.size` before reading large files
4. **Debouncing**: The preview system automatically debounces rapid selection changes
5. **Caching**: Consider using `PreviewCache` for expensive operations

## Example: Full Custom Handler

See `src/view/preview/handlers/` for complete examples of each built-in handler.

## Contributing

To contribute a new preview handler:

1. Create a new file in `src/view/preview/handlers/`
2. Implement the `PreviewHandler` trait
3. Register in `create_default_registry()`
4. Add to default config in `config.rs`
5. Add tests in `tests/preview_handlers.rs`
6. Update documentation in `PREVIEW_ARCHITECTURE.md` (this file)
7. Submit a pull request

## Questions or Issues

For questions about the preview architecture or to propose new handlers:
- Open an issue: https://github.com/brainmod/heike/issues
- Discussions: https://github.com/brainmod/heike/discussions
