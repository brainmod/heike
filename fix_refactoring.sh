#!/bin/bash
# Script to fix all field accesses after state struct refactoring

set -e

echo "Starting refactoring fixes..."

# Function to apply fixes to a single file
fix_file() {
    local file=$1
    echo "Processing $file..."

    # Create backup
    cp "$file" "$file.bak"

    # Use perl for complex multi-line replacements
    perl -i -pe '
        # Fix mode assignments (self.mode = AppMode::X -> self.mode.set_mode(AppMode::X))
        # This needs to handle multi-line cases and various AppMode variants
        s/self\.mode\s*=\s*AppMode::Normal(?!\w)/self.mode.set_mode(AppMode::Normal)/g;
        s/self\.mode\s*=\s*AppMode::Visual(?!\w)/self.mode.set_mode(AppMode::Visual)/g;
        s/self\.mode\s*=\s*AppMode::Filtering(?!\w)/self.mode.set_mode(AppMode::Filtering)/g;
        s/self\.mode\s*=\s*AppMode::Command(?!\w)/self.mode.set_mode(AppMode::Command)/g;
        s/self\.mode\s*=\s*AppMode::Help(?!\w)/self.mode.set_mode(AppMode::Help)/g;
        s/self\.mode\s*=\s*AppMode::Rename(?!\w)/self.mode.set_mode(AppMode::Rename)/g;
        s/self\.mode\s*=\s*AppMode::DeleteConfirm(?!\w)/self.mode.set_mode(AppMode::DeleteConfirm)/g;
        s/self\.mode\s*=\s*AppMode::SearchInput(?!\w)/self.mode.set_mode(AppMode::SearchInput)/g;

        # Fix AppMode::SearchResults assignments
        s/self\.mode\s*=\s*AppMode::SearchResults/self.mode.set_mode(AppMode::SearchResults/g;

        # Fix mode comparisons and reads (self.mode == -> self.mode.mode ==)
        s/self\.mode\s*==\s*AppMode/self.mode.mode == AppMode/g;
        s/self\.mode\s*!=\s*AppMode/self.mode.mode != AppMode/g;

        # Fix mode in match expressions
        s/match\s+self\.mode\s*\{/match self.mode.mode {/g;
        s/match\s+&self\.mode\s*\{/match \&self.mode.mode {/g;

        # Fix mode in if let expressions
        s/if let AppMode::SearchResults\s*\{(.*?)\}\s*=\s*self\.mode/if let AppMode::SearchResults {$1} = self.mode.mode/g;
        s/if let AppMode::SearchResults\s*\{(.*?)\}\s*=\s*&self\.mode/if let AppMode::SearchResults {$1} = \&self.mode.mode/g;

        # Fix mode in matches! macro
        s/matches!\(\s*self\.mode\s*,/matches!(self.mode.mode,/g;

        # Navigation state fields
        s/self\.current_path\b/self.navigation.current_path/g;
        s/self\.history\b/self.navigation.history/g;
        s/self\.history_index\b/self.navigation.history_index/g;
        s/self\.pending_selection_path\b/self.navigation.pending_selection_path/g;

        # Selection state fields
        s/self\.selected_index\b/self.selection.selected_index/g;
        s/self\.multi_selection\b/self.selection.multi_selection/g;
        s/self\.directory_selections\b/self.selection.directory_selections/g;
        s/self\.last_selection_change\b/self.selection.last_selection_change/g;
        s/self\.disable_autoscroll\b/self.selection.disable_autoscroll/g;
        s/self\.last_g_press\b/self.selection.last_g_press/g;

        # Entry state fields
        s/self\.all_entries\b/self.entries.all_entries/g;
        s/self\.visible_entries\b/self.entries.visible_entries/g;
        s/self\.parent_entries\b/self.entries.parent_entries/g;

        # UI state fields
        s/self\.show_hidden\b/self.ui.show_hidden/g;
        s/self\.theme\b/self.ui.theme/g;
        s/self\.sort_options\b/self.ui.sort_options/g;
        s/self\.error_message\b/self.ui.error_message/g;
        s/self\.info_message\b/self.ui.info_message/g;
        s/self\.panel_widths\b/self.ui.panel_widths/g;
        s/self\.is_loading\b/self.ui.is_loading/g;
        s/self\.search_query\b/self.ui.search_query/g;
        s/self\.search_options\b/self.ui.search_options/g;
        s/self\.search_in_progress\b/self.ui.search_in_progress/g;
        s/self\.search_file_count\b/self.ui.search_file_count/g;

        # Mode state fields (other than mode itself)
        s/self\.command_buffer\b/self.mode.command_buffer/g;
        s/self\.focus_input\b/self.mode.focus_input/g;
    ' "$file"

    echo "✓ Fixed $file"
}

# Fix all files
fix_file "/home/user/heike/src/app.rs"
fix_file "/home/user/heike/src/input.rs"
fix_file "/home/user/heike/src/view/panels.rs"
fix_file "/home/user/heike/src/view/modals.rs"

echo ""
echo "All files processed!"
echo "Backups saved with .bak extension"
echo ""
echo "Now attempting to compile to verify fixes..."
