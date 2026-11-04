use std::time::Instant;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, TextureHandle, Vec2};
use gilrs::{Axis, Gilrs};
use crate::importer::{self, OpModeInfo};

const FIELD_SIZE_IN: f32 = 144.0;
const ROBOT_SIZE_IN: f32 = 18.0;
const MAX_SPEED_IN_S: f32 = 48.0;
const MAX_TURN_DEG_S: f32 = 180.0;
const MAX_POLLEN: u8 = 4;
const POLLEN_RADIUS_IN: f32 = 1.4;
const INTAKE_RANGE_IN: f32 = 10.0;
const SHOT_SPEED_IN_S: f32 = 42.0;
const SHOT_FLIGHT_SECONDS: f32 = 0.72;

#[derive(Clone, Copy, Debug)]
struct Pollen {
    x: f32,
    y: f32,
}

#[derive(Clone, Copy, Debug)]
struct PollenProjectile {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    elapsed: f32,
}

pub struct VisualizerApp {
    robot_x: f32, robot_y: f32, heading_deg: f32, drive_speed: f32,
    is_running: bool, teleop_active: bool,
    last_frame: Instant, elapsed_seconds: f32,
    field_texture: Option<TextureHandle>, field_load_attempted: bool,
    project_name: Option<String>, op_modes: Vec<OpModeInfo>,
    selected_op_mode: Option<usize>, show_source: bool, import_status: String,
    gamepad: Option<Gilrs>, controller_status: String,
    stick_forward: f32, stick_strafe: f32, stick_turn: f32,
    forward_velocity: f32, strafe_velocity: f32, turn_velocity: f32,
    pollen_count: u8,
    field_pollen: Vec<Pollen>,
    pollen_projectile: Option<PollenProjectile>,
    pollen_status: String,
}

impl Default for VisualizerApp {
    fn default() -> Self {
        let (gamepad, controller_status) = match Gilrs::new() {
            Ok(g) => (Some(g), "Looking for a controller…".to_owned()),
            Err(e) => (None, format!("Controller input unavailable: {e}")),
        };
        Self {
            robot_x: 24.0, robot_y: 24.0, heading_deg: 0.0, drive_speed: MAX_SPEED_IN_S,
            is_running: false, teleop_active: false,
            last_frame: Instant::now(), elapsed_seconds: 0.0,
            field_texture: None, field_load_attempted: false, project_name: None,
            op_modes: Vec::new(), selected_op_mode: None, show_source: false,
            import_status: "No FTC project loaded.".to_owned(),
            gamepad, controller_status, stick_forward: 0.0, stick_strafe: 0.0, stick_turn: 0.0,
            forward_velocity: 0.0, strafe_velocity: 0.0, turn_velocity: 0.0,
            pollen_count: MAX_POLLEN,
            field_pollen: starting_field_pollen(),
            pollen_projectile: None,
            pollen_status: "4 preloaded pollen onboard.".to_owned(),
        }
    }
}

impl VisualizerApp {
    fn reset(&mut self) {
        self.robot_x = 24.0; self.robot_y = 24.0; self.heading_deg = 0.0;
        self.elapsed_seconds = 0.0; self.is_running = false; self.teleop_active = false;
        self.forward_velocity = 0.0; self.strafe_velocity = 0.0; self.turn_velocity = 0.0;
        self.last_frame = Instant::now();
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

    fn poll_controller(&mut self) {
        let Some(gilrs) = self.gamepad.as_mut() else { return; };
        while gilrs.next_event().is_some() {}
        let connected = gilrs.gamepads().find(|(_, gamepad)| gamepad.is_connected());
        if let Some((id, gamepad)) = connected {
            self.controller_status = format!("Connected: {}", gamepad.name());
            self.stick_strafe = deadzone(gamepad.value(Axis::LeftStickX));
            self.stick_forward = deadzone(gamepad.value(Axis::LeftStickY));
            self.stick_turn = deadzone(gamepad.value(Axis::RightStickX));
            let _ = id;
        } else {
            self.controller_status = "No controller detected. Connect an Xbox/compatible gamepad.".to_owned();
            self.stick_forward = 0.0;
            self.stick_strafe = 0.0;
            self.stick_turn = 0.0;
        }
    }


    fn intake_pollen(&mut self) {
        if self.pollen_count >= MAX_POLLEN {
            self.pollen_status = "Robot is full (4/4 pollen).".to_owned();
            return;
        }

        let heading = self.heading_deg.to_radians();
        let intake_x = self.robot_x + heading.sin() * (ROBOT_SIZE_IN * 0.5 + 2.0);
        let intake_y = self.robot_y + heading.cos() * (ROBOT_SIZE_IN * 0.5 + 2.0);
        let nearest = self.field_pollen.iter().enumerate()
            .map(|(i, pollen)| (i, (pollen.x - intake_x).hypot(pollen.y - intake_y)))
            .filter(|(_, distance)| *distance <= INTAKE_RANGE_IN)
            .min_by(|a, b| a.1.total_cmp(&b.1));

        if let Some((index, _)) = nearest {
            self.field_pollen.remove(index);
            self.pollen_count += 1;
            self.pollen_status = format!("Pollen collected: {}/4 onboard.", self.pollen_count);
        } else {
            self.pollen_status = "No pollen close enough to the intake.".to_owned();
        }
    }

    fn shoot_pollen(&mut self) {
        if self.pollen_count == 0 {
            self.pollen_status = "No pollen onboard to shoot.".to_owned();
            return;
        }
        if self.pollen_projectile.is_some() {
            self.pollen_status = "Wait for the current pollen to land.".to_owned();
            return;
        }

        let heading = self.heading_deg.to_radians();
        let speed = SHOT_SPEED_IN_S;
        let start_x = self.robot_x + heading.sin() * (ROBOT_SIZE_IN * 0.5 + 1.0);
        let start_y = self.robot_y + heading.cos() * (ROBOT_SIZE_IN * 0.5 + 1.0);
        self.pollen_projectile = Some(PollenProjectile {
            x: start_x,
            y: start_y,
            vx: heading.sin() * speed,
            vy: heading.cos() * speed,
            elapsed: 0.0,
        });
        self.pollen_count -= 1;
        self.pollen_status = format!("Pollen launched: {}/4 onboard.", self.pollen_count);
    }

    fn update_pollen(&mut self, dt: f32) {
        if let Some(mut projectile) = self.pollen_projectile {
            projectile.elapsed += dt;
            projectile.x += projectile.vx * dt;
            projectile.y += projectile.vy * dt;

            if projectile.elapsed >= SHOT_FLIGHT_SECONDS
                || projectile.x < POLLEN_RADIUS_IN
                || projectile.x > FIELD_SIZE_IN - POLLEN_RADIUS_IN
                || projectile.y < POLLEN_RADIUS_IN
                || projectile.y > FIELD_SIZE_IN - POLLEN_RADIUS_IN
            {
                let landed = Pollen {
                    x: projectile.x.clamp(POLLEN_RADIUS_IN, FIELD_SIZE_IN - POLLEN_RADIUS_IN),
                    y: projectile.y.clamp(POLLEN_RADIUS_IN, FIELD_SIZE_IN - POLLEN_RADIUS_IN),
                };
                self.field_pollen.push(landed);
                self.pollen_projectile = None;
                self.pollen_status = format!("Pollen landed back on the field. {}/4 onboard.", self.pollen_count);
            } else {
                self.pollen_projectile = Some(projectile);
            }
        }
    }

    fn telemetry_value(&self, expression: &str) -> String {
        let key = expression.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase();

        // PedroPathing and common pose APIs: expose the visualizer's simulated pose.
        match key.as_str() {
            "heading" | "headingdeg" | "robotheading" | "heading_degrees" => return format!("{:.1}°", self.heading_deg),
            "headingrad" | "headingradians" | "robotpose.heading()" | "follower.pose().heading()" => {
                return format!("{:.3} rad", self.heading_deg.to_radians());
            }
            "x" | "robotx" | "posx" | "getx()" | "robotpose.x()" | "follower.pose().x()" => {
                return format!("{:.1} in", self.robot_x);
            }
            "y" | "roboty" | "posy" | "gety()" | "robotpose.y()" | "follower.pose().y()" => {
                return format!("{:.1} in", self.robot_y);
            }
            "runtime" | "elapsedtime" | "elapsed_seconds" => return format!("{:.2} s", self.elapsed_seconds),
            "pollen" | "pollencount" | "pollen_count" | "robot.pollen" => return format!("{}/4", self.pollen_count),
            "gamepad1.left_stick_x" | "gamepad1.leftstickx" => return format!("{:.2}", self.stick_strafe),
            "gamepad1.left_stick_y" | "gamepad1.leftsticky" => return format!("{:.2}", self.stick_forward),
            "gamepad1.right_stick_x" | "gamepad1.rightstickx" => return format!("{:.2}", self.stick_turn),
            _ => {}
        }

        // Resolve simple numeric variables declared in the selected OpMode.
        // Complex Java/Kotlin expressions are intentionally not guessed.
        if matches!(key.as_str(), "p1" | "i1" | "d1" | "f1") {
            if let Some(value) = self.selected_op_mode
                .and_then(|index| self.op_modes.get(index))
                .and_then(|mode| numeric_variable(&mode.source, &key))
            {
                return format!("{value}");
            }
            return format!("{} (variable value unavailable)", expression);
        }

        if key.contains(".getvelocity()") {
            return format!("{} (requires a simulated motor or live motor data)", expression);
        }

        format!("{} (expression not evaluated)", expression)
    }

    fn update_simulation(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.05);
        self.last_frame = now;
        self.update_pollen(dt);

        if self.teleop_active {
            self.poll_controller();

            // Mecanum inverse kinematics: mix chassis commands into four wheel powers.
            let f = self.stick_forward;
            let s = self.stick_strafe;
            let r = self.stick_turn;
            let mut wheels = [f + s + r, f - s - r, f - s + r, f + s - r];
            let largest = wheels.iter().fold(1.0_f32, |m, p| m.max(p.abs()));
            for power in &mut wheels { *power /= largest; }

            // Convert wheel powers back into robot-relative forward, strafe and turn.
            let target_forward = (wheels[0] + wheels[1] + wheels[2] + wheels[3]) * 0.25;
            let target_strafe = (wheels[0] - wheels[1] - wheels[2] + wheels[3]) * 0.25;
            let target_turn = (wheels[0] - wheels[1] + wheels[2] - wheels[3]) * 0.25;

            // Acceleration-limited motor response.
            self.forward_velocity = approach(self.forward_velocity, target_forward * MAX_SPEED_IN_S, 72.0 * dt);
            self.strafe_velocity = approach(self.strafe_velocity, target_strafe * MAX_SPEED_IN_S, 72.0 * dt);
            self.turn_velocity = approach(self.turn_velocity, target_turn * MAX_TURN_DEG_S, 360.0 * dt);

            let heading = self.heading_deg.to_radians();
            let field_dx = self.strafe_velocity * heading.cos() + self.forward_velocity * heading.sin();
            let field_dy = -self.strafe_velocity * heading.sin() + self.forward_velocity * heading.cos();
            self.robot_x = (self.robot_x + field_dx * dt)
                .clamp(ROBOT_SIZE_IN / 2.0, FIELD_SIZE_IN - ROBOT_SIZE_IN / 2.0);
            self.robot_y = (self.robot_y + field_dy * dt)
                .clamp(ROBOT_SIZE_IN / 2.0, FIELD_SIZE_IN - ROBOT_SIZE_IN / 2.0);
            self.heading_deg = (self.heading_deg + self.turn_velocity * dt).rem_euclid(360.0);
            self.elapsed_seconds += dt;
            ctx.request_repaint();
        } else if self.is_running {
            let h = self.heading_deg.to_radians();
            self.robot_x = (self.robot_x + h.sin() * self.drive_speed * dt)
                .clamp(ROBOT_SIZE_IN / 2.0, FIELD_SIZE_IN - ROBOT_SIZE_IN / 2.0);
            self.robot_y = (self.robot_y + h.cos() * self.drive_speed * dt)
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

        // Field pollen: yellow balls at the manual's Flower and Garden staging areas.
        for pollen in &self.field_pollen {
            let position = Pos2::new(
                field.left() + pollen.x * scale,
                field.bottom() - pollen.y * scale,
            );
            painter.circle_filled(position, POLLEN_RADIUS_IN * scale, Color32::from_rgb(255, 218, 35));
            painter.circle_stroke(position, POLLEN_RADIUS_IN * scale, Stroke::new(1.0, Color32::from_rgb(120, 92, 0)));
        }

        if let Some(projectile) = self.pollen_projectile {
            let flight = (projectile.elapsed / SHOT_FLIGHT_SECONDS).clamp(0.0, 1.0);
            let arc_height = 18.0 * 4.0 * flight * (1.0 - flight);
            let position = Pos2::new(
                field.left() + projectile.x * scale,
                field.bottom() - projectile.y * scale - arc_height * scale,
            );
            painter.circle_filled(position, POLLEN_RADIUS_IN * scale, Color32::from_rgb(255, 235, 90));
            painter.circle_stroke(position, POLLEN_RADIUS_IN * scale, Stroke::new(1.2, Color32::WHITE));
        }

        let center = Pos2::new(field.left() + self.robot_x * scale, field.bottom() - self.robot_y * scale);
        let half = ROBOT_SIZE_IN * scale / 2.0;
        let robot = Rect::from_center_size(center, Vec2::splat(half * 2.0));
        painter.rect_filled(robot, 2.0, Color32::from_rgb(230, 55, 165));
        painter.rect_stroke(robot, 2.0, Stroke::new(1.5_f32, Color32::WHITE), egui::StrokeKind::Inside);
        let h = self.heading_deg.to_radians();
        // Heading: 0 degrees points up-field; positive angles rotate clockwise.
        let nose = Pos2::new(center.x + h.sin() * half, center.y - h.cos() * half);
        painter.line_segment([center, nose], Stroke::new(3.0_f32, Color32::YELLOW));
        painter.circle_filled(nose, 3.5, Color32::YELLOW);
        painter.text(rect.left_top() + Vec2::new(7.0, 7.0), Align2::LEFT_TOP,
            "BIOBUZZ FIELD", FontId::proportional(12.0), Color32::LIGHT_GRAY);
        painter.text(rect.right_bottom() - Vec2::new(7.0, 7.0), Align2::RIGHT_BOTTOM,
            "144 in × 144 in", FontId::proportional(11.0), Color32::LIGHT_GRAY);
    }
}


fn starting_field_pollen() -> Vec<Pollen> {
    let mut pollen = Vec::with_capacity(24);

    // Four Flowers attached to the four field walls; four pollen at each retrieval side.
    for offset in [-5.0_f32, -1.7, 1.7, 5.0] {
        pollen.push(Pollen { x: 72.0 + offset, y: 10.0 });
        pollen.push(Pollen { x: 134.0, y: 72.0 + offset });
        pollen.push(Pollen { x: 72.0 + offset, y: 134.0 });
        pollen.push(Pollen { x: 10.0, y: 72.0 + offset });
    }

    // Four pollen in each alliance Garden, staged in opposite corners.
    for offset in [0.0_f32, 4.5, 9.0, 13.5] {
        pollen.push(Pollen { x: 8.0 + offset, y: 8.0 });
        pollen.push(Pollen { x: 136.0 - offset, y: 136.0 });
    }

    pollen
}

fn numeric_variable(source: &str, variable: &str) -> Option<f32> {
    for line in source.lines() {
        let Some((left, right)) = line.split_once('=') else { continue; };
        let Some(declared_name) = left
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .filter(|part| !part.is_empty())
            .last()
        else { continue; };

        if declared_name == variable {
            let literal = right.trim().trim_end_matches(';').trim();
            if let Ok(value) = literal.parse::<f32>() {
                return Some(value);
            }
        }
    }
    None
}

fn deadzone(value: f32) -> f32 {
    if value.abs() < 0.12 { 0.0 } else { value }
}

fn approach(current: f32, target: f32, max_delta: f32) -> f32 {
    current + (target - current).clamp(-max_delta, max_delta)
}

impl eframe::App for VisualizerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.load_field_image(ctx);
        let intake_pressed = ctx.input(|input| input.key_pressed(egui::Key::E));
        let shoot_pressed = ctx.input(|input| input.key_pressed(egui::Key::Space));
        if intake_pressed { self.intake_pollen(); }
        if shoot_pressed { self.shoot_pollen(); }
        self.update_simulation(ctx);
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("FTC Code Visualizer"); ui.separator(); ui.label("BIOBUZZ · 2026–2027");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("Simulation time: {:.1}s", self.elapsed_seconds));
                });
            });
        });
        egui::SidePanel::left("controls").resizable(true).default_width(320.0).show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("sidebar_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
            ui.heading("Robot"); ui.add_space(8.0); ui.label("Position (inches)");
            ui.add(egui::Slider::new(&mut self.robot_x, 0.0..=FIELD_SIZE_IN).text("X"));
            ui.add(egui::Slider::new(&mut self.robot_y, 0.0..=FIELD_SIZE_IN).text("Y"));
            ui.add(egui::Slider::new(&mut self.heading_deg, 0.0..=360.0).text("Heading").suffix("°"));
            ui.add_space(12.0); ui.separator(); ui.heading("Simulation");
            ui.add(egui::Slider::new(&mut self.drive_speed, 0.0..=60.0).text("Test speed").suffix(" in/s"));
            ui.horizontal(|ui| {
                if ui.button(if self.is_running { "Pause test" } else { "Run test" }).clicked() {
                    self.is_running = !self.is_running; self.teleop_active = false; self.last_frame = Instant::now();
                }
                if ui.button("Reset").clicked() { self.reset(); }
            });
            ui.add_space(12.0); ui.separator(); ui.heading("TeleOp controller");
            ui.label(&self.controller_status);
            ui.label("Left stick: robot-relative forward / strafe");
            ui.label("Right stick X: rotate");
            ui.label("Mecanum wheel mixing with normalized wheel power and acceleration.");
            if ui.button(if self.teleop_active { "Stop TeleOp" } else { "Start TeleOp" }).clicked() {
                self.teleop_active = !self.teleop_active;
                self.is_running = false;
                self.last_frame = Instant::now();
            }
            ui.label(format!("Input: F {:.2} · S {:.2} · R {:.2}",
                self.stick_forward, self.stick_strafe, self.stick_turn));
            ui.add_space(8.0);
            ui.separator();
            ui.heading("Pollen");
            ui.strong(format!("Carrying: {}/4", self.pollen_count));
            ui.label(&self.pollen_status);
            ui.horizontal(|ui| {
                if ui.button("Intake pollen [E]").clicked() {
                    self.intake_pollen();
                }
                if ui.button("Shoot pollen [Space]").clicked() {
                    self.shoot_pollen();
                }
            });
            ui.label("Move the front intake close to a yellow pollen ball, then intake.");
            ui.add_space(8.0);
            ui.heading("OpMode telemetry");
            if let Some(index) = self.selected_op_mode {
                if let Some(mode) = self.op_modes.get(index) {
                    if mode.telemetry.is_empty() {
                        ui.label("No telemetry.addData() or telemetry.addLine() calls detected in this OpMode.");
                    } else {
                        egui::ScrollArea::vertical().id_salt("telemetry_scroll").max_height(260.0).auto_shrink([false, false]).show(ui, |ui| {
                            for item in &mode.telemetry {
                                if item.expression.is_empty() {
                                    ui.label(&item.caption);
                                } else {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.strong(format!("{}:", item.caption));
                                        ui.label(self.telemetry_value(&item.expression));
                                    });
                                }
                            }
                        });
                    }
                }
            } else {
                ui.label("Select an OpMode to inspect its telemetry calls.");
            }


            if let Some(index) = self.selected_op_mode {
                if let Some(mode) = self.op_modes.get(index) {
                    if mode.kind == "Autonomous" {
                        ui.colored_label(Color32::YELLOW, "Autonomous execution is not implemented yet. This OpMode is currently source-preview only.");
                    }
                }
            }

            ui.add_space(12.0); ui.separator(); ui.heading("Android Studio project");
            if ui.button("Import FTC project folder").clicked() { self.import_project(); }
            ui.label(&self.import_status);
            if let Some(name) = &self.project_name {
                ui.label(format!("Project: {name}")); ui.label("Detected OpModes:");
                let mut clicked = None;
                egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
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
