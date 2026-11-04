use std::time::Instant;

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};

const FIELD_SIZE_IN: f32 = 144.0;
const ROBOT_SIZE_IN: f32 = 18.0;
const GRID_SPACING_IN: f32 = 24.0;

const RED: Color32 = Color32::from_rgb(225, 45, 55);
const BLUE: Color32 = Color32::from_rgb(45, 75, 220);
const YELLOW: Color32 = Color32::from_rgb(255, 190, 35);

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

    fn draw_basket(
        painter: &egui::Painter,
        to_screen: &impl Fn(f32, f32) -> Pos2,
        center_x: f32,
        center_y: f32,
        color: Color32,
    ) {
        // Stylized overhead outline based on the supplied field reference.
        let half_w = 10.5;
        let half_h = 4.2;
        let points = vec![
            to_screen(center_x - half_w, center_y - half_h),
            to_screen(center_x - half_w, center_y + half_h * 0.45),
            to_screen(center_x, center_y + half_h * 1.25),
            to_screen(center_x + half_w, center_y + half_h * 0.45),
            to_screen(center_x + half_w, center_y - half_h),
        ];

        painter.add(egui::Shape::convex_polygon(
            points.clone(),
            Color32::from_gray(85),
            Stroke::new(1.0_f32, color),
        ));
        painter.add(egui::Shape::closed_line(
            points,
            Stroke::new(1.5_f32, color),
        ));

        let inner = Rect::from_center_size(
            to_screen(center_x, center_y - 0.5),
            Vec2::new(half_w * 1.55, half_h * 0.85),
        );
        painter.rect_filled(inner, 1.0, Color32::from_gray(145));
        painter.rect_stroke(
            inner,
            1.0,
            Stroke::new(0.8_f32, color),
            egui::StrokeKind::Inside,
        );
    }

    fn draw_field(&self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let side = available.x.min(available.y).max(100.0);
        let (response, painter) = ui.allocate_painter(Vec2::splat(side), Sense::hover());
        let rect = response.rect;

        painter.rect_filled(rect, 3.0, Color32::from_rgb(25, 27, 30));

        let inset = side * 0.025;
        let field = Rect::from_min_max(
            rect.min + Vec2::splat(inset),
            rect.max - Vec2::splat(inset),
        );
        let scale = field.width() / FIELD_SIZE_IN;

        let to_screen = |x: f32, y: f32| -> Pos2 {
            Pos2::new(field.left() + x * scale, field.bottom() - y * scale)
        };

        let field_rect = Rect::from_min_max(to_screen(0.0, FIELD_SIZE_IN), to_screen(FIELD_SIZE_IN, 0.0));
        painter.rect_filled(field_rect, 0.0, Color32::from_rgb(43, 44, 46));

        // Six-by-six floor tile grid: each tile is 24 inches square.
        for tile in 0..=6 {
            let inch = tile as f32 * GRID_SPACING_IN;
            let x = to_screen(inch, 0.0).x;
            let y = to_screen(0.0, inch).y;
            let grid_color = Color32::from_rgb(17, 18, 20);
            painter.line_segment(
                [Pos2::new(x, field_rect.top()), Pos2::new(x, field_rect.bottom())],
                Stroke::new(1.0_f32, grid_color),
            );
            painter.line_segment(
                [Pos2::new(field_rect.left(), y), Pos2::new(field_rect.right(), y)],
                Stroke::new(1.0_f32, grid_color),
            );
        }

        // Field perimeter.
        painter.rect_stroke(
            field_rect,
            0.0,
            Stroke::new(2.5_f32, Color32::from_rgb(12, 13, 15)),
            egui::StrokeKind::Inside,
        );

        // Alliance areas near the outer walls.
        let zone_stroke = Stroke::new(1.2_f32, RED);
        let red_zone_top = Rect::from_two_pos(to_screen(0.0, 120.0), to_screen(10.0, 96.0));
        let red_zone_bottom = Rect::from_two_pos(to_screen(0.0, 48.0), to_screen(24.0, 0.0));
        painter.rect_stroke(red_zone_top, 0.0, zone_stroke, egui::StrokeKind::Inside);
        painter.rect_stroke(red_zone_bottom, 0.0, zone_stroke, egui::StrokeKind::Inside);

        let blue_zone_top = Rect::from_two_pos(to_screen(120.0, 144.0), to_screen(144.0, 120.0));
        let blue_zone_bottom = Rect::from_two_pos(to_screen(134.0, 48.0), to_screen(144.0, 24.0));
        painter.rect_stroke(blue_zone_top, 0.0, Stroke::new(1.2_f32, BLUE), egui::StrokeKind::Inside);
        painter.rect_stroke(blue_zone_bottom, 0.0, Stroke::new(1.2_f32, BLUE), egui::StrokeKind::Inside);

        // Four red baskets on the left and four blue baskets on the right.
        for y in [55.0, 66.0, 78.0, 89.0] {
            Self::draw_basket(&painter, &to_screen, 52.0, y, RED);
            Self::draw_basket(&painter, &to_screen, 92.0, y, BLUE);
        }

        // Central supports and the two yellow crossbars.
        let support_color = Color32::from_rgb(115, 119, 124);
        for (x1, y1, x2, y2) in [
            (62.0, 58.0, 62.0, 86.0),
            (82.0, 58.0, 82.0, 86.0),
            (62.0, 72.0, 82.0, 72.0),
            (62.0, 79.0, 82.0, 79.0),
        ] {
            painter.line_segment(
                [to_screen(x1, y1), to_screen(x2, y2)],
                Stroke::new(2.0_f32, support_color),
            );
        }
        for y in [71.0, 77.0] {
            let beam = Rect::from_two_pos(to_screen(62.0, y + 1.0), to_screen(82.0, y - 1.0));
            painter.rect_filled(beam, 1.0, Color32::from_rgb(235, 190, 95));
            painter.rect_stroke(beam, 1.0, Stroke::new(0.8_f32, Color32::from_rgb(70, 60, 45)), egui::StrokeKind::Inside);
        }

        // Small yellow field elements around the perimeter.
        for x in [18.0, 72.0, 126.0] {
            for offset in [-3.0, 0.0, 3.0] {
                let p = to_screen(x + offset, 140.0);
                painter.circle_filled(p, scale * 1.15, YELLOW);
                painter.circle_stroke(p, scale * 1.15, Stroke::new(0.7_f32, Color32::BLACK));
            }
        }
        for y in [18.0, 72.0, 126.0] {
            for offset in [-3.0, 0.0, 3.0] {
                let p = to_screen(4.0, y + offset);
                painter.circle_filled(p, scale * 1.15, YELLOW);
                painter.circle_stroke(p, scale * 1.15, Stroke::new(0.7_f32, Color32::BLACK));
            }
        }

        // Robot footprint and heading.
        let robot_center = to_screen(self.robot_x, self.robot_y);
        let robot_half = ROBOT_SIZE_IN * scale / 2.0;
        let robot_rect = Rect::from_center_size(robot_center, Vec2::splat(robot_half * 2.0));
        painter.rect_filled(robot_rect, 2.0, Color32::from_rgb(230, 55, 165));
        painter.rect_stroke(robot_rect, 2.0, Stroke::new(1.5_f32, Color32::WHITE), egui::StrokeKind::Inside);

        let heading = self.heading_deg.to_radians();
        let nose = Pos2::new(
            robot_center.x + heading.cos() * robot_half,
            robot_center.y - heading.sin() * robot_half,
        );
        painter.line_segment([robot_center, nose], Stroke::new(3.0_f32, YELLOW));
        painter.circle_filled(nose, 3.5, YELLOW);

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
            "144 in × 144 in  |  24 in grid",
            FontId::proportional(11.0),
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
                ui.label("144 × 144 inches");
                ui.label("24 × 24 inch floor tiles");
                ui.label("Field element positions are approximate");
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                self.draw_field(ui);
            });
        });
    }
}
