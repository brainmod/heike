# **Heike**

**Origin:** Named after the *Heikegani* (平家蟹), a species of crab native to Japan with a shell that bears the pattern of a human face, often said to resemble the face of an angry samurai.
**Philosophy:** Heike is a GUI spiritual successor to the terminal file manager **Yazi**. It marries the speed and keyboard-centric efficiency of a TUI with the rich media capabilities and distinct visual layout of a modern GUI (using egui).

## **Project Status: Active Prototype**

**Current Version:** 0.6.0 (The "Manager" Update)

### **Recent Updates** (2025-11-22)
* ✅ **Icon Display Improvements:** Enhanced icon rendering with consistent sizing (14pt) across all panes
* ✅ **Navigation Fix:** Corrected pane navigation behavior - clicking the active directory in parent pane now navigates up
* ✅ **UX Enhancement:** Improved Miller column navigation to match expected file manager behavior

## **Core Features**

### **Interface & Layout**
* **Miller Columns:** 3-pane layout (Parent → Current → Preview)
* **Resizable Panels:** Customizable sidebar and preview pane widths
* **Unified Styling:** Consistent striped table appearance across all panes
* **App Icon:** Custom Heikegani crab icon in window title bar
* **Theme System:** Light/Dark mode toggle with visual indicator
* **Responsive Preview:**
  * Directory contents preview in right pane
  * File content preview with syntax highlighting
  * Image preview for common formats (PNG, JPG, WEBP, etc.)
  * Clickable navigation in preview pane

### **Navigation & Input**
* **Vim-style Keybindings:**
  * `j`/`k` or Arrow Keys: Navigate up/down
  * `h`/`l` or Backspace/Enter: Parent directory / Enter directory
  * `gg` / `G`: Jump to top / bottom
  * `v`: Visual selection mode for multi-select
  * `/`: Fuzzy filter mode
  * `:`: Command mode
  * `Esc`: Return to normal mode
* **Mouse Support:**
  * Click to navigate
  * Right-click context menu (Open, Copy, Cut, Paste, Rename, Delete, Properties)
  * Drag & Drop files from external applications
* **Browser-style History:** Alt+Left/Right for Back/Forward
* **Breadcrumb Navigation:** Clickable path segments in top bar
* **Directory Selection Memory:** Remembers last selected item per directory

### **File Operations**
* **Clipboard Operations:**
  * `y`: Yank/Copy selected files
  * `x`: Cut selected files
  * `p`: Paste clipboard contents
* **File Management:**
  * `d`: Delete with confirmation prompt
  * `r`: Rename with inline modal
* **Visual Multi-Select:** Select multiple files for batch operations
* **Feedback System:** Info and error message toasts in bottom bar

### **Search & Filtering**
* **Fuzzy Filter:** Press `/` to filter current view instantly
* **Enter to Finalize:** Search finalizes on Enter, allowing navigation in filtered results
* **Hidden Files Toggle:** `.` key or checkbox to show/hide hidden files

### **Performance & Architecture**
* **Async I/O:** Dedicated worker thread for filesystem operations
* **Non-blocking UI:** Spinners and loading states for smooth experience
* **File System Watcher:** Auto-refresh when files change externally
* **Debounced Loading:** 200ms delay to prevent stuttering during fast scrolling
* **Selection Auto-Scroll:** Selected items automatically scroll into view

### **Preview Capabilities**
* **Syntax Highlighting:** Basic syntax highlighting for code files (Rust, Python, JS, TS, TOML, JSON)
* **Image Preview:** Async loading for PNG, JPG, WEBP, GIF, BMP
* **Directory Preview:** Shows directory contents in preview pane with clickable navigation
* **Binary Detection:** Safe handling of binary files
* **Preview Status:** PDF placeholder ("not supported"), binary file indicators

### **Visual & Icons**
* **File Type Icons:** Emoji-based icons for different file types
* **Extensible Icon System:** Easy to customize icon mappings
* **Visual Feedback:** Cut files dimmed, multi-selected files highlighted
* **Drag & Drop Overlay:** Visual indicator when dragging files over the window

## **Planned Enhancements**

### **Icon System Improvements**
* [x] Consistent icon sizing across all panes (14pt)
* [ ] Nerd Font support for professional icon rendering
* [ ] Custom icon themes
* [ ] Icon size configuration option

### **Enhanced Syntax Highlighting**
* [ ] Full syntax highlighting library integration (syntect or tree-sitter)
* [ ] Language auto-detection
* [ ] Configurable color schemes
* [ ] Line numbers in code preview

### **Extended Preview Support**
* [ ] PDF preview integration
* [ ] Video thumbnail generation
* [ ] Audio file metadata display
* [ ] Archive contents preview (ZIP, TAR, etc.)
* [ ] Markdown rendering
* [ ] HTML preview

### **Hotkey & Keybinding Extensions**
* [ ] Configurable keybindings
* [ ] Macro recording and playback
* [ ] Custom command aliases
* [ ] Bookmark system (jump to favorite directories)
* [ ] Quick navigation marks

### **Advanced Features**
* [ ] Multiple workspace tabs
* [ ] Split panes for side-by-side file management
* [ ] Bulk rename operations
* [ ] File search across directories
* [ ] Plugin system (Lua or Wasm)
* [ ] Embedded terminal pane
* [ ] Git integration indicators
* [ ] Network/remote file system support

## **Command Mode Commands**

* `:q` or `:quit` - Exit application
* `:mkdir <name>` - Create new directory
* `:touch <name>` - Create new file

## **Building & Running**

```bash
cargo build --release
cargo run
```

**Requirements:** Rust 1.70+, modern graphics drivers supporting egui/wgpu
