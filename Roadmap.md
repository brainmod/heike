Rusty Yazi Roadmap

Phase 1: The "Miller Column" Layout (Visual Structure) ✅

Yazi uses a 3-pane layout: Parent Directory (Left) -> Current Directory (Center) -> Preview (Right).

[x] Refactor Layout: Split the central panel into three distinct sections using egui::SidePanel or ui.columns(3).

[x] Parent Pane (Left):

Display the contents of current_path.parent().

Render it with lower opacity or "dimmed" text to indicate it is context.

[x] Current Pane (Center):

Keep the current Table logic here.

Ensure this pane has keyboard focus by default.

[x] Preview Pane (Right):

Create a placeholder area that updates whenever selected_index changes.

Initially just show the file name and basic metadata (size, permissions).

Phase 2: The "Yazi Feel" (Vim Navigation) ✅

Yazi is keyboard-centric. Mouse support is secondary.

[x] Vim Bindings:

Map j / k to Arrow Down / Arrow Up.

Map h to Go to Parent (Left).

Map l to Enter Directory (Right) or Open File.

Map gg to Top, G to Bottom.

[x] Breadcrumb Navigation:

Replace the text edit path bar with clickable breadcrumbs (e.g., ~ > Code > Rust > rusty_yazi).

[x] Command Palette:

Implement a pop-up modal (triggered by :) for commands like quit, mkdir, touch.

Phase 3: High-Performance Previews ✅

One of Yazi's best features is checking file content without opening it.

[x] Image Preview:

Integrate egui_extras image loaders.

Load images asynchronously to prevent UI freeze.

Resize images to fit the right pane while maintaining aspect ratio.

[x] Text/Code Preview:

Read the first N lines of the selected text file.

(Optional) Add syntax highlighting (using syntect crate later).

[x] Binary/Unknown:

Show a hex dump or a "Binary File" placeholder for non-text files.

Phase 4: Architecture Overhaul (Async I/O) ✅

Currently, fs::read_dir runs on the UI thread. Large folders will freeze the app.

[x] The Backend Thread:

Spawn a dedicated thread (or Tokio runtime) for file operations.

Use std::sync::mpsc (or crossbeam) to send Vec<FileEntry> from Backend to Frontend.

[x] Loading States:

Show a spinner while the directory is being read.

[x] Debounced Previews:

Don't load the preview instantly if the user is scrolling fast. Wait for the selection to settle for 200ms.

Phase 5: "Power User" Features

[ ] Fuzzy Finding:

Trigger with f or /.

Pop up a search box that filters the current list in real-time.

Use a fuzzy matching crate like nucleo-matcher or fuzzy-matcher.

[ ] Selection Mode:

Allow selecting multiple files (visual mode v).

Batch operations: Bulk Delete, Bulk Move.

[ ] File Icons:

Integrate a Nerd Font or an icon crate to show specific icons for .rs, .toml, .js, etc.