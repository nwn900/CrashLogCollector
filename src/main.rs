use crashlog_collector::app::CrashLogCollectorApp;
use eframe::egui::{IconData, ViewportBuilder};

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_inner_size([860.0, 680.0])
            .with_min_inner_size([760.0, 560.0])
            .with_icon(load_app_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "CrashLog Collector",
        native_options,
        Box::new(|cc| Ok(Box::new(CrashLogCollectorApp::new(cc)))),
    )
}

fn load_app_icon() -> IconData {
    let icon_bytes = include_bytes!("../1.0/clc.png");
    let image = image::load_from_memory(icon_bytes)
        .expect("app icon PNG should decode")
        .into_rgba8();
    let (width, height) = image.dimensions();

    IconData {
        rgba: image.into_raw(),
        width,
        height,
    }
}
