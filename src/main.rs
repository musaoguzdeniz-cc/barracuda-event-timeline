#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod events;
mod theme;
mod ui;
mod windows;

fn main() -> eframe::Result {
    eframe::run_native(
        "Barracuda Windows Olay Zaman Çizelgesi",
        eframe::NativeOptions {
            viewport: eframe::egui::ViewportBuilder::default()
                .with_inner_size([1180.0, 760.0])
                .with_min_inner_size([880.0, 560.0]),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(ui::Timeline::new(cc)))),
    )
}
