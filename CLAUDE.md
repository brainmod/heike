# Heike: AI Assistant Development Guide

## Project Overview

**Heike** is a GUI file manager built with Rust and egui, inspired by the terminal file manager Yazi. Named after the *Heikegani* (平家蟹), a Japanese crab with a shell pattern resembling a samurai face, Heike combines the speed and keyboard-centric efficiency of a TUI with rich media capabilities of a modern GUI.

**Current Version:** 0.8.2 (The "Stability & Tabs" Update)
**Status:** Active Prototype
**Repository:** https://github.com/brainmod/heike

---

## Architecture Overview

### Current Structure

```
heike/
├── src/
│   ├── main.rs             # Entry point (67 lines)
│   ├── app.rs              # Heike struct, update loop (~1600 lines)
│   ├── entry.rs            # FileEntry struct (99 lines)
│   ├── config.rs           # Configuration system (TOML)
│   ├── input.rs            # Keyboard handling (extracted)
│   ├── style.rs            # Theme, layout constants (76 lines)
│   ├── state/
│   │   ├── mod.rs          # State module exports
│   │   ├── mode.rs         # AppMode enum
│   │   ├── clipboard.rs    # ClipboardOp enum
│   │   ├── search.rs       # SearchResult, SearchOptions
│   │   ├── tabs.rs         # TabsManager, TabState
│   │   ├── navigation.rs   # NavigationState
│   │   ├── selection.rs    # SelectionState
│   │   ├── entry.rs        # EntryState
│   │   └── ui.rs           # UIState
│   ├── io/
│   │   ├── mod.rs          # IO module exports
│   │   ├── directory.rs    # Directory reading (62 lines)
│   │   ├── search.rs       # Content search (361 lines)
│   │   └── worker.rs       # Async worker thread (78 lines)
│   └── view/
│       ├── mod.rs          # View module exports
│       ├── panels.rs       # Miller columns rendering (322 lines)
│       ├── modals.rs       # Dialogs/popups (extracted)
│       └── preview/
│           ├── mod.rs      # Preview system core
│           ├── handler.rs  # PreviewHandler trait
│           ├── registry.rs # Handler registry
│           └── handlers/   # Individual preview handlers
│               ├── text.rs
│               ├── markdown.rs
│               ├── image.rs
│               ├── directory.rs
│               ├── archive.rs
│               ├── pdf.rs
│               ├── office.rs
│               ├── audio.rs
│               └── binary.rs
├── assets/
│   ├── heike_icon.png
│   └── JetBrainsMonoNerdFont-Regular.ttf
├── examples/
│   └── convert_icon.rs     # Icon conversion utility
├── Cargo.toml              # Dependencies and metadata
├── README.md               # User-facing documentation
├── CLAUDE.md               # This file (AI assistant guide)
└── FIXES.md                # Detailed fix recommendations
```

### Recent Refactoring Progress

✅ **Completed:**
- Split monolithic main.rs into modules
- Extracted state structs (NavigationState, SelectionState, EntryState, UIState, ModeState)
- Extracted TabsManager for multi-tab support
- Extracted view/panels.rs for Miller columns rendering
- Extracted view/modals.rs for dialogs
- Modularized preview system with handler trait and registry
- Created configuration system with TOML support

---

## Key Components

### Core Data Structures

#### `Heike` struct (src/app.rs)
The main application state container. Key fields:

```rust
pub struct Heike {
    // --- Navigation State ---
    pub current_path: PathBuf,
    pub history: Vec<PathBuf>,
    pub history_index: usize,
    pub all_entries: Vec<FileEntry>,
    pub visible_entries: Vec<FileEntry>,
    pub parent_entries: Vec<FileEntry>,
    pub selected_index: Option<usize>,
    pub multi_selection: HashSet<PathBuf>,
    pub directory_selections: HashMap<PathBuf, usize>,
    pub pending_selection_path: Option<PathBuf>,

    // --- UI State ---
    pub mode: AppMode,
    pub command_buffer: String,
    pub focus_input: bool,
    pub theme: Theme,
    pub show_hidden: bool,

    // --- Layout State ---
    pub panel_widths: [f32; 2],  // [parent, preview]
    pub dragging_divider: Option<usize>,
    pub last_screen_size: egui::Vec2,
    pub disable_autoscroll: bool,

    // --- Clipboard & Operations ---
    pub clipboard: HashSet<PathBuf>,
    pub clipboard_op: Option<ClipboardOp>,

    // --- Search State ---
    pub search_query: String,
    pub search_options: SearchOptions,
    pub search_in_progress: bool,
    pub search_file_count: usize,

    // --- Async I/O ---
    pub command_tx: Sender<IoCommand>,
    pub result_rx: Receiver<IoResult>,
    pub is_loading: bool,
    pub watcher: Option<Box<dyn Watcher>>,
    pub watcher_rx: Receiver<Result<Event, notify::Error>>,
    pub watched_path: Option<PathBuf>,

    // --- Syntax Highlighting ---
    pub syntax_set: SyntaxSet,
    pub theme_set: ThemeSet,

    // --- Messages & Feedback ---
    pub error_message: Option<(String, Instant)>,
    pub info_message: Option<(String, Instant)>,

    // --- Timing ---
    pub last_g_press: Option<Instant>,
    pub last_selection_change: Instant,
}
```

#### `FileEntry` struct (src/entry.rs)
Represents a file or directory with metadata:

```rust
struct FileEntry {
    path: PathBuf,
    name: String,
    is_dir: bool,
    is_symlink: bool,
    size: u64,
    modified: SystemTime,
    extension: String,
}
```

**Key methods:**
- `from_path(PathBuf) -> Option<Self>` - Safe construction from path
- `get_icon() -> &str` - Returns Nerd Font icon glyph
- `display_name() -> String` - Adds arrow indicator for symlinks

#### `AppMode` enum (src/state/mode.rs)
Application modal state machine:

```rust
pub enum AppMode {
    Normal,
    Visual,
    Filtering,
    Command,
    Help,
    Rename,
    DeleteConfirm,
    SearchInput,
    SearchResults {
        query: String,
        results: Vec<SearchResult>,
        selected_index: usize,
    },
}
```

#### `IoCommand` and `IoResult` enums (src/io/worker.rs)
Async communication with worker thread:

```rust
pub enum IoCommand {
    LoadDirectory(PathBuf, bool),         // (path, show_hidden)
    LoadParent(PathBuf, bool),            // (path, show_hidden)
    SearchContent {
        query: String,
        root_path: PathBuf,
        options: SearchOptions,
    },
}

pub enum IoResult {
    DirectoryLoaded { path: PathBuf, entries: Vec<FileEntry> },
    ParentLoaded(Vec<FileEntry>),
    SearchCompleted(Vec<SearchResult>),
    SearchProgress(usize),
    Error(String),
}
```

---

## Critical Code Conventions

### 1. Async Directory Loading Pattern

**ALWAYS verify path matches before applying results:**

```rust
// ✅ CORRECT
if let Ok(result) = self.io_receiver.try_recv() {
    match result {
        IoResult::DirectoryLoaded { path, entries } => {
            // Race condition guard!
            if path == self.current_path {
                self.entries = entries;
                self.apply_filter();
                self.loading = false;
            }
        }
    }
}

// ❌ WRONG - Can apply stale results
if let Ok(result) = self.io_receiver.try_recv() {
    match result {
        IoResult::DirectoryLoaded { path, entries } => {
            self.entries = entries;  // Path not verified!
        }
    }
}
```

### 2. Selection Validation

**ALWAYS validate selection bounds after operations that modify entries:**

```rust
fn validate_selection(&mut self) {
    if let Some(idx) = self.selected_index {
        if self.visible_entries.is_empty() {
            self.selected_index = None;
        } else if idx >= self.visible_entries.len() {
            self.selected_index = Some(self.visible_entries.len() - 1);
        }
    }
}

// Call after: filter, delete, paste, directory load, etc.
```

### 3. Layout Constants

**ALWAYS use constants from `src/style.rs`:**

```rust
use style::*;

// ✅ CORRECT
let icon_size = ICON_SIZE;
let max_preview = MAX_PREVIEW_SIZE;

// ❌ WRONG
let icon_size = 14.0;  // Magic number!
```

### 4. Clipboard Validation

**ALWAYS validate paths exist before operations:**

```rust
// ✅ CORRECT - Clean stale entries
self.clipboard.retain(|entry| entry.path.exists());
if self.clipboard.is_empty() {
    self.info_message = Some(("Clipboard is empty".into(), Instant::now()));
    return;
}

// ❌ WRONG - Don't assume paths still exist
for entry in &self.clipboard {
    fs::copy(&entry.path, &dest)?;  // May fail!
}
```

### 5. Message Auto-Dismiss

**ALWAYS set timestamp for messages:**

```rust
// ✅ CORRECT
self.error_message = Some((format!("Error: {}", err), Instant::now()));
self.info_message = Some(("File copied".into(), Instant::now()));

// In update() loop:
if let Some((_, time)) = &self.error_message {
    if time.elapsed() > Duration::from_secs(MESSAGE_TIMEOUT_SECS) {
        self.error_message = None;
    }
}
```

### 6. Preview Size Guards

**ALWAYS check file size before preview:**

```rust
// ✅ CORRECT
if entry.size > MAX_PREVIEW_SIZE {
    ui.label(format!("File too large ({}) - skipping preview",
                    bytesize::ByteSize(entry.size)));
    return;
}

// ❌ WRONG - Can freeze UI on large files
let content = fs::read_to_string(&entry.path)?;
```

### 7. Binary Detection

**ALWAYS check for binary content before text operations:**

```rust
fn is_likely_binary(path: &Path) -> bool {
    let mut buf = [0u8; 8192];
    if let Ok(mut f) = fs::File::open(path) {
        if let Ok(n) = std::io::Read::read(&mut f, &mut buf) {
            let null_bytes = buf[..n].iter().filter(|&&b| b == 0).count();
            // More than 1% null bytes = binary
            return null_bytes > n / 100;
        }
    }
    false
}
```

### 8. UTF-8 String Truncation

**ALWAYS truncate strings at char boundaries, not byte positions:**

```rust
// ✅ CORRECT - Safe char boundary truncation
let preview = if text.chars().count() > 60 {
    let truncated: String = text.chars().take(60).collect();
    format!("{}...", truncated)
} else {
    text.clone()
};

// ❌ WRONG - Can panic on multi-byte UTF-8
let preview = if text.len() > 60 {
    format!("{}...", &text[..60])  // PANIC if byte 60 is inside a char!
} else {
    text.clone()
};

// Alternative: Use char_indices for byte-aware truncation
fn truncate_at_char_boundary(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }

    // Find last valid char boundary before max_bytes
    let mut idx = max_bytes;
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    &s[..idx]
}
```

### 9. RefCell Borrow Patterns

**ALWAYS ensure immutable borrows are dropped before mutable borrows:**

```rust
// ✅ CORRECT - Scope ensures immutable borrow is dropped
let cached_content = {
    let cache = context.preview_cache.borrow();
    cache.get(&entry.path, entry.modified)
};

let content = if let Some(cached) = cached_content {
    cached
} else {
    let content = fs::read_to_string(&entry.path)?;
    context.preview_cache.borrow_mut().insert(entry.path.clone(), content.clone(), entry.modified);
    content
};

// ❌ WRONG - Immutable borrow still active during borrow_mut()
let content = if let Some(cached) = context.preview_cache.borrow().get(&entry.path, entry.modified) {
    cached
} else {
    let content = fs::read_to_string(&entry.path)?;
    context.preview_cache.borrow_mut().insert(entry.path.clone(), content.clone(), entry.modified);
    // PANIC: RefCell already borrowed!
    content
};
```

---

## Development Workflows

### Adding a New Feature

1. **Read existing code first** - Never propose changes without understanding current implementation
2. **Check CLAUDE.md task list** - See if feature is already planned
3. **Use TodoWrite tool** - Track multi-step features
4. **Follow existing patterns** - Match code style and architecture
5. **Validate edge cases** - Empty dirs, missing files, large files, symlinks
6. **Update documentation** - Modify README.md and this file if needed
7. **Test thoroughly** - See "Testing Scenarios" section

### Fixing a Bug

1. **Reproduce the issue** - Understand the bug before coding
2. **Identify root cause** - Use Grep/Read tools to find relevant code
3. **Check for similar patterns** - Bug may exist elsewhere
4. **Write the fix** - Follow conventions in this document
5. **Validate the fix** - Test edge cases
6. **Update task list** - Mark bug as fixed in CLAUDE.md
7. **Update version changelog** - Add to README.md if significant

### Refactoring Code

1. **Don't refactor unsolicited** - Only refactor when explicitly asked
2. **Keep scope minimal** - Don't "improve" surrounding code
3. **Don't add features** - Refactor ≠ enhancement
4. **Don't add comments** - Unless logic is genuinely complex
5. **Don't abstract prematurely** - Three similar lines < premature abstraction
6. **Delete unused code completely** - No `_unused` or `// removed` comments

---

## AI Assistant Guidelines

### What to Do ✅

- **Read files before editing** - ALWAYS use Read tool first
- **Use specialized tools** - Read/Edit/Write, not bash cat/sed
- **Validate assumptions** - Check paths exist, bounds are valid
- **Track complex tasks** - Use TodoWrite for multi-step features
- **Follow conventions** - Match existing code style exactly
- **Test edge cases** - Empty, missing, large, binary, symlinks
- **Update docs** - Keep README and CLAUDE.md synchronized
- **Use layout constants** - Import from `style.rs`
- **Handle errors gracefully** - Show user-friendly messages
- **Preserve state** - Don't lose selection, clipboard, history

### What NOT to Do ❌

- **Don't over-engineer** - Keep solutions simple and focused
- **Don't add unrequested features** - Only do what's asked
- **Don't add unnecessary error handling** - Trust internal code
- **Don't add backwards compatibility hacks** - Delete unused code
- **Don't guess parameters** - Ask user if unclear
- **Don't use placeholders** - Get real values before proceeding
- **Don't skip validation** - Always check bounds, existence, size
- **Don't assume paths exist** - Especially for clipboard/history
- **Don't read binary files as text** - Check with `is_likely_binary`
- **Don't create files unnecessarily** - Prefer editing existing files

### Code Quality Standards

- **Security:** Check for command injection, XSS, path traversal
- **Performance:** Don't block UI thread, use worker for I/O
- **Reliability:** Validate all external input and async results
- **Maintainability:** Follow existing patterns, avoid magic numbers
- **Simplicity:** Minimum complexity for current requirements

---

## Common Code Patterns

### Pattern: Loading a Directory

```rust
fn navigate_to(&mut self, path: PathBuf) {
    if !path.is_dir() {
        self.error_message = Some(("Not a directory".into(), Instant::now()));
        return;
    }

    self.current_path = path.clone();
    self.loading = true;
    self.pending_path = Some(path.clone());

    let _ = self.io_sender.send(IoCommand::LoadDirectory {
        path,
        show_hidden: self.show_hidden,
    });
}
```

### Pattern: Rendering a Table

```rust
use egui_extras::{TableBuilder, Column};

TableBuilder::new(ui)
    .striped(true)
    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
    .column(Column::exact(ICON_COL_WIDTH))
    .column(Column::remainder().clip(true))  // IMPORTANT: .clip(true)!
    .column(Column::initial(80.0).resizable(true))
    .header(HEADER_HEIGHT, |mut header| {
        header.col(|ui| { ui.label(""); });
        header.col(|ui| { ui.label("Name"); });
        header.col(|ui| { ui.label("Size"); });
    })
    .body(|body| {
        body.rows(ROW_HEIGHT, entries.len(), |mut row| {
            let entry = &entries[row.index()];

            row.col(|ui| {
                ui.label(egui::RichText::new(entry.get_icon())
                    .size(ICON_SIZE));
            });

            row.col(|ui| {
                truncated_label(ui, entry.display_name());
            });

            row.col(|ui| {
                ui.label(bytesize::ByteSize(entry.size).to_string());
            });
        });
    });
```

### Pattern: Modal Dialog

```rust
if self.mode == AppMode::ShowingHelp {
    egui::Window::new("Help")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(modal_width(ctx))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .max_height(modal_max_height(ctx))
                .show(ui, |ui| {
                    // Content
                });

            if ui.button("Close").clicked() || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.mode = AppMode::Normal;
            }
        });
}
```

### Pattern: Keyboard Input Handling

```rust
fn handle_input(&mut self, ctx: &egui::Context) {
    // Check mode first
    match self.mode {
        AppMode::Normal => self.handle_normal_mode(ctx),
        AppMode::Filter => self.handle_filter_mode(ctx),
        AppMode::Visual => self.handle_visual_mode(ctx),
        AppMode::Command => self.handle_command_mode(ctx),
        _ => {}
    }
}

fn handle_normal_mode(&mut self, ctx: &egui::Context) {
    ctx.input(|i| {
        // Single key presses
        if i.key_pressed(egui::Key::J) || i.key_pressed(egui::Key::ArrowDown) {
            self.move_selection(1);
        }

        // Modifier keys
        if i.modifiers.shift && i.key_pressed(egui::Key::S) {
            self.mode = AppMode::Search;
        }

        // Don't forget to consume the key!
        // (egui handles this automatically for key_pressed)
    });
}
```

---

## Task Management System

### Current Task Priorities

See the full task list at the end of this document. Key priorities:

**🟢 Mostly Complete: Code Organization**
- ✅ Split monolithic `main.rs` into modules (DONE)
- ✅ Created `app.rs`, `entry.rs`, `state/`, `io/`, `view/` (DONE)
- ✅ Extract layout constants to `style.rs` (DONE)
- ✅ Extract `input.rs` keyboard handling (DONE)
- ✅ Extract `view/panels.rs` and `view/modals.rs` (DONE)
- 🔶 Integrate logical state structs into Heike (IN PROGRESS)
  - ✅ Created `NavigationState`, `SelectionState`, `EntryState`, `UIState`, `ModeState` (DONE)
  - ⏳ Migrate Heike fields to use new state structs (NEXT PHASE)

**🟡 Medium: Performance**
- Incremental watcher updates (diff fs events)
- Virtual scrolling for code preview
- Preview caching by path + mtime
- Lazy archive preview

**🟡 Medium: UX Features**
- Trash bin support (using `trash` crate)
- Sort options (name/size/modified/extension)
- File permissions display (Unix format)
- Bulk rename (vidir-style)
- Bookmarks (`g` prefix shortcuts)
- Tabs (multiple directories)

**🟢 Low: Additional Features**
- Settings persistence (TOML)
- CLI path argument
- zoxide integration
- Git status indicators
- Custom opener rules

### Yazi Feature Parity Tracker

| Feature | Status |
|---------|--------|
| Async I/O | ✅ Done |
| Miller columns | ✅ Done |
| Vim keybindings | ✅ Done |
| Visual mode | ✅ Done |
| Filter `/` | ✅ Done |
| Command `:` | ✅ Done |
| Image preview | ✅ Done |
| Syntax highlighting | ✅ Done |
| Archive preview | ✅ Done |
| Content search | ✅ Done |
| Tabs | ❌ Todo |
| Bulk rename | ❌ Todo |
| Trash bin | ❌ Todo |
| Bookmarks | ❌ Todo |
| Git status | ❌ Todo |
| Task manager | ❌ Todo |
| Plugin system | ❌ Todo |

---

## Testing Scenarios

### Must Test After Layout Changes

1. **Maximize window** with long filename selected → no black gap
2. **Resize window** rapidly → panels stay proportional
3. **Long filename** (>100 chars) → text clips with ellipsis, no overflow
4. **Deep directory path** (>10 levels) → breadcrumbs scroll horizontally
5. **Small window** (<800px wide) → modals fit, panels respect minimums
6. **Drag dividers** → smooth resize, respects min/max bounds

### Must Test After File Operation Changes

1. **Delete file** → selection moves to next/previous item
2. **Copy/paste** → original files preserved, new files created
3. **Cut/paste** → files moved, originals removed
4. **Rename** → selection stays on renamed item
5. **Create directory** → new dir appears and is selected
6. **Stale clipboard** (deleted source files) → paste shows error, skips missing

### Must Test After Navigation Changes

1. **Back/forward** → returns to correct directory and selection
2. **History with deleted dirs** → skips missing, shows error
3. **Filter mode** → exit filter restores previous selection
4. **Parent click** → navigates to parent, remembers child selection
5. **Preview click** (dir) → navigates into directory

### Must Test After Search Changes

1. **Content search** → finds matches in text/pdf/zip files
2. **Search results navigation** (n/N) → cycles through matches correctly
3. **Enter on search result** → opens file (future: at line number)
4. **Empty search query** → shows error
5. **Search with no results** → shows appropriate message
6. **Case sensitivity toggle** → affects results correctly

---

## Layout Constants Reference

All layout constants are defined in `src/style.rs`:

```rust
// Sizing
pub const ICON_SIZE: f32 = 14.0;
pub const ICON_COL_WIDTH: f32 = 30.0;
pub const ROW_HEIGHT: f32 = 24.0;
pub const HEADER_HEIGHT: f32 = 20.0;
pub const DIVIDER_WIDTH: f32 = 4.0;

// Panel constraints
pub const PARENT_MIN: f32 = 100.0;
pub const PARENT_MAX: f32 = 400.0;
pub const PARENT_DEFAULT: f32 = 200.0;
pub const PREVIEW_MIN: f32 = 150.0;
pub const PREVIEW_MAX: f32 = 800.0;
pub const PREVIEW_DEFAULT: f32 = 350.0;

// Modals
pub const MODAL_MIN_WIDTH: f32 = 300.0;
pub const MODAL_MAX_WIDTH: f32 = 500.0;
pub const MODAL_WIDTH_RATIO: f32 = 0.6;
pub const MODAL_HEIGHT_RATIO: f32 = 0.8;

// Timing
pub const PREVIEW_DEBOUNCE_MS: u64 = 200;
pub const DOUBLE_PRESS_MS: u64 = 500;
pub const MESSAGE_TIMEOUT_SECS: u64 = 5;

// Preview limits
pub const HEX_PREVIEW_BYTES: usize = 512;
pub const TEXT_PREVIEW_LIMIT: usize = 100_000;
pub const ARCHIVE_PREVIEW_ITEMS: usize = 100;
pub const MAX_PREVIEW_SIZE: u64 = 10 * 1024 * 1024; // 10MB

// Helper functions
pub fn modal_width(ctx: &egui::Context) -> f32;
pub fn modal_max_height(ctx: &egui::Context) -> f32;
pub fn truncated_label(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) -> egui::Response;
```

---

## Dependencies Guide

### Core Dependencies

```toml
# GUI Framework
eframe = "0.33.2"                              # Main GUI framework
egui_extras = { version = "0.33.2", features = ["all_loaders"] }

# System Integration
open = "5.3"                                   # Open files with system default
directories = "5.0"                            # Find standard directories
notify = "7.0"                                 # File system watching

# Utilities
chrono = "0.4"                                 # Timestamps
bytesize = "1.3"                              # Human-readable file sizes
image = "0.25"                                # Image loading

# Preview & Syntax
syntect = "5.2"                               # Syntax highlighting
pulldown-cmark = "0.12"                       # Markdown rendering

# Archive Support
zip = "2.2"                                   # ZIP archives
tar = "0.4"                                   # TAR archives
flate2 = "1.0"                               # Gzip compression

# Document Support
lopdf = "0.34"                                # PDF reading
calamine = "0.32"                             # Excel (XLS/XLSX)
docx-rs = "0.4"                              # Word (DOCX)
id3 = "1.14"                                 # Audio metadata

# Search
grep-searcher = "0.1"                         # Text searching
grep-regex = "0.1"                           # Regex matching
grep-matcher = "0.1"                         # Matcher interface
ignore = "0.4"                               # Gitignore-aware walking
rayon = "1.10"                               # Parallel operations
```

### Planned Dependencies

```toml
# For future features (not yet added)
trash = "5.0"                                 # Trash bin support
serde = { version = "1.0", features = ["derive"] }  # Settings
toml = "0.8"                                  # Settings file format
gix = "0.68"                                  # Git status (optional, heavy)
```

---

## Git Workflow

### Branch Strategy

- **Main branch:** `main` (production-ready)
- **Feature branches:** `feat/description` or `claude/session-id`
- **Bug fix branches:** `fix/description`
- **Chore branches:** `chore/description`

### Commit Message Format

Follow existing patterns from git log:

```
feat(component): brief description

- Detailed change 1
- Detailed change 2
- Detailed change 3
```

**Examples:**
- `feat(ui): truncate labels, symlink indicators, status bar polish`
- `fix(core): harden async loading and selection retention`
- `chore(docs): update README with latest features`

### PR Guidelines

When creating pull requests:

1. **Write descriptive PR title** - Same format as commits
2. **Summarize changes** - 1-3 bullet points
3. **Include test plan** - How to verify the changes
4. **Reference issues** - Link related issues/tasks
5. **Update docs** - README.md, CLAUDE.md if needed

---

## Complete Task List

## Critical: Bugs

- [x] **Race condition in async loading** — Verify `current_path` matches result path before applying `DirectoryLoaded`
- [x] **Selection lost after filter clear** — Restore selection to previously selected item when exiting filter mode
- [x] **Clipboard paths can go stale** — Validate source paths exist before paste operation
- [x] **History contains deleted directories** — Validate directory exists on back/forward navigation
- [x] **File size limits on preview** — Add `MAX_PREVIEW_SIZE` check before `fs::read_to_string`
- [x] **Shift+S content search hotkey** — Ensure search modal opens from all contexts
- [x] **Arrow keys mirror h/l navigation** — Bind Left/Right arrows to parent/enter actions
- [x] **Search results navigation scroll** — Add scroll_to_row for search results table
- [x] **Binary file detection false positives** — Improved is_likely_binary to check null byte percentage
- [x] **Search results auto-scroll alignment** — Changed from Center to None to match main view behavior
- [x] **Search results not clickable** — Added click handlers to search result rows
- [x] **UTF-8 byte boundary panic in search** — Fixed preview truncation to use char boundaries instead of byte slicing
- [x] **Mouse scroll decoupling** — Scrolling via mouse should not recenter view on selected item; only keyboard nav/scroll should recenter (enhanced edge case fix: reset disable_autoscroll on navigation)
- [x] **Parent directory selection** — Navigating to parent should restore previous folder as active (selected) item (implemented using pending_selection_path; also fixed selection memory fallback in apply_filter())
- [x] **RefCell borrow panic in preview cache** — Fixed preview handlers to properly scope immutable borrows before attempting mutable borrows

## High: Layout Fixes

- [x] **Strip-based layout** — Replace SidePanel approach with `egui_extras::Strip` to eliminate black gap
- [x] **Manual resize dividers** — Add draggable dividers between panes
- [x] **Column clipping** — Add `.clip(true)` to all `Column::remainder()` calls
- [x] **Truncated labels** — Add `truncated_label()` helper with ellipsis overflow
- [x] **ScrollArea constraints** — Add `max_height(ui.available_height())` to all ScrollAreas
- [x] **Image preview sizing** — Add `maintain_aspect_ratio(true)` and height constraint
- [x] **Responsive modals** — Scale modal width/height to screen size
- [x] **Breadcrumb overflow** — Wrap breadcrumbs in horizontal ScrollArea

## High: Code Organization

- [x] **Split monolith** — Extract into modules:
  - [x] `src/app.rs` — Heike struct, update loop (2178 lines)
  - [x] `src/entry.rs` — FileEntry (99 lines)
  - [x] `src/state/mod.rs` — Mode, Clipboard, Search state structs
  - [x] `src/io/mod.rs` — Directory reading, search, worker thread
  - [x] `src/view/mod.rs` — Preview rendering (799 lines in preview.rs)
  - [ ] `src/input.rs` — Keyboard handling (placeholder only, still in app.rs)
  - [x] `src/style.rs` — Theme, layout constants (76 lines)
- [ ] **Extract remaining UI** — Move from app.rs to view/:
  - [ ] `src/view/panels.rs` — Miller columns rendering
  - [ ] `src/view/modals.rs` — Dialogs and popups
- [ ] **Group Heike fields** — Split into `NavigationState`, `EntryState`, `ModeState`, etc.
- [x] **Layout constants module** — All constants in `src/style.rs`
- [ ] **Modularize preview components** — Make preview rendering pluggable:
  - [ ] Create `PreviewHandler` trait for extensible preview types
  - [ ] Allow config to enable/disable specific preview components
  - [ ] Design for Yazi plugin compatibility (e.g., Lua preview plugins)
  - [ ] Document plugin architecture for community extensions

## Medium: Performance

- [x] **Incremental watcher updates** — Diff fs events instead of full refresh
- [x] **Virtual scrolling for code preview** — Only highlight visible lines (1000 line limit)
- [x] **Preview caching** — Memoize preview content by path + modified time
- [x] **Parent directory caching** — Skip re-read when parent unchanged
- [x] **Lazy archive preview** — Don't iterate full archive for count

## Medium: UX Features

- [x] **Trash bin support** — Add `trash` crate, replace `fs::remove_file`
- [x] **Sort options** — Name/Size/Modified/Extension, Asc/Desc, dirs-first toggle
- [x] **Symlink indication** — Check `fs::symlink_metadata`, show indicator
- [x] **File permissions display** — Unix `rwxr-xr-x` format in preview
- [x] **Status line info** — Selected size, item count, current path display
- [x] **Additional vim/yazi keybinds** — Implement missing binds:
  - [x] `Ctrl-D` / `Ctrl-U` — Half-page down/up navigation
  - [x] `Ctrl-F` / `Ctrl-B` — Full-page down/up navigation
  - [x] `Ctrl-R` — Invert selection (Yazi-compatible)
  - [x] Fix **Visual/selection mode** — Review yazi implementation and correct behavior
    - [x] Invert selection (Ctrl+R) added
    - [x] Unset mode (V for deselection while navigating) — DONE (V toggles visual mode)
    - [x] Selection count in status bar — DONE (shows selected count with size)
    - [x] Visual distinction between cursor and selected items — DONE (yellow ▶ and ✓ prefix)
  - [ ] Additional vim binds that make sense for file navigation (e.g., `o` to open in new tab)
- [ ] **Bulk rename** — vidir-style multi-file rename mode
- [x] **Bookmarks** — `g` prefix shortcuts (gd=Downloads, gh=Home, etc.) — DONE
  - [x] Default bookmarks: h=home, d=Downloads, p=Projects, t=/tmp
  - [x] Configurable via config.toml
  - [x] Path expansion for ~ (home directory)
- [x] **Tabs** — Multiple directory tabs — DONE
  - [x] TabsManager state system
  - [x] Tab creation, switching, closing
  - [x] Per-tab state (path, history, selection)
  - [x] UI tab bar rendering (custom implementation with Frame + horizontal ScrollArea)
  - [x] Tab keyboard shortcuts (Ctrl+T new tab, Ctrl+W close, Alt+1-9 switch, Ctrl+Tab/Shift+Tab cycle)

## Medium: Error Handling

- [x] **Search progress tracking** — Track files searched, skipped, errors
- [ ] **Retry logic for file ops** — Backoff retry for transient failures (optional)
- [x] **Consistent Result/Option usage** — Standardize error handling patterns (UIState helpers)
- [x] **Message auto-dismiss** — Clear info/error messages after timeout (MESSAGE_TIMEOUT_SECS)

## Low: Security Hardening

- [x] **Path traversal protection** — Canonicalize and verify `:mkdir`/`:touch` paths
- [x] **Preview size limits** — Skip preview for files > 10MB

## Low: Code Quality

- [x] **Remove dead code** — Audit unused imports and functions
- [x] **Reduce cloning** — Clone only PathBuf in context menus, not full entry
- [x] **Fix double-press timer** — Clear stale `last_g_press` properly

## Low: Additional Features

- [x] **Configuration system** — Create config file support (~/.config/heike/config.toml):
  - [x] Font size customization (12.0pt default)
  - [x] Icon size customization (14.0pt default)
  - [x] Theme selection (dark/light mode)
  - [x] Panel widths (parent/preview pane sizing)
  - [x] UI preferences (show_hidden, sort options, dirs_first)
  - [ ] Font override (path to custom TTF file) — Future enhancement
  - [ ] Keybinding customization (TOML-based) — Future enhancement
  - [ ] Custom color overrides — Future enhancement
- [ ] **Settings persistence** — Save panel widths, theme, show_hidden to TOML
- [ ] **CLI path argument** — Accept starting directory as arg
- [ ] **zoxide integration** — Jump to frecent directories
- [ ] **Git status indicators** — Show modified/untracked/ignored status
- [ ] **Custom opener rules** — Config file for extension → application mapping

## Backlog: Future Considerations

- [ ] **Advanced plugin system** — Lua or WASM extensibility (beyond preview components)
- [ ] **Custom themes** — User-defined color schemes with full customization
- [ ] **Task manager UI** — Show async operation progress and background tasks
- [ ] **Split panes** — Side-by-side directory comparison
- [ ] **Yazi plugin ecosystem bridge** — Runtime for running Yazi plugins in Heike

---

## Quick Command Reference

### Vim-style Navigation
- `j`/`k` or ↑/↓ — Navigate up/down
- `h` or ← or Backspace — Go to parent directory
- `l` or → — Enter directory
- `Enter` — Open file / Enter directory
- `gg` / `G` — Jump to top / bottom
- `Alt+←` / `Alt+→` — Back / Forward in history

### Selection & Operations
- `v` — Visual selection mode
- `Shift+V` — Select all
- `Ctrl+A` — Select all
- `Space` — Toggle selection
- `y` — Yank (copy) selected files
- `x` — Cut selected files
- `p` — Paste clipboard contents
- `d` — Delete with confirmation
- `r` — Rename selected file

### Modes & Search
- `/` — Fuzzy filter mode
- `:` — Command mode (`:q`, `:mkdir`, `:touch`)
- `Shift+S` — Content search
- `Esc` — Return to normal mode
- `.` — Toggle hidden files

### Other
- `?` — Show help
- Mouse: Click to select, double-click to open, right-click for context menu

---

## Additional Resources

- **README.md** — User-facing documentation and feature list
- **FIXES.md** — Detailed technical fixes and migration guide
- **Cargo.toml** — Full dependency list with versions
- **GitHub Issues** — Bug reports and feature requests

---

*Last updated: 2025-12-14*

### Previous Session (Performance optimizations + polish):

  **Performance Optimizations:**
  - Implemented preview caching with LRU eviction (100 entries)
    - Cache validation using path + mtime
    - Integrated into TextPreviewHandler and MarkdownPreviewHandler
  - Implemented virtual scrolling for code preview
    - 1000-line limit for syntax highlighting
    - Performance: ~200ms → ~20ms for large files
  - Implemented parent directory caching
    - Avoids redundant disk I/O when navigating siblings
    - Cache invalidation on parent directory changes
  - Enhanced search progress tracking
    - Detailed statistics: files searched, skipped, errors
    - Real-time progress display in search modal

  **Code Quality & Polish:**
  - Refactored message handling for consistency
    - All code now uses UIState::set_error() and set_info() helpers
    - Centralized message expiration logic in clear_expired_messages()
    - Improves maintainability and reduces duplication
  - Removed dead code and unused imports
  - Reduced unnecessary cloning in context menus
  - Fixed stale double-press timer issue
  - Added sort options display to status bar
  - Added keyboard shortcuts to UI (Help modal Close button)

  **Documentation:**
  - Updated CLAUDE.md task list with completion status
  - All Medium: Performance tasks completed
  - All Low: Code Quality tasks completed
  - Most Medium: Error Handling tasks completed

### Current Session (RefCell panic fix + tabs UI implementation):

  **Bug Fixes:**
  - Fixed RefCell borrow panic in preview cache
    - Issue: Immutable borrow still active during borrow_mut() call
    - Solution: Scope immutable borrow to ensure it's dropped before mutable borrow
    - Affected files: text.rs and markdown.rs preview handlers
    - Added RefCell borrow pattern to CLAUDE.md conventions

  **New Features:**
  - Implemented tab bar UI rendering
    - Custom tab bar with Frame-based tab buttons
    - Active tab highlighting with visual distinction
    - Close buttons on individual tabs (disabled for last tab)
    - Plus button to create new tabs
    - Horizontal scrolling for many tabs
    - Click to switch tabs, click × to close

  **Documentation Updates:**
  - Updated CLAUDE.md with RefCell borrow convention (#9)
  - Updated architecture section to reflect current module structure
  - Marked tabs feature as completed (UI now implemented)
  - Updated version to 0.8.2 (Stability & Tabs update)
  - Documented completed refactoring progress
  - Updated README with 0.8.2 version history

*For questions or clarifications, refer to git commit history or ask the repository maintainer.*
