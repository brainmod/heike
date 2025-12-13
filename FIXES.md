# Heike: Recommended Fixes & Improvements

## Priority 1: Layout Gap Fix (Strip-Based Layout)

Replace SidePanel approach with `egui_extras::Strip` to eliminate black gap on maximize.

### Current Problem
```
┌─────────┬─────────────────────────┬──████████──┬─────────┐
│ Parent  │       Current           │ BLACK GAP  │ Preview │
└─────────┴─────────────────────────┴──████████──┴─────────┘
```

Three independent panel calculations desync during window resize.

### Solution

**Add to `Heike` struct:**
```rust
struct Heike {
    // ... existing fields
    panel_widths: [f32; 2],      // [parent, preview] - current is remainder
    dragging_divider: Option<usize>,
    last_screen_size: egui::Vec2,
}

// In new()
panel_widths: [200.0, 350.0],
dragging_divider: None,
last_screen_size: egui::Vec2::ZERO,
```

**Replace all three panel declarations with:**
```rust
// Remove:
// - egui::SidePanel::left("parent_panel")...
// - egui::SidePanel::right("preview_panel")...
// - egui::CentralPanel::default()... (the main one)

// Add single CentralPanel with Strip:
egui::CentralPanel::default().show(ctx, |ui| {
    use egui_extras::{StripBuilder, Size};
    
    StripBuilder::new(ui)
        .size(Size::exact(self.panel_widths[0]).at_least(100.0))
        .size(Size::exact(4.0))      // Left divider
        .size(Size::remainder())      // Current - guarantees no gap
        .size(Size::exact(4.0))      // Right divider
        .size(Size::exact(self.panel_widths[1]).at_least(150.0))
        .horizontal(|mut strip| {
            strip.cell(|ui| self.render_parent_pane(ui));
            strip.cell(|ui| self.render_divider(ui, 0));
            strip.cell(|ui| self.render_current_pane(ui));
            strip.cell(|ui| self.render_divider(ui, 1));
            strip.cell(|ui| self.render_preview_pane(ui));
        });
});
```

**Add divider handler:**
```rust
fn render_divider(&mut self, ui: &mut egui::Ui, index: usize) {
    let response = ui.allocate_response(
        ui.available_size(),
        egui::Sense::drag()
    );
    
    let color = if response.hovered() || response.dragged() {
        ui.visuals().widgets.active.bg_fill
    } else {
        egui::Color32::from_gray(60)
    };
    ui.painter().rect_filled(response.rect, 0.0, color);
    
    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    
    if response.dragged() {
        let delta = response.drag_delta().x;
        match index {
            0 => self.panel_widths[0] = (self.panel_widths[0] + delta).clamp(100.0, 400.0),
            1 => self.panel_widths[1] = (self.panel_widths[1] - delta).clamp(150.0, 500.0),
            _ => {}
        }
    }
}
```

**Refactor render methods** (extract from current `update()`):
```rust
fn render_parent_pane(&self, ui: &mut egui::Ui) { /* move parent panel content */ }
fn render_current_pane(&mut self, ui: &mut egui::Ui) { /* move central panel content */ }
fn render_preview_pane(&self, ui: &mut egui::Ui) { /* move preview panel content */ }
```

---

## Priority 2: Text Overflow Clipping

Prevents long filenames from forcing panel expansion.

### All Table Columns
```rust
// Find all instances of:
.column(Column::remainder())

// Replace with:
.column(Column::remainder().clip(true))
```

**Affected locations:**
- Parent pane table (~line 1113)
- Current pane table (~line 1165)
- Preview directory table (~line 878)
- Archive preview table (~line 945)
- Search results table

### Truncated Labels with Ellipsis
```rust
// Add helper function
fn truncated_label(ui: &mut egui::Ui, text: &str, max_width: f32) -> egui::Response {
    let available = max_width.min(ui.available_width());
    let font_id = egui::TextStyle::Body.resolve(ui.style());
    let mut job = egui::text::LayoutJob::single_section(
        text.to_string(),
        egui::TextFormat::simple(font_id, ui.visuals().text_color())
    );
    job.wrap = egui::text::TextWrapping {
        max_width: available,
        max_rows: 1,
        break_anywhere: false,
        overflow_character: Some('…'),
    };
    ui.label(job)
}

// Usage in table name columns:
row.col(|ui| {
    truncated_label(ui, &entry.name, ui.available_width());
});
```

---

## Priority 3: ScrollArea Constraints

Prevent content overflow in all scrollable regions.

```rust
// Pattern for all ScrollAreas:
egui::ScrollArea::vertical()
    .id_salt("unique_id")
    .auto_shrink([false, false])      // Don't collapse
    .max_height(ui.available_height()) // Respect container
    .show(ui, |ui| {
        ui.set_max_width(ui.available_width());  // Prevent horizontal expansion
        // content
    });
```

**Apply to:**
- `preview_code` ScrollArea
- `preview_md` ScrollArea
- `preview_dir` ScrollArea
- `preview_archive` ScrollArea
- `parent_scroll` ScrollArea
- `current_scroll` ScrollArea
- `search_results_scroll` ScrollArea

---

## Priority 4: Image Preview Sizing

```rust
// Current
ui.add(egui::Image::new(uri).max_width(ui.available_width()));

// Fixed
let available = ui.available_size();
ui.add(
    egui::Image::new(uri)
        .max_width(available.x)
        .max_height(available.y - 100.0)  // Reserve header space
        .maintain_aspect_ratio(true)
        .shrink_to_fit()
);
```

---

## Priority 5: Responsive Modals

```rust
// Helper for modal sizing
fn modal_width(ctx: &egui::Context) -> f32 {
    (ctx.screen_rect().width() * 0.6).clamp(300.0, 500.0)
}

fn modal_max_height(ctx: &egui::Context) -> f32 {
    ctx.screen_rect().height() * 0.8
}

// Apply to all Windows:
egui::Window::new("Help")
    .collapsible(false)
    .resizable(false)
    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
    .default_width(modal_width(ctx))
    .show(ctx, |ui| {
        ui.set_max_height(modal_max_height(ctx));
        egui::ScrollArea::vertical().show(ui, |ui| {
            // content
        });
    });
```

---

## Priority 6: Breadcrumb Overflow

```rust
// In top panel, wrap breadcrumbs in horizontal scroll
ui.horizontal(|ui| {
    // Nav buttons (fixed)
    if ui.button("⬅").clicked() { /* ... */ }
    if ui.button("➡").clicked() { /* ... */ }
    if ui.button("⬆").clicked() { /* ... */ }
    ui.add_space(10.0);
    
    // Breadcrumbs (scrollable)
    let breadcrumb_width = ui.available_width() - 180.0; // Reserve right controls
    egui::ScrollArea::horizontal()
        .id_salt("breadcrumbs")
        .max_width(breadcrumb_width)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                for component in &components {
                    // breadcrumb buttons
                }
            });
        });
    
    // Right controls in remaining space
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        // theme toggle, help, etc.
    });
});
```

---

## Code Organization: Layout Constants

```rust
// src/layout.rs (new file)
pub mod layout {
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
    pub const PREVIEW_MAX: f32 = 500.0;
    pub const PREVIEW_DEFAULT: f32 = 350.0;
    
    // Modals
    pub const MODAL_MIN_WIDTH: f32 = 300.0;
    pub const MODAL_MAX_WIDTH: f32 = 500.0;
    pub const MODAL_WIDTH_RATIO: f32 = 0.6;
    pub const MODAL_HEIGHT_RATIO: f32 = 0.8;
    
    // Timing
    pub const PREVIEW_DEBOUNCE_MS: u64 = 200;
    pub const DOUBLE_PRESS_MS: u64 = 500;  // for gg
    
    // Preview limits
    pub const HEX_PREVIEW_BYTES: usize = 512;
    pub const TEXT_PREVIEW_LIMIT: usize = 100_000;  // chars
    pub const ARCHIVE_PREVIEW_ITEMS: usize = 100;
}
```

---

## Performance: Large File Handling

### Virtual Scrolling for Code Preview
```rust
fn render_syntax_highlighted(&self, ui: &mut egui::Ui, entry: &FileEntry) {
    let content = match fs::read_to_string(&entry.path) {
        Ok(c) => c,
        Err(e) => { ui.colored_label(egui::Color32::RED, format!("Error: {}", e)); return; }
    };
    
    let lines: Vec<&str> = content.lines().collect();
    let line_height = 14.0;
    let total_height = lines.len() as f32 * line_height;
    
    egui::ScrollArea::vertical()
        .id_salt("preview_code")
        .auto_shrink([false, false])
        .show_viewport(ui, |ui, viewport| {
            ui.set_height(total_height);
            
            let start = (viewport.top() / line_height) as usize;
            let end = ((viewport.bottom() / line_height) as usize + 1).min(lines.len());
            
            // Only highlight visible lines
            let syntax = self.syntax_set
                .find_syntax_by_extension(&entry.extension)
                .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());
            let theme_name = if self.theme == Theme::Dark { "base16-ocean.dark" } else { "base16-ocean.light" };
            let theme = &self.theme_set.themes[theme_name];
            
            ui.allocate_space(egui::vec2(0.0, start as f32 * line_height)); // Spacer
            
            let mut highlighter = HighlightLines::new(syntax, theme);
            // Skip to start line (re-highlight for context)
            for line in lines.iter().take(start) {
                let _ = highlighter.highlight_line(line, &self.syntax_set);
            }
            
            let mut job = egui::text::LayoutJob::default();
            for line in lines.iter().skip(start).take(end - start) {
                if let Ok(ranges) = highlighter.highlight_line(line, &self.syntax_set) {
                    for (style, text) in ranges {
                        job.append(text, 0.0, egui::TextFormat {
                            font_id: egui::FontId::monospace(12.0),
                            color: egui::Color32::from_rgb(style.foreground.r, style.foreground.g, style.foreground.b),
                            ..Default::default()
                        });
                    }
                }
                job.append("\n", 0.0, Default::default());
            }
            ui.label(job);
        });
}
```

### Early Bail for Binary Files
```rust
fn is_likely_binary(path: &Path) -> bool {
    let mut buf = [0u8; 8192];
    if let Ok(mut f) = fs::File::open(path) {
        if let Ok(n) = std::io::Read::read(&mut f, &mut buf) {
            // Check for null bytes (binary indicator)
            return buf[..n].contains(&0);
        }
    }
    false
}

// In render_preview, before attempting text read:
if !is_likely_text_extension && is_likely_binary(&entry.path) {
    self.render_binary_info(ui, entry);
    return;
}
```

---

## Additional Recommendations

### 1. Keyboard Focus State
```rust
// Track if text input has focus to prevent hotkey conflicts
struct Heike {
    text_input_focused: bool,
}

// In handle_input:
if self.text_input_focused {
    return; // Let text input handle keys
}
```

### 2. Error Toast Auto-Dismiss
```rust
struct Heike {
    error_message: Option<(String, Instant)>,
    info_message: Option<(String, Instant)>,
}

// In update(), clear old messages:
if let Some((_, time)) = &self.error_message {
    if time.elapsed() > Duration::from_secs(5) {
        self.error_message = None;
    }
}
```

### 3. Selection Bounds Check
```rust
// After any operation that might change entries:
fn validate_selection(&mut self) {
    if let Some(idx) = self.selected_index {
        if self.visible_entries.is_empty() {
            self.selected_index = None;
        } else if idx >= self.visible_entries.len() {
            self.selected_index = Some(self.visible_entries.len() - 1);
        }
    }
}
```

### 4. Persist Settings to Disk
```rust
// Using directories crate (already in deps)
fn config_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "heike")
        .map(|d| d.config_dir().join("settings.toml"))
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Settings {
    panel_widths: [f32; 2],
    show_hidden: bool,
    theme: String,
    last_path: Option<PathBuf>,
}
```

Add `serde = { version = "1.0", features = ["derive"] }` and `toml = "0.8"` to deps.

### 5. Startup Path from Args
```rust
fn main() -> eframe::Result<()> {
    let start_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| {
            directories::UserDirs::new()
                .map(|d| d.home_dir().to_path_buf())
                .unwrap_or_default()
        });
    
    // Pass to Heike::new()
}
```

---

## Migration Checklist

- [x] Add `panel_widths`, `dragging_divider`, `last_screen_size` to struct
- [x] Extract `render_parent_pane()`, `render_current_pane()`, `render_preview_pane()`
- [x] Replace three panels with Strip layout
- [x] Add `render_divider()` method
- [x] Add `.clip(true)` to all `Column::remainder()`
- [x] Add `truncated_label()` helper
- [x] Constrain all ScrollAreas
- [x] Fix image preview sizing
- [x] Add responsive modal sizing
- [x] Wrap breadcrumbs in ScrollArea
- [x] Create `style.rs` constants module (was layout.rs)
- [x] Add binary file detection
- [x] Add message auto-dismiss
- [ ] Add settings persistence (optional)
- [ ] Add CLI path argument (optional)

### Code Organization (completed 2025-12)

- [x] Extract `src/app.rs` — Heike struct, update loop
- [x] Extract `src/entry.rs` — FileEntry struct
- [x] Extract `src/style.rs` — Theme, layout constants
- [x] Extract `src/state/` — AppMode, ClipboardOp, SearchResult, SearchOptions
- [x] Extract `src/io/` — Directory reading, search, worker thread
- [x] Extract `src/view/preview.rs` — File preview rendering
- [ ] Extract `src/input.rs` — Keyboard handling (placeholder exists)
- [ ] Extract `src/view/panels.rs` — Miller columns (placeholder exists)
- [ ] Extract `src/view/modals.rs` — Dialogs (placeholder exists)

---

## Testing Scenarios

After implementing, verify these scenarios:

1. **Maximize window** with long filename selected — no black gap
2. **Resize window** rapidly — panels stay proportional
3. **Long filename** in parent/current/preview — text clips with ellipsis
4. **Large code file** (>10k lines) — smooth scrolling
5. **Deep directory path** — breadcrumbs scroll horizontally
6. **Small window** (<800px wide) — modals fit, panels respect minimums
7. **Drag dividers** — smooth resize, respects min/max
8. **Image larger than preview pane** — scales down, maintains aspect ratio
