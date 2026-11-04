use std::time::Instant;

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, TextureHandle, Vec2};

const FIELD_SIZE_IN: f32 = 144.0;
const ROBOT_SIZE_IN: f32 = 18.0;

pub struct VisualizerApp {
    robot_x: f32,
    robot_y: f32,
    heading_deg: f32,
    drive_speed: f32,
    is_running: bool,
    last_frame: Instant,
    elapsed_seconds: f32,
    field_texture: Option<TextureHandle>,
    field_load_attempted: bool,
}

impl Default for VisualizerApp {
    fn default() -> Self {
        Self {
            robot_x: 24.0,
            robot_y: 24.0,
            heading_deg: 0.0,
            drive_speed: 24.0,
            is_running: false,
            last_frame: Instant::now(),
            elapsed_seconds: 0.0,
            field_texture: None,
            field_load_attempted: false,
        }
    }
}

impl VisualizerApp {
    fn reset(&mut self) {
        self.robot_x = 24.0;
        self.robot_y = 24.0;
        self.heading_deg = 0.0;
        self.elapsed_seconds = 0.0;
        self.is_running = false;
        self.last_frame = Instant::now();
    }

    fn load_field_image(&mut self, ctx: &egui::Context) {
        if self.field_load_attempted {
            return;
        }
        self.field_load_attempted = true;

        let path = std::path::Path::new("assets/biobuzz-field.png");
        match image::open(path) {
            Ok(image) => {
                let image = image.to_rgba8();
                let size = [image.width() as usize, image.height() as usize];
                let pixels = image.as_flat_samples();
                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                self.field_texture = Some(ctx.load_texture(
                    "biobuzz-field",
                    color_image,
                    egui::TextureOptions::LINEAR,
                ));
            }
            Err(error) => {
                eprintln!(
                    "Could not load {}: {error}. Add the supplied screenshot at this path.",
                    path.display()
                );
            }
        }
    }

    fn update_simulation(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.05);
        self.last_frame = now;

        if self.is_running {
            let heading = self.heading_deg.to_radians();
            self.robot_x += heading.cos() * self.drive_speed * dt;
            self.robot_y += heading.sin() * self.drive_speed * dt;
            self.robot_x = self.robot_x.clamp(ROBOT_SIZE_IN / 2.0, FIELD_SIZE_IN - ROBOT_SIZE_IN / 2.0);
            self.robot_y = self.robot_y.clamp(ROBOT_SIZE_IN / 2.0, FIELD_SIZE_IN - ROBOT_SIZE_IN / 2.0);
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
        let field = Rect::from_min_max(
            rect.min + Vec2::splat(inset),
            rect.max - Vec2::splat(inset),
        );

        if let Some(texture) = &self.field_texture {
            painter.image(
                texture.id(),
                field,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            painter.rect_filled(field, 0.0, Color32::from_rgb(43, 44, 46));
            painter.text(
                field.center(),
                Align2::CENTER_CENTER,
                "Field image missing\nPlace it at assets/biobuzz-field.png",
                FontId::proportional(16.0),
                Color32::WHITE,
            );
        }

        // Coordinates map to the full 144 × 144 inch field image.
        let scale = field.width() / FIELD_SIZE_IN;
        let to_screen = |x: f32, y: f32| -> Pos2 {
            Pos2::new(field.left() + x * scale, field.bottom() - y * scale)
        };

        let robot_center = to_screen(self.robot_x, self.robot_y);
        let robot_half = ROBOT_SIZE_IN * scale / 2.0;
        let robot_rect = Rect::from_center_size(
            robot_center,
            Vec2::splat(robot_half * 2.0),
        );

        painter.rect_filled(robot_rect, 2.0, Color32::from_rgb(230, 55, 165));
        painter.rect_stroke(
            robot_rect,
            2.0,
            Stroke::new(1.5_f32, Color32::WHITE),
            egui::StrokeKind::Inside,
        );

        let heading = self.heading_deg.to_radians();
        let nose = Pos2::new(
            robot_center.x + heading.cos() * robot_half,
            robot_center.y - heading.sin() * robot_half,
        );
        painter.line_segment(
            [robot_center, nose],
            Stroke::new(3.0_f32, Color32::YELLOW),
        );
        painter.circle_filled(nose, 3.5, Color32::YELLOW);

        painter.text(
            rect.left_top() + Vec2::new(7.0, 7.0),
            Align2::LEFT_TOP,
            "BIOBUZZ FIELD",
            FontId::proportional(12.0),
            Color32::LIGHT_GRAY,
        );
        painter.text(
            rect.right_bottom() - Vec2::new(7.0, 7.0),
            Align2::RIGHT_BOTTOM,
            "144 in × 144 in",
            FontId::proportional(11.0),
            Color32::LIGHT_GRAY,
        );
    }
}

impl eframe::App for VisualizerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.load_field_image(ctx);
        self.update_simulation(ctx);

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("FTC Code Visualizer");
                ui.separator();
                ui.label("BIOBUZZ · 2026–2027");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("Simulation time: {:.1}s", self.elapsed_seconds));
                });
            });
        });

        egui::SidePanel::left("controls")
            .resizable(true)
            .default_width(270.0)
            .show(ctx, |ui| {
                ui.heading("Robot");
                ui.add_space(8.0);

                ui.label("Position (inches)");
                ui.add(egui::Slider::new(&mut self.robot_x, 0.0..=FIELD_SIZE_IN).text("X"));
                ui.add(egui::Slider::new(&mut self.robot_y, 0.0..=FIELD_SIZE_IN).text("Y"));
                ui.add_space(4.0);
                ui.add(
                    egui::Slider::new(&mut self.heading_deg, 0.0..=360.0)
                        .text("Heading")
                        .suffix("°"),
                );

                ui.add_space(12.0);
                ui.separator();
                ui.heading("Test simulation");
                ui.add(egui::Slider::new(&mut self.drive_speed, 0.0..=60.0).text("Drive speed").suffix(" in/s"));

                ui.horizontal(|ui| {
                    if ui.button(if self.is_running { "Pause" } else { "Run" }).clicked() {
                        self.is_running = !self.is_running;
                        self.last_frame = Instant::now();
                    }
                    if ui.button("Reset").clicked() {
                        self.reset();
                    }
                });

                ui.add_space(12.0);
                ui.separator();
                ui.heading("Project");
                ui.label("No FTC project loaded.");
                ui.add_space(4.0);
                ui.label("Project import and OpMode parsing are not implemented yet.");

                ui.add_space(12.0);
                ui.separator();
                ui.heading("Field status");
                ui.label("144 × 144 inches");
                ui.label("Background: supplied field screenshot");
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                self.draw_field(ui);
            });
        });
    }
}
