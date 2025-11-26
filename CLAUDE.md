# Heike: Development Task List

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

## High: Layout Fixes (see FIXES.md)

- [x] **Strip-based layout** — Replace SidePanel approach with `egui_extras::Strip` to eliminate black gap
- [x] **Manual resize dividers** — Add draggable dividers between panes
- [x] **Column clipping** — Add `.clip(true)` to all `Column::remainder()` calls
- [x] **Truncated labels** — Add `truncated_label()` helper with ellipsis overflow
- [x] **ScrollArea constraints** — Add `max_height(ui.available_height())` to all ScrollAreas
- [x] **Image preview sizing** — Add `maintain_aspect_ratio(true)` and height constraint
- [x] **Responsive modals** — Scale modal width/height to screen size
- [x] **Breadcrumb overflow** — Wrap breadcrumbs in horizontal ScrollArea

## High: Code Organization

- [ ] **Split monolith** — Extract into modules:
  - [ ] `src/app.rs` — Heike struct, update loop
  - [ ] `src/entry.rs` — FileEntry
  - [ ] `src/state/mod.rs` — Mode, Clipboard, Search state structs
  - [ ] `src/io/mod.rs` — Directory reading, search, watcher, worker thread
  - [ ] `src/view/mod.rs` — Panel rendering, preview, modals
  - [ ] `src/input.rs` — Keyboard handling
  - [ ] `src/style.rs` — Theme, layout constants
- [ ] **Group Heike fields** — Split into `NavigationState`, `EntryState`, `ModeState`, etc.
- [ ] **Layout constants module** — Extract magic numbers to named constants

## Medium: Performance

- [ ] **Incremental watcher updates** — Diff fs events instead of full refresh
- [ ] **Virtual scrolling for code preview** — Only highlight visible lines
- [ ] **Preview caching** — Memoize preview content by path + modified time
- [ ] **Parent directory caching** — Skip re-read when parent unchanged
- [ ] **Lazy archive preview** — Don't iterate full archive for count

## Medium: UX Features

- [ ] **Trash bin support** — Add `trash` crate, replace `fs::remove_file`
- [ ] **Sort options** — Name/Size/Modified/Extension, Asc/Desc, dirs-first toggle
- [x] **Symlink indication** — Check `fs::symlink_metadata`, show indicator
- [ ] **File permissions display** — Unix `rwxr-xr-x` format in preview
- [ ] **Status line info** — Selected size, item count, git branch, disk space
- [ ] **Bulk rename** — vidir-style multi-file rename mode
- [ ] **Bookmarks** — `g` prefix shortcuts (gd=Downloads, gh=Home, etc.)
- [ ] **Tabs** — Multiple directory tabs with `iced_aw::Tabs` or similar

## Medium: Error Handling

- [ ] **Search progress tracking** — Track files searched, skipped, errors
- [ ] **Retry logic for file ops** — Backoff retry for transient failures
- [ ] **Consistent Result/Option usage** — Standardize error handling patterns
- [ ] **Message auto-dismiss** — Clear info/error messages after timeout

## Low: Security Hardening

- [ ] **Path traversal protection** — Canonicalize and verify `:mkdir`/`:touch` paths
- [ ] **Preview size limits** — Skip preview for files > 10MB

## Low: Code Quality

- [ ] **Remove dead code** — Audit unused imports and functions
- [ ] **Reduce cloning** — Clone only PathBuf in context menus, not full entry
- [ ] **Fix double-press timer** — Clear stale `last_g_press` properly

## Low: Additional Features

- [ ] **Settings persistence** — Save panel widths, theme, show_hidden to TOML
- [ ] **CLI path argument** — Accept starting directory as arg
- [ ] **zoxide integration** — Jump to frecent directories
- [ ] **Git status indicators** — Show modified/untracked/ignored status
- [ ] **Custom opener rules** — Config file for extension → application mapping

## Backlog: Future Considerations

- [ ] **Configurable keybindings** — TOML-based keymap
- [ ] **Custom themes** — User-defined color schemes
- [ ] **Plugin system** — Lua or WASM extensibility
- [ ] **Task manager UI** — Show async operation progress
- [ ] **Split panes** — Side-by-side directory comparison

---

## Yazi Feature Parity

| Feature | Status |
|---------|--------|
| Async I/O | ✅ |
| Miller columns | ✅ |
| Vim keybindings | ✅ |
| Visual mode | ✅ |
| Filter `/` | ✅ |
| Command `:` | ✅ |
| Image preview | ✅ |
| Syntax highlighting | ✅ |
| Archive preview | ✅ |
| Content search | ✅ |
| Tabs | ❌ |
| Bulk rename | ❌ |
| Trash bin | ❌ |
| Bookmarks | ❌ |
| Git status | ❌ |
| Task manager | ❌ |
| Plugin system | ❌ |

---

## File Structure Target

```
src/
├── main.rs
├── app.rs
├── entry.rs
├── input.rs
├── style.rs
├── state/
│   ├── mod.rs
│   ├── mode.rs
│   ├── clipboard.rs
│   └── search.rs
├── io/
│   ├── mod.rs
│   ├── directory.rs
│   ├── search.rs
│   ├── watcher.rs
│   └── worker.rs
└── view/
    ├── mod.rs
    ├── panels.rs
    ├── preview.rs
    ├── modals.rs
    └── table.rs
```

---

## Quick Reference: Layout Constants

```rust
// src/style.rs
pub const ICON_SIZE: f32 = 14.0;
pub const ICON_COL_WIDTH: f32 = 30.0;
pub const ROW_HEIGHT: f32 = 24.0;
pub const DIVIDER_WIDTH: f32 = 4.0;
pub const PARENT_BOUNDS: (f32, f32) = (100.0, 400.0);
pub const PREVIEW_BOUNDS: (f32, f32) = (150.0, 800.0);
pub const MODAL_WIDTH_RATIO: f32 = 0.6;
pub const PREVIEW_DEBOUNCE_MS: u64 = 200;
pub const DOUBLE_PRESS_MS: u64 = 500;
pub const MAX_PREVIEW_SIZE: u64 = 10 * 1024 * 1024;
pub const ARCHIVE_PREVIEW_ITEMS: usize = 100;
pub const MESSAGE_TIMEOUT_SECS: u64 = 5;
```

---

## Dependencies to Add

```toml
# Cargo.toml additions
trash = "5.0"           # Trash bin support
serde = { version = "1.0", features = ["derive"] }  # Settings
toml = "0.8"            # Settings file format
gix = "0.68"            # Git status (optional, heavy)
```
