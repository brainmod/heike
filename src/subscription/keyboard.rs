use crate::message::Message;
use crate::model::Mode;
use iced::event::{self, Event};
use iced::keyboard::{self, key::Named, Key, Modifiers};
use iced::Subscription;

pub fn keyboard_subscription(_mode: Mode) -> Subscription<Message> {
    event::listen_with(|event, _status, _id| match event {
        Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
            Some(Message::KeyPressed(key, modifiers))
        }
        _ => None,
    })
}

pub fn handle_key(key: Key, modifiers: Modifiers, current_mode: &Mode) -> Option<Message> {
    match current_mode {
        Mode::Normal | Mode::Visual => handle_normal_mode(key, modifiers),
        Mode::Filter | Mode::Command | Mode::Rename | Mode::Search => {
            handle_input_mode(key, modifiers)
        }
        Mode::SearchResults(_) => handle_search_results_mode(key, modifiers),
        Mode::Confirm(_) => handle_confirm_mode(key, modifiers),
        Mode::GPrefix => handle_g_prefix(key, modifiers),
    }
}

fn handle_normal_mode(key: Key, modifiers: Modifiers) -> Option<Message> {
    match (key, modifiers) {
        // Vim navigation
        (Key::Character(c), m) if m.is_empty() => match c.as_str() {
            "j" => Some(Message::SelectDelta(1)),
            "k" => Some(Message::SelectDelta(-1)),
            "h" => Some(Message::NavigateUp),
            "l" => Some(Message::InputSubmit),
            "g" => Some(Message::SetMode(Mode::GPrefix)),
            "G" => Some(Message::SelectLast),
            "y" => Some(Message::Yank),
            "x" => Some(Message::Cut),
            "p" => Some(Message::Paste),
            "d" => Some(Message::Delete),
            "r" => Some(Message::SetMode(Mode::Rename)),
            "v" => Some(Message::ToggleMultiSelect),
            "/" => Some(Message::SetMode(Mode::Filter)),
            ":" => Some(Message::SetMode(Mode::Command)),
            "." => Some(Message::ToggleHidden),
            "n" => Some(Message::NextSearchResult),
            "N" => Some(Message::PrevSearchResult),
            _ => None,
        },
        // Shift+S for search
        (Key::Character(c), m) if m.shift() && c.as_str() == "s" => {
            Some(Message::SetMode(Mode::Search))
        }
        // Alt+Arrow for history (must come before general arrow keys)
        (Key::Named(Named::ArrowLeft), m) if m.alt() => Some(Message::NavigateBack),
        (Key::Named(Named::ArrowRight), m) if m.alt() => Some(Message::NavigateForward),
        // Arrow keys
        (Key::Named(Named::ArrowDown), _) => Some(Message::SelectDelta(1)),
        (Key::Named(Named::ArrowUp), _) => Some(Message::SelectDelta(-1)),
        (Key::Named(Named::ArrowLeft), _) => Some(Message::NavigateUp),
        (Key::Named(Named::ArrowRight), _) => Some(Message::InputSubmit),
        (Key::Named(Named::Enter), _) => Some(Message::InputSubmit),
        (Key::Named(Named::Escape), _) => Some(Message::ClearMultiSelect),
        (Key::Named(Named::Backspace), m) if m.is_empty() => Some(Message::NavigateUp),
        _ => None,
    }
}

fn handle_input_mode(key: Key, _modifiers: Modifiers) -> Option<Message> {
    match key {
        Key::Named(Named::Enter) => Some(Message::InputSubmit),
        Key::Named(Named::Escape) => Some(Message::CancelInput),
        _ => None, // Input changes handled by text_input widget
    }
}

fn handle_search_results_mode(key: Key, modifiers: Modifiers) -> Option<Message> {
    match (key, modifiers) {
        (Key::Character(c), m) if m.is_empty() => match c.as_str() {
            "j" | "n" => Some(Message::NextSearchResult),
            "k" | "N" => Some(Message::PrevSearchResult),
            _ => None,
        },
        (Key::Named(Named::ArrowDown), _) => Some(Message::NextSearchResult),
        (Key::Named(Named::ArrowUp), _) => Some(Message::PrevSearchResult),
        (Key::Named(Named::Enter), _) => Some(Message::InputSubmit),
        (Key::Named(Named::Escape), _) => Some(Message::SetMode(Mode::Normal)),
        _ => None,
    }
}

fn handle_confirm_mode(key: Key, _modifiers: Modifiers) -> Option<Message> {
    match key {
        Key::Character(c) if c.as_str() == "y" => Some(Message::ConfirmDelete),
        Key::Named(Named::Enter) => Some(Message::ConfirmDelete),
        Key::Named(Named::Escape) => Some(Message::SetMode(Mode::Normal)),
        Key::Character(c) if c.as_str() == "n" => Some(Message::SetMode(Mode::Normal)),
        _ => None,
    }
}

fn handle_g_prefix(key: Key, _modifiers: Modifiers) -> Option<Message> {
    match key {
        Key::Character(c) if c.as_str() == "g" => Some(Message::SelectFirst),
        Key::Named(Named::Escape) => Some(Message::SetMode(Mode::Normal)),
        _ => Some(Message::SetMode(Mode::Normal)), // Reset on any other key
    }
}
