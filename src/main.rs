#![windows_subsystem = "windows"]

use aeropdf::app::AeroPdfApp;
use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let initial_file = args.get(1).map(PathBuf::from);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("AeroPDF")
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(true)
            .with_inner_size([860.0, 720.0])
            .with_min_inner_size([400.0, 300.0]),
        ..Default::default()
    };

    eframe::run_native(
        "AeroPDF",
        options,
        Box::new(|_cc| Ok(Box::new(AeroPdfApp::new(initial_file)))),
    )
}
