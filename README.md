# **Heike (formerly Rusty Yazi)**

**Origin:** Named after the *Heikegani* (平家蟹), a species of crab native to Japan with a shell that bears the pattern of a human face, often said to resemble the face of an angry samurai.  
**Philosophy:** Heike is a GUI spiritual successor to the terminal file manager **Yazi**. It marries the speed and keyboard-centric efficiency of a TUI with the rich media capabilities and distinct visual layout of a modern GUI (using egui).

## **Project Status: Active Prototype**

**Current Version:** 0.6.0 (The "Manager" Update)

## **Phase 1: Foundation & Layout (Completed) ✅**

* **Miller Columns:** 3-pane layout (Parent \-\> Current \-\> Preview).  
* **Visuals:** Resizable SidePanels vs CentralPanel.  
* **Context:** Parent directory is dimmed/grayed out; Current is bright/white.

## **Phase 2: Navigation & Input (Completed) ✅**

* **Vim Bindings:** j/k (nav), h/l (parent/enter), gg/G (top/bottom).  
* **History:** Browser-style Back/Forward (Alt+Left/Right).  
* **Breadcrumbs:** Clickable path segments in the top bar.  
* **Search:** Basic path entry validation.

## **Phase 3: Media & Previews (Completed) ✅**

* **Image Preview:** Async loading of png, jpg, webp.  
* **Code Preview:** Syntax-highlighting (lite) for rs, py, js, etc.  
* **Safety:** Binary file detection and PDF "not supported" stubs.  
* **Performance:** Debounced loading (200ms delay) to prevent stuttering during fast scrolling.

## **Phase 4: Architecture (Completed) ✅**

* **Async I/O:** Dedicated worker thread for filesystem operations.  
* **Channels:** mpsc communication between UI and backend.  
* **Loading States:** Spinners and non-blocking UI updates.

## **Phase 5: Power User Features (Completed) ✅**

* **Modes:** State machine for Normal, Visual, Command, Filtering.  
* **Fuzzy Filter:** Press / to filter current view instantly.  
* **Visual Mode:** Press v to select multiple files for batch operations.  
* **Icons:** Nerd-font style emoji icons based on file extension.

## **Phase 6: File Management (Current) ✅**

* **Clipboard:** Internal Copy/Cut state (HashSet\<PathBuf\>).  
* **Operations:**  
  * y (Yank/Copy)  
  * x (Cut)  
  * p (Paste)  
  * d (Delete with confirmation)  
  * r (Rename with modal)  
* **Feedback:** "Info" and "Error" message toast system in the bottom bar.

## **Phase 7: The "Polished" Experience (In Progress)**

*Focus: UX refinement and closing the gap with native file managers.*

* \[x\] **App Icon Integration:**
  * Use the new icon.svg (convert to .ico/.png) for the window title bar.
* \[x\] **Drag & Drop:**
  * Allow dragging files from Heike to external apps (Explorer/Finder).
  * Allow dropping files into Heike to copy them.
* \[ \] **Context Menu:**
  * Right-click menu for mouse users (Open, Copy, Rename, Properties).
* \[ \] **Watcher:**
  * Integrate notify crate to auto-refresh when files are changed externally.
* \[ \] **Theme System:**
  * Allow toggling Light/Dark mode.
  * Configurable accent colors (currently hardcoded "Rust Orange" / "Light Blue").

## **Phase 8: Advanced Features (Long Term)**

* \[ \] **Tabs:** Multiple workspace tabs.  
* \[ \] **Plugin System:** Lua or Wasm plugin support (ambitious).  
* \[ \] **Terminal Integration:** Embedded terminal pane (using portable-pty).
