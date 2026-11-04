use std::time::Instant;

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};

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
        let (response, painter) =
            ui.allocate_painter(Vec2::splat(side), Sense::hover());
        let rect = response.rect;

        painter.rect_filled(rect, 4.0, Color32::from_rgb(31, 35, 40));
        painter.rect_stroke(
            rect,
            4.0,
            Stroke::new(2.0_f32, Color32::from_gray(180)),
            egui::StrokeKind::Inside,
        );

        let inset = side * 0.035;
        let field = Rect::from_min_max(
            rect.min + Vec2::splat(inset),
            rect.max - Vec2::splat(inset),
        );
        let scale = field.width() / FIELD_SIZE_IN;

        let to_screen = |x: f32, y: f32| -> Pos2 {
            Pos2::new(
                field.left() + x * scale,
                field.bottom() - y * scale,
            )
        };

        // A neutral grid is used until the official season field geometry is added.
        for inch in (0..=144).step_by(12) {
            let x = field.left() + inch as f32 * scale;
            let y = field.bottom() - inch as f32 * scale;
            let color = if inch % 24 == 0 {
                Color32::from_gray(105)
            } else {
                Color32::from_gray(65)
            };
            painter.line_segment(
                [Pos2::new(x, field.top()), Pos2::new(x, field.bottom())],
                Stroke::new(1.0_f32, color),
            );
            painter.line_segment(
                [Pos2::new(field.left(), y), Pos2::new(field.right(), y)],
                Stroke::new(1.0_f32, color),
            );
        }

        painter.rect_stroke(
            field,
            0.0,
            Stroke::new(2.0_f32, Color32::from_gray(190)),
            egui::StrokeKind::Inside,
        );

        let center = to_screen(72.0, 72.0);
        painter.circle_stroke(center, scale * 12.0, Stroke::new(2.0_f32, Color32::from_gray(150)));
        painter.line_segment(
            [to_screen(72.0, 0.0), to_screen(72.0, 144.0)],
            Stroke::new(1.5_f32, Color32::from_gray(120)),
        );
        painter.line_segment(
            [to_screen(0.0, 72.0), to_screen(144.0, 72.0)],
            Stroke::new(1.5_f32, Color32::from_gray(120)),
        );

        let robot_center = to_screen(self.robot_x, self.robot_y);
        let robot_half = ROBOT_SIZE_IN * scale / 2.0;
        let robot_rect = Rect::from_center_size(
            robot_center,
            Vec2::splat(robot_half * 2.0),
        );
        painter.rect_filled(robot_rect, 3.0, Color32::from_rgb(40, 145, 220));
        painter.rect_stroke(
            robot_rect,
            3.0,
            Stroke::new(2.0_f32, Color32::WHITE),
            egui::StrokeKind::Inside,
        );

        let heading = self.heading_deg.to_radians();
        let nose = Pos2::new(
            robot_center.x + heading.cos() * robot_half,
            robot_center.y - heading.sin() * robot_half,
        );
        painter.line_segment(
            [robot_center, nose],
            Stroke::new(4.0_f32, Color32::YELLOW),
        );
        let left = heading + 2.5;
        let right = heading - 2.5;
        let tip = Pos2::new(
            robot_center.x + heading.cos() * robot_half * 1.25,
            robot_center.y - heading.sin() * robot_half * 1.25,
        );
        let wing_a = Pos2::new(
            robot_center.x + left.cos() * robot_half * 0.65,
            robot_center.y - left.sin() * robot_half * 0.65,
        );
        let wing_b = Pos2::new(
            robot_center.x + right.cos() * robot_half * 0.65,
            robot_center.y - right.sin() * robot_half * 0.65,
        );
        painter.add(egui::Shape::convex_polygon(
            vec![tip, wing_a, wing_b],
            Color32::YELLOW,
            Stroke::NONE,
        ));

        painter.text(
            rect.left_top() + Vec2::new(8.0, 8.0),
            Align2::LEFT_TOP,
            "FIELD PREVIEW",
            FontId::proportional(12.0),
            Color32::LIGHT_GRAY,
        );
        painter.text(
            rect.right_bottom() - Vec2::new(8.0, 8.0),
            Align2::RIGHT_BOTTOM,
            "144 in × 144 in",
            FontId::proportional(12.0),
            Color32::LIGHT_GRAY,
        );
    }
}

impl eframe::App for VisualizerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
                ui.label("144 × 144 inch coordinate space");
                ui.label("Season field geometry: pending verification");
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                self.draw_field(ui);
            });
        });
    }
}
