mod app;
mod importer;
mod ftc_interpreter;
mod java_interpreter;
mod renderer3d;
mod step_importer;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("FTC Code Visualizer")
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([900.0, 600.0]),
        depth_buffer: 24,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    eframe::run_native(
        "FTC Code Visualizer",
        options,
        Box::new(|cc| Ok(Box::new(app::VisualizerApp::new(cc)))),
    )
}
