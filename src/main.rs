mod app;
mod io;
mod message;
mod model;
mod style;
mod subscription;

use app::Heike;
use iced::Size;

fn main() -> iced::Result {
    iced::application("Heike", Heike::update, Heike::view)
        .subscription(Heike::subscription)
        .theme(Heike::theme)
        .window_size(Size::new(1200.0, 800.0))
        .run_with(Heike::new)
}
