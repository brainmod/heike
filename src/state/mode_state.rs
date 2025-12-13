// Mode state - application modal and input state
use crate::state::AppMode;

pub struct ModeState {
    pub mode: AppMode,
    pub command_buffer: String,
    pub focus_input: bool,
}

impl ModeState {
    pub fn new() -> Self {
        Self {
            mode: AppMode::Normal,
            command_buffer: String::new(),
            focus_input: false,
        }
    }

    pub fn set_mode(&mut self, mode: AppMode) {
        self.mode = mode;
    }

    pub fn set_input_focus(&mut self, focus: bool) {
        self.focus_input = focus;
    }

    pub fn clear_buffer(&mut self) {
        self.command_buffer.clear();
        self.focus_input = false;
    }
}
