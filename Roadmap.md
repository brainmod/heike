Rusty Yazi Roadmap

Phases 1-5 (Completed) ✅

Miller Columns Layout

Vim Navigation & Async I/O

Previews (Image/Text)

Filtering & Visual Selection

Icons & History

Phase 6: File Manipulation (The "Manager" Update)

Without this, it's just a file viewer.

[ ] Clipboard State:

Implement internal state to track paths marked for Copy or Cut.

Visual indicators for Cut (dimmed) vs Copy.

[ ] Operations:

y: Yank (Copy) selection to clipboard.

x: Cut selection to clipboard.

p: Paste clipboard to current directory.

d: Delete selection (with Confirmation Modal).

r: Rename selected file (with Input Modal).

[ ] Recursive Logic:

Ensure Copy/Delete works on directories (basic implementation).

Phase 7: Advanced Previews & Polish

[ ] PDF Preview: Render first page as image.

[ ] Syntax Highlighting: Better colorization.

[ ] Theme Support: Configurable colors.