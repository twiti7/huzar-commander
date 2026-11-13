#![windows_subsystem = "windows"]

use huzar_commander::app::CommanderApp;

fn load_app_icon() -> egui::IconData {
    let png_bytes = include_bytes!("../assets/huzar_icon.png");
    let img = image::load_from_memory(png_bytes)
        .expect("Nie udało się załadować ikony programu")
        .into_rgba8();
    let (width, height) = img.dimensions();
    egui::IconData {
        rgba: img.into_raw(),
        width,
        height,
    }
}

fn main() -> eframe::Result<()> {
    let icon = load_app_icon();
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Huzar Commander")
            .with_icon(icon)
            .with_inner_size([1100.0, 700.0])
            .with_min_inner_size([800.0, 500.0])
            .with_maximized(true),
        ..Default::default()
    };

    eframe::run_native(
        "Huzar Commander",
        native_options,
        Box::new(|cc| Ok(Box::new(CommanderApp::new(cc)))),
    )
}
