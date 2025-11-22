# **Heike**

**Origin:** Named after the *Heikegani* (平家蟹), a species of crab native to Japan with a shell that bears the pattern of a human face, often said to resemble the face of an angry samurai.
**Philosophy:** Heike is a GUI spiritual successor to the terminal file manager **Yazi**. It marries the speed and keyboard-centric efficiency of a TUI with the rich media capabilities and distinct visual layout of a modern GUI (using egui).

## **Project Status: Active Prototype**

**Current Version:** 0.6.0 (The "Manager" Update)

### **Recent Updates** (2025-11-22)

#### **Layout & Interaction Improvements**
* ✅ **Fixed Column Widths:** Set reasonable defaults and disabled auto-resize based on content for more stable layout
* ✅ **Smart Autoscroll:** Autoscroll now temporarily disables when user manually scrolls via mouse wheel or scroll bar
* ✅ **Wrapped Navigation:** Arrow keys and hjkl now wrap around at top/bottom of file list
* ✅ **Double-Click to Open:** Changed file/directory opening to require Enter or double-click (single-click now only selects)
* ✅ **Arrow Key Bindings:** Arrow keys bound to same actions as hjkl (Up/Down = j/k navigation)

#### **Preview & Display**
* ✅ **Comprehensive Preview Support:** Native file preview without OS dependencies
* ✅ **Enhanced Syntax Highlighting:** Professional code highlighting using syntect (50+ languages)
* ✅ **Markdown Rendering:** Native markdown preview with formatted headings and code blocks
* ✅ **Archive Preview:** ZIP/TAR/GZ archive content listing
* ✅ **PDF Support:** PDF metadata and text extraction preview
* ✅ **Audio Metadata:** ID3 tag reading for MP3 files
* ✅ **Hex Viewer:** Binary file preview with hex dump display
* ✅ **Nerd Font Integration:** Icons now use Nerd Font glyphs for crisp, professional rendering
* ✅ **Icon Display Improvements:** Enhanced icon rendering with consistent sizing (14pt) across all panes
* ✅ **Navigation Fix:** Fixed pane navigation to work as proper Miller Columns - clicking left/right panes shifts content appropriately
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
  * Click to select
  * Double-click to open/navigate
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
* **Smart Auto-Scroll:** Selected items automatically scroll into view, but intelligently disables when user manually scrolls

### **Preview Capabilities**
**All preview features work natively without OS dependencies - pure Rust libraries only!**

* **Enhanced Syntax Highlighting:** Professional code highlighting using syntect library
  * Support for 50+ programming languages (Rust, Python, JS/TS, C/C++, Java, Go, Ruby, PHP, Swift, Kotlin, Scala, and many more)
  * Theme-aware highlighting (adapts to light/dark mode)
  * Full file content preview (no size limits)
  * Smart syntax detection by file extension and content
* **Markdown Rendering:** Native markdown preview with proper formatting
  * Heading hierarchy (H1-H6) with size differentiation
  * Code block and inline code formatting
  * Paragraph spacing and text wrapping
* **Image Preview:** Async loading for PNG, JPG, JPEG, GIF, WEBP, BMP, SVG, ICO
* **PDF Preview:** Native PDF support without OS dependencies
  * PDF metadata extraction (title, author, page count)
  * Text content extraction and preview
  * 2000 character preview limit with truncation indicator
* **Archive Preview:** Native archive content listing
  * ZIP archive support with file listing
  * TAR/GZ/TGZ support with decompression
  * Shows file names, sizes, and directory structure
  * Visual file/folder icons in archive listing
* **Audio Metadata:** MP3 ID3 tag reading
  * Title, artist, album, year, genre display
  * Album art detection and size info
  * Framework ready for FLAC, OGG, M4A, WAV
* **Binary File Viewer:** Hex dump display for unknown file types
  * Offset + Hex + ASCII column layout
  * 512-byte preview window
  * Proper byte alignment and formatting
* **Directory Preview:** Shows directory contents in preview pane with clickable navigation
* **Smart Fallback System:** Text → Syntax Highlighting → Hex View

### **Visual & Icons**
* **File Type Icons:** Nerd Font glyphs for professional icon rendering (50+ file types supported)
* **Bundled Font:** JetBrainsMono Nerd Font included - no external dependencies
* **Extensible Icon System:** Easy to customize icon mappings
* **Visual Feedback:** Cut files dimmed, multi-selected files highlighted
* **Drag & Drop Overlay:** Visual indicator when dragging files over the window

## **Planned Enhancements**

### **Icon System Improvements**
* [x] Consistent icon sizing across all panes (14pt)
* [x] Nerd Font support for professional icon rendering
* [ ] Custom icon themes
* [ ] Icon size configuration option

### **Enhanced Syntax Highlighting**
* [x] Full syntax highlighting library integration (syntect)
* [x] Language auto-detection
* [x] Theme-aware color schemes (light/dark mode)
* [ ] Line numbers in code preview
* [ ] Configurable custom color schemes

### **Extended Preview Support**
* [x] PDF preview integration (metadata + text extraction)
* [x] Audio file metadata display (MP3 ID3 tags)
* [x] Archive contents preview (ZIP, TAR, GZ, TGZ)
* [x] Markdown rendering
* [x] Binary hex viewer
* [ ] Video thumbnail generation
* [ ] PDF page rendering (currently text-only)
* [ ] HTML preview with rendering
* [ ] Office document preview (DOCX, XLSX, PPTX)

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

**Requirements:**
* Rust 1.70+
* Modern graphics drivers supporting egui/wgpu

**Note:** JetBrainsMono Nerd Font is bundled with the application, so icons work out of the box without any additional setup!
