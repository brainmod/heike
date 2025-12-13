mod app;
mod config;
mod entry;
mod input;
mod io;
mod state;
mod style;
mod view;

use app::Heike;
use config::Config;
use eframe::egui;

fn main() -> eframe::Result<()> {
    // Load the app icon
    let icon_bytes = include_bytes!("../assets/heike_icon.png");
    let icon_image = image::load_from_memory(icon_bytes)
        .expect("Failed to load icon")
        .to_rgba8();
    let (icon_width, icon_height) = icon_image.dimensions();
    let icon_data = egui::IconData {
        rgba: icon_image.into_raw(),
        width: icon_width,
        height: icon_height,
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 700.0])
            .with_title("Heike")
            .with_icon(icon_data)
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        "Heike",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);

            // Load configuration
            let config = Config::load();

            // Configure fonts to use bundled Nerd Font for icon rendering
            let mut fonts = egui::FontDefinitions::default();

            // Use bundled JetBrainsMono Nerd Font
            let nerd_font_data = include_bytes!("../assets/JetBrainsMonoNerdFont-Regular.ttf");
            fonts.font_data.insert(
                "nerd_font".to_owned(),
                egui::FontData::from_static(nerd_font_data).into(),
            );

            // Add to proportional and monospace families (prioritize Nerd Font)
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "nerd_font".to_owned());

            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .insert(0, "nerd_font".to_owned());

            cc.egui_ctx.set_fonts(fonts);

            Ok(Box::new(Heike::new(cc.egui_ctx.clone(), config)))
        }),
    )
}
