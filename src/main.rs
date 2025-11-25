mod app;
mod io;
mod message;
mod model;
mod style;
mod subscription;

use app::Heike;
use iced::{font, Size, Task};
use crate::message::Message;

pub const FONT_BYTES: &[u8] = include_bytes!("../assets/JetBrainsMonoNerdFont-Regular.ttf");
pub const FONT_NAME: &str = "JetBrainsMonoNerdFont";

fn main() -> iced::Result {
    iced::application("Heike", Heike::update, Heike::view)
        .subscription(Heike::subscription)
        .theme(Heike::theme)
        .window_size(Size::new(1200.0, 800.0))
        .run_with(|| {
            let (app, initial_task) = Heike::new();
            let font_loading_task = font::load(FONT_BYTES)
                .map(|res| Message::FontLoaded(res.map(|_| font::Family::Name(FONT_NAME))));
            (app, Task::batch([initial_task, font_loading_task]))
        })
}
