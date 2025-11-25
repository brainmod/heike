# Heike: egui → iced Transition Guide

## Overview

This document outlines the architectural transition from egui (immediate mode) to iced (Elm architecture) for the Heike file manager. The goal is idiomatic iced, not a 1:1 port.

## Package Versions (as of Nov 2025)

```toml
[dependencies]
# Core
iced = { version = "0.13", features = ["image", "svg", "tokio", "highlighter"] }
iced_aw = "0.11"  # Additional widgets (tabs, menu, split)

# Async runtime
tokio = { version = "1.41", features = ["full"] }

# Retained from current
chrono = "0.4"
directories = "6.0"
bytesize = "1.3"
notify = "7.0"
syntect = "5.2"
pulldown-cmark = "0.12"
zip = "2.2"
tar = "0.4"
flate2 = "1.1"
id3 = "1.14"
lopdf = "0.36"
calamine = "0.32"
docx-rs = "0.4"
grep-searcher = "0.1"
grep-regex = "0.1"
ignore = "0.4"
rayon = "1.10"
image = "0.25"
open = "5.3"
```

## Architecture Shift

### egui (Current)
- Immediate mode: rebuild UI every frame
- State mutation during render
- Input handled inline with drawing
- `RefCell` patterns for deferred actions

### iced (Target)
- Elm architecture: Model → Message → Update → View
- Immutable view, mutations only in `update()`
- `Command`/`Task` for async operations
- `Subscription` for external events (file watcher, keyboard)

## Core Types Mapping

```rust
// === MODEL ===
pub struct Heike {
    // Navigation
    current_path: PathBuf,
    history: Vec<PathBuf>,
    history_idx: usize,
    
    // Entries
    entries: Vec<FileEntry>,
    parent_entries: Vec<FileEntry>,
    selected: Option<usize>,
    multi_select: HashSet<PathBuf>,
    
    // Mode (replaces AppMode enum)
    mode: Mode,
    input_buffer: String,
    
    // Clipboard
    clipboard: Clipboard,
    
    // Search
    search: SearchState,
    
    // Settings
    show_hidden: bool,
    theme: Theme,
}

#[derive(Default)]
pub enum Mode {
    #[default]
    Normal,
    Visual,
    Filter,
    Command,
    Rename,
    Search,
    SearchResults(Vec<SearchResult>),
    Confirm(ConfirmAction),
}

// === MESSAGES ===
#[derive(Debug, Clone)]
pub enum Message {
    // Navigation
    Navigate(PathBuf),
    NavigateUp,
    NavigateBack,
    NavigateForward,
    Select(usize),
    SelectDelta(i32),  // +1/-1 for j/k
    
    // Modes
    SetMode(Mode),
    InputChanged(String),
    InputSubmit,
    
    // File ops
    Yank,
    Cut,
    Paste,
    Delete,
    ConfirmDelete,
    
    // Async results
    DirectoryLoaded(Result<Vec<FileEntry>, String>),
    SearchComplete(Vec<SearchResult>),
    FileWatcherEvent,
    
    // UI
    ToggleHidden,
    ToggleTheme,
    OpenFile(PathBuf),
}
```

## Idiomatic iced Patterns

### 1. Use `Task` for Async I/O

```rust
fn update(&mut self, message: Message) -> Task<Message> {
    match message {
        Message::Navigate(path) => {
            self.current_path = path.clone();
            Task::perform(
                load_directory(path, self.show_hidden),
                Message::DirectoryLoaded
            )
        }
        Message::DirectoryLoaded(Ok(entries)) => {
            self.entries = entries;
            Task::none()
        }
        _ => Task::none()
    }
}

async fn load_directory(path: PathBuf, show_hidden: bool) -> Result<Vec<FileEntry>, String> {
    tokio::task::spawn_blocking(move || read_directory(&path, show_hidden))
        .await
        .map_err(|e| e.to_string())?
}
```

### 2. Use `Subscription` for File Watcher

```rust
fn subscription(&self) -> Subscription<Message> {
    Subscription::batch([
        keyboard::on_key_press(handle_key),
        file_watcher_subscription(self.current_path.clone()),
    ])
}

fn file_watcher_subscription(path: PathBuf) -> Subscription<Message> {
    iced::subscription::channel(path.clone(), 100, |mut output| async move {
        let (tx, mut rx) = tokio::sync::mpsc::channel(10);
        let mut watcher = notify::recommended_watcher(move |_| {
            let _ = tx.blocking_send(());
        }).unwrap();
        watcher.watch(&path, RecursiveMode::NonRecursive).unwrap();
        
        loop {
            if rx.recv().await.is_some() {
                let _ = output.send(Message::FileWatcherEvent).await;
            }
        }
    })
}
```

### 3. Use `pane_grid` for Miller Columns

```rust
fn view(&self) -> Element<Message> {
    let panes = pane_grid::PaneGrid::new(&self.panes, |_id, pane, _| {
        pane_grid::Content::new(match pane {
            Pane::Parent => self.view_parent_list(),
            Pane::Current => self.view_current_list(),
            Pane::Preview => self.view_preview(),
        })
    })
    .width(Fill)
    .height(Fill)
    .on_resize(10, Message::PaneResized);
    
    container(panes).into()
}
```

### 4. Keyboard Handling via Subscription

```rust
fn handle_key(key: Key, modifiers: Modifiers) -> Option<Message> {
    use keyboard::key::Named;
    
    match (key, modifiers) {
        // Vim navigation
        (Key::Character(c), Modifiers::NONE) => match c.as_str() {
            "j" => Some(Message::SelectDelta(1)),
            "k" => Some(Message::SelectDelta(-1)),
            "h" => Some(Message::NavigateUp),
            "l" => Some(Message::InputSubmit), // Enter dir
            "g" => Some(Message::SetMode(Mode::GPrefix)),
            "G" => Some(Message::SelectLast),
            "y" => Some(Message::Yank),
            "x" => Some(Message::Cut),
            "p" => Some(Message::Paste),
            "d" => Some(Message::SetMode(Mode::Confirm(ConfirmAction::Delete))),
            "r" => Some(Message::SetMode(Mode::Rename)),
            "v" => Some(Message::SetMode(Mode::Visual)),
            "/" => Some(Message::SetMode(Mode::Filter)),
            ":" => Some(Message::SetMode(Mode::Command)),
            "." => Some(Message::ToggleHidden),
            _ => None,
        },
        (Key::Named(Named::ArrowDown), _) => Some(Message::SelectDelta(1)),
        (Key::Named(Named::ArrowUp), _) => Some(Message::SelectDelta(-1)),
        (Key::Named(Named::Enter), _) => Some(Message::InputSubmit),
        (Key::Named(Named::Escape), _) => Some(Message::SetMode(Mode::Normal)),
        (Key::Named(Named::Backspace), Modifiers::NONE) => Some(Message::NavigateUp),
        // Alt+Arrow for history
        (Key::Named(Named::ArrowLeft), m) if m.alt() => Some(Message::NavigateBack),
        (Key::Named(Named::ArrowRight), m) if m.alt() => Some(Message::NavigateForward),
        _ => None,
    }
}
```

## Yazi Feature Alignment

| Yazi Feature | Heike Status | iced Implementation |
|--------------|--------------|---------------------|
| Miller columns | ✅ | `pane_grid` with 3 panes |
| Vim keybindings (hjkl, gg/G) | ✅ | `keyboard::on_key_press` subscription |
| Visual selection | ✅ | `Mode::Visual` + `HashSet<PathBuf>` |
| Yank/Cut/Paste | ✅ | `Clipboard` struct, `Task` for I/O |
| Fuzzy filter `/` | ✅ | `text_input` + filter in `update()` |
| Command mode `:` | ✅ | Parse commands in `InputSubmit` |
| Hidden files `.` | ✅ | Toggle + `Task::perform(reload)` |
| Image preview | ✅ | `iced::widget::image` |
| Syntax highlight | ✅ | `iced::highlighter` or syntect |
| History back/fwd | ✅ | `Vec<PathBuf>` + index |
| File watcher | ✅ | `Subscription::channel` + notify |
| Bulk rename | ❌ TODO | Modal + batch `Task` |
| Tabs | ❌ TODO | `iced_aw::Tabs` |
| Bookmarks `g` prefix | ❌ TODO | `HashMap<char, PathBuf>` |
| Trash bin | ❌ TODO | `trash` crate |
| Archive extract | ❌ TODO | `Task::perform` with zip/tar |
| Git status | ❌ TODO | `gix` crate indicators |

## File Structure

```
src/
├── main.rs           # Entry point, iced::application
├── app.rs            # Heike struct, update(), view()
├── message.rs        # Message enum
├── model/
│   ├── mod.rs
│   ├── entry.rs      # FileEntry
│   ├── mode.rs       # Mode enum
│   └── clipboard.rs  # Clipboard state
├── view/
│   ├── mod.rs
│   ├── parent.rs     # Parent pane
│   ├── current.rs    # Current directory list
│   ├── preview.rs    # Preview pane
│   └── modals.rs     # Command/rename/confirm dialogs
├── io/
│   ├── mod.rs
│   ├── directory.rs  # read_directory()
│   ├── search.rs     # Content search
│   └── ops.rs        # Copy/move/delete
├── subscription/
│   ├── mod.rs
│   ├── keyboard.rs   # Key handling
│   └── watcher.rs    # File system watcher
└── style/
    ├── mod.rs
    └── theme.rs      # Light/dark themes
```

## Migration Steps

1. **Scaffold** - Create `Message` enum and empty `update()`/`view()`
2. **Navigation** - Implement directory loading with `Task`
3. **Keyboard** - Add `Subscription` for vim bindings
4. **List rendering** - Use `scrollable` + `column` or `iced_aw::Grid`
5. **Preview** - Port preview renderers (image, syntax, hex)
6. **File ops** - Implement clipboard with async `Task`
7. **Search** - Async search with progress via `Subscription::channel`
8. **Polish** - Theming, icons, modals

## Key Differences to Embrace

| egui Pattern | iced Equivalent |
|--------------|-----------------|
| `ctx.input(key_pressed)` | `keyboard::on_key_press` subscription |
| `RefCell<Option<Action>>` | Return `Message` from view, handle in update |
| `ui.spinner()` | Conditional `text("Loading...")` + store loading state |
| `egui::Window::show()` | `iced_aw::Modal` or overlay in view |
| `TableBuilder` | `scrollable(column![...])` or custom widget |
| Immediate `fs::read` | `Task::perform(async_read, Message::Result)` |

## Notes

- **No tables built-in**: Use `iced_aw::Grid` or manual `Row`/`Column`
- **Context menus**: Use `iced_aw::ContextMenu`
- **Drag-drop**: Native support via `iced::event::Event::File`
- **Icons**: Embed Nerd Font as before, use `text()` with font family
- **State in subscription closures**: Pass owned/cloned data, not references
