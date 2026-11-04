use std::time::Instant;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, TextureHandle, Vec2};
use crate::importer::{self, OpModeInfo};

const FIELD_SIZE_IN: f32 = 144.0;
const ROBOT_SIZE_IN: f32 = 18.0;

pub struct VisualizerApp {
    robot_x: f32, robot_y: f32, heading_deg: f32, drive_speed: f32,
    is_running: bool, last_frame: Instant, elapsed_seconds: f32,
    field_texture: Option<TextureHandle>, field_load_attempted: bool,
    project_name: Option<String>, op_modes: Vec<OpModeInfo>,
    selected_op_mode: Option<usize>, show_source: bool, import_status: String,
}

impl Default for VisualizerApp {
    fn default() -> Self {
        Self {
            robot_x: 24.0, robot_y: 24.0, heading_deg: 0.0, drive_speed: 24.0,
            is_running: false, last_frame: Instant::now(), elapsed_seconds: 0.0,
            field_texture: None, field_load_attempted: false, project_name: None,
            op_modes: Vec::new(), selected_op_mode: None, show_source: false,
            import_status: "No FTC project loaded.".to_owned(),
        }
    }
}

impl VisualizerApp {
    fn reset(&mut self) {
        self.robot_x = 24.0; self.robot_y = 24.0; self.heading_deg = 0.0;
        self.elapsed_seconds = 0.0; self.is_running = false; self.last_frame = Instant::now();
    }

    fn import_project(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            match importer::scan_project(&path) {
                Ok(scan) => {
                    self.project_name = Some(scan.project_name);
                    self.op_modes = scan.op_modes;
                    self.selected_op_mode = None;
                    self.import_status = format!("Scanned {} source files in {}. Found {} OpMode(s).",
                        scan.source_files, scan.source_root.display(), self.op_modes.len());
                }
                Err(error) => {
                    self.import_status = error;
                    self.project_name = None;
                    self.op_modes.clear();
                    self.selected_op_mode = None;
                }
            }
        }
    }

    fn load_field_image(&mut self, ctx: &egui::Context) {
        if self.field_load_attempted { return; }
        self.field_load_attempted = true;
        let path = std::path::Path::new("assets/biobuzz-field.png");
        match image::open(path) {
            Ok(image) => {
                let image = image.to_rgba8();
                let size = [image.width() as usize, image.height() as usize];
                let pixels = image.as_flat_samples();
                let color = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                self.field_texture = Some(ctx.load_texture("biobuzz-field", color, egui::TextureOptions::LINEAR));
            }
            Err(error) => eprintln!("Could not load {}: {error}", path.display()),
        }
    }

    fn update_simulation(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.05);
        self.last_frame = now;
        if self.is_running {
            let h = self.heading_deg.to_radians();
            self.robot_x = (self.robot_x + h.cos() * self.drive_speed * dt)
                .clamp(ROBOT_SIZE_IN / 2.0, FIELD_SIZE_IN - ROBOT_SIZE_IN / 2.0);
            self.robot_y = (self.robot_y + h.sin() * self.drive_speed * dt)
                .clamp(ROBOT_SIZE_IN / 2.0, FIELD_SIZE_IN - ROBOT_SIZE_IN / 2.0);
            self.elapsed_seconds += dt;
            ctx.request_repaint();
        }
    }

    fn draw_field(&self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let side = available.x.min(available.y).max(100.0);
        let (response, painter) = ui.allocate_painter(Vec2::splat(side), Sense::hover());
        let rect = response.rect;
        let inset = side * 0.025;
        let field = Rect::from_min_max(rect.min + Vec2::splat(inset), rect.max - Vec2::splat(inset));
        if let Some(texture) = &self.field_texture {
            painter.image(texture.id(), field, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        } else {
            painter.rect_filled(field, 0.0, Color32::from_rgb(43, 44, 46));
            painter.text(field.center(), Align2::CENTER_CENTER,
                "Field image missing\nPlace it at assets/biobuzz-field.png",
                FontId::proportional(16.0), Color32::WHITE);
        }
        let scale = field.width() / FIELD_SIZE_IN;
        let to_screen = |x: f32, y: f32| Pos2::new(field.left() + x * scale, field.bottom() - y * scale);
        let center = to_screen(self.robot_x, self.robot_y);
        let half = ROBOT_SIZE_IN * scale / 2.0;
        let robot = Rect::from_center_size(center, Vec2::splat(half * 2.0));
        painter.rect_filled(robot, 2.0, Color32::from_rgb(230, 55, 165));
        painter.rect_stroke(robot, 2.0, Stroke::new(1.5, Color32::WHITE), egui::StrokeKind::Inside);
        let h = self.heading_deg.to_radians();
        let nose = Pos2::new(center.x + h.cos() * half, center.y - h.sin() * half);
        painter.line_segment([center, nose], Stroke::new(3.0, Color32::YELLOW));
        painter.circle_filled(nose, 3.5, Color32::YELLOW);
        painter.text(rect.left_top() + Vec2::new(7.0, 7.0), Align2::LEFT_TOP,
            "BIOBUZZ FIELD", FontId::proportional(12.0), Color32::LIGHT_GRAY);
        painter.text(rect.right_bottom() - Vec2::new(7.0, 7.0), Align2::RIGHT_BOTTOM,
            "144 in × 144 in", FontId::proportional(11.0), Color32::LIGHT_GRAY);
    }
}

impl eframe::App for VisualizerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.load_field_image(ctx);
        self.update_simulation(ctx);
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("FTC Code Visualizer"); ui.separator(); ui.label("BIOBUZZ · 2026–2027");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("Simulation time: {:.1}s", self.elapsed_seconds));
                });
            });
        });
        egui::SidePanel::left("controls").resizable(true).default_width(300.0).show(ctx, |ui| {
            ui.heading("Robot"); ui.add_space(8.0); ui.label("Position (inches)");
            ui.add(egui::Slider::new(&mut self.robot_x, 0.0..=FIELD_SIZE_IN).text("X"));
            ui.add(egui::Slider::new(&mut self.robot_y, 0.0..=FIELD_SIZE_IN).text("Y"));
            ui.add(egui::Slider::new(&mut self.heading_deg, 0.0..=360.0).text("Heading").suffix("°"));
            ui.add_space(12.0); ui.separator(); ui.heading("Test simulation");
            ui.add(egui::Slider::new(&mut self.drive_speed, 0.0..=60.0).text("Drive speed").suffix(" in/s"));
            ui.horizontal(|ui| {
                if ui.button(if self.is_running { "Pause" } else { "Run" }).clicked() {
                    self.is_running = !self.is_running; self.last_frame = Instant::now();
                }
                if ui.button("Reset").clicked() { self.reset(); }
            });
            ui.add_space(12.0); ui.separator(); ui.heading("Android Studio project");
            if ui.button("Import FTC project folder").clicked() { self.import_project(); }
            ui.label(&self.import_status);
            if let Some(name) = &self.project_name {
                ui.label(format!("Project: {name}")); ui.label("Detected OpModes:");
                let mut clicked = None;
                egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                    for (i, mode) in self.op_modes.iter().enumerate() {
                        if ui.selectable_label(self.selected_op_mode == Some(i),
                            format!("{} · {}", mode.kind, mode.name)).clicked() { clicked = Some(i); }
                    }
                });
                if let Some(i) = clicked { self.selected_op_mode = Some(i); self.show_source = true; }
                if self.op_modes.is_empty() { ui.label("No @TeleOp or @Autonomous classes were detected."); }
            }
            ui.add_space(12.0); ui.separator(); ui.heading("Field status");
            ui.label("144 × 144 inches"); ui.label("Background: supplied field screenshot");
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| self.draw_field(ui));
        });
        if self.show_source {
            if let Some(index) = self.selected_op_mode {
                if let Some(mode) = self.op_modes.get(index) {
                    let title = format!("{} — {}", mode.kind, mode.name);
                    let mut source = mode.source.clone();
                    let path = mode.path.display().to_string();
                    let language = mode.language.clone();
                    egui::Window::new(title).open(&mut self.show_source)
                        .default_size([720.0, 620.0]).show(ctx, |ui| {
                            ui.label(format!("{language} · {path}")); ui.separator();
                            egui::ScrollArea::both().show(ui, |ui| {
                                ui.add(egui::TextEdit::multiline(&mut source)
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(f32::INFINITY).code_editor().interactive(false));
                            });
                        });
                }
            }
        }
    }
}
