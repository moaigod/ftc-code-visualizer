use std::time::Instant;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, TextureHandle, Vec2};
use gilrs::{Axis, Button, Gilrs};
use crate::importer::{self, OpModeInfo};

const FIELD_SIZE_IN: f32 = 144.0;
const ROBOT_SIZE_IN: f32 = 18.0;
const MAX_SPEED_IN_S: f32 = 48.0;
const MAX_TURN_DEG_S: f32 = 180.0;
const MAX_POLLEN: u8 = 4;
const POLLEN_RADIUS_IN: f32 = 1.4;
const NECTAR_RADIUS_IN: f32 = 1.8;
const SHOT_FLIGHT_SECONDS: f32 = 0.72;
const FLOWER_INTAKE_COOLDOWN_SECONDS: f32 = 0.5;
const HIVE_STARTING_NECTAR: u8 = 3;
const HIVE_TIP_LOAD_UNITS: f32 = 8.0;
const HIVE_NECTAR_LOAD_UNITS: f32 = 1.6;
const HIVE_POLLEN_LOAD_UNITS: f32 = 1.0;
const RED_HIVE_A: (f32, f32) = (58.0, 59.0); // Starts untipped
const RED_HIVE_B: (f32, f32) = (58.0, 85.0); // Starts tipped
const BLUE_HIVE_A: (f32, f32) = (86.0, 85.0); // Starts untipped
const BLUE_HIVE_B: (f32, f32) = (86.0, 57.0); // Starts tipped
const HIVE_CELL_HALF_WIDTH_IN: f32 = 11.0;
const HIVE_CELL_HALF_DEPTH_IN: f32 = 7.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ElementKind { Pollen, Nectar }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HiveSide { Red, Blue }

#[derive(Clone, Copy, Debug)]
struct Pollen {
    x: f32,
    y: f32,
    radius: f32,
    from_flower: bool,
    scored: bool,
    kind: ElementKind,
    hive: Option<HiveSide>,
    vx: f32,
    vy: f32,
}

#[derive(Clone, Copy, Debug)]
struct CarriedElement {
    kind: ElementKind,
    hive: Option<HiveSide>,
}

#[derive(Clone, Copy, Debug)]
struct PollenProjectile {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    elapsed: f32,
    kind: ElementKind,
    hive: Option<HiveSide>,
}

pub struct VisualizerApp {
    robot_x: f32, robot_y: f32, heading_deg: f32, drive_speed: f32,
    is_running: bool, teleop_active: bool,
    last_frame: Instant, elapsed_seconds: f32,
    field_texture: Option<TextureHandle>, field_load_attempted: bool,
    project_name: Option<String>, op_modes: Vec<OpModeInfo>,
    selected_op_mode: Option<usize>, show_source: bool, import_status: String,
    gamepad: Option<Gilrs>, selected_gamepad: Option<gilrs::GamepadId>, controller_status: String,
    stick_forward: f32, stick_strafe: f32, stick_turn: f32,
    forward_velocity: f32, strafe_velocity: f32, turn_velocity: f32,
    carried_elements: Vec<CarriedElement>,
    field_pollen: Vec<Pollen>,
    pollen_projectile: Option<PollenProjectile>,
    pollen_status: String,
    drive_wheel_power: [f32; 4],
    drive_wheel_velocity: [f32; 4],
    previous_gamepad_buttons: Vec<String>,
    flower_intake_cooldown: f32,
    intake_running: bool,
    feeder_running: bool,
    flywheel_velocity: f32,
    flywheel_target_velocity: f32,
    shooter_velocity_scale: f32,
    shot_cooldown: f32,
    intake_cycle_timer: f32,
    shot_sequence: u32,
    red_hive_pollen: u8,
    blue_hive_pollen: u8,
    red_hive_extra_nectar: u8,
    blue_hive_extra_nectar: u8,
    red_hive_tipped: bool,
    blue_hive_tipped: bool,
    red_hive_tip_count: u32,
    blue_hive_tip_count: u32,
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
            gamepad, selected_gamepad: None, controller_status, stick_forward: 0.0, stick_strafe: 0.0, stick_turn: 0.0,
            forward_velocity: 0.0, strafe_velocity: 0.0, turn_velocity: 0.0,
            carried_elements: vec![CarriedElement { kind: ElementKind::Pollen, hive: None }; MAX_POLLEN as usize],
            field_pollen: starting_field_pollen(),
            pollen_projectile: None,
            pollen_status: "4 preloaded pollen onboard.".to_owned(),
            drive_wheel_power: [0.0; 4],
            drive_wheel_velocity: [0.0; 4],
            previous_gamepad_buttons: Vec::new(),
            flower_intake_cooldown: 0.0,
            intake_running: false,
            feeder_running: false,
            flywheel_velocity: 0.0,
            flywheel_target_velocity: 0.0,
            shooter_velocity_scale: 0.0320,
            shot_cooldown: 0.0,
            intake_cycle_timer: 0.0,
            shot_sequence: 0,
            red_hive_pollen: 0,
            blue_hive_pollen: 0,
            red_hive_extra_nectar: 0,
            blue_hive_extra_nectar: 0,
            red_hive_tipped: false,
            blue_hive_tipped: false,
            red_hive_tip_count: 0,
            blue_hive_tip_count: 0,
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

    fn poll_controller(&mut self) -> Vec<String> {
        let Some(gilrs) = self.gamepad.as_mut() else { return Vec::new(); };
        while gilrs.next_event().is_some() {}
        let selected_id = self.selected_gamepad
            .filter(|id| gilrs.gamepad(*id).is_connected())
            .or_else(|| gilrs.gamepads().find(|(_, gamepad)| gamepad.is_connected()).map(|(id, _)| id));
        self.selected_gamepad = selected_id;
        if let Some(id) = selected_id {
            let gamepad = gilrs.gamepad(id);
            let detected_name = gamepad.name().to_ascii_lowercase();
            let layout = if detected_name.contains("dualsense") || detected_name.contains("dualshock") || detected_name.contains("playstation") || detected_name.contains("sony") {
                "PlayStation"
            } else if detected_name.contains("xbox") || detected_name.contains("xinput") || detected_name.contains("microsoft") {
                "Xbox"
            } else {
                "standard gamepad"
            };
            self.controller_status = format!("Connected: {} ({layout} detected)", gamepad.name());
            self.stick_strafe = deadzone(gamepad.value(Axis::LeftStickX));
            self.stick_forward = deadzone(gamepad.value(Axis::LeftStickY));
            self.stick_turn = deadzone(gamepad.value(Axis::RightStickX));
            let buttons = [
                (Button::South, "south"), (Button::East, "east"),
                (Button::West, "west"), (Button::North, "north"),
                (Button::LeftTrigger, "left_trigger"), (Button::RightTrigger, "right_trigger"),
                (Button::DPadUp, "dpad_up"), (Button::DPadDown, "dpad_down"),
                (Button::DPadLeft, "dpad_left"), (Button::DPadRight, "dpad_right"),
                (Button::Start, "start"), (Button::Select, "back"),
            ];
            buttons.iter().filter_map(|(button, name)|
                gamepad.is_pressed(*button).then_some((*name).to_owned())
            ).collect()
        } else {
            self.controller_status = "No controller detected. Connect an Xbox/compatible gamepad.".to_owned();
            self.stick_forward = 0.0;
            self.stick_strafe = 0.0;
            self.stick_turn = 0.0;
            Vec::new()
        }
    }

    fn detected_gamepad_actions(&self) -> Vec<(String, String)> {
        let Some(mode) = self.selected_op_mode.and_then(|i| self.op_modes.get(i)) else {
            return Vec::new();
        };
        find_gamepad_actions(&mode.source)
    }

    fn run_gamepad_action(&mut self, button: &str, action: &str) {
        let normalized = action.to_ascii_lowercase();
        if normalized.contains("intake") {
            self.intake_running = true;
        } else if normalized.contains("feeder") {
            self.feeder_running = true;
            self.pollen_status = "Feeder activated by the selected OpMode.".to_owned();
        } else if normalized.contains("outtake") || normalized.contains("shooter") {
            let source = self.selected_op_mode.and_then(|i| self.op_modes.get(i)).map(|m| m.source.as_str()).unwrap_or("");
            let program = crate::ftc_interpreter::interpret(source);
            let target = program.commands.iter()
                .filter(|command| command.button.as_deref() == Some(button) && command.method == "setVelocity")
                .filter_map(|command| command.value)
                .max_by(f32::total_cmp);
            if let Some(value) = target {
                self.flywheel_target_velocity = value.max(0.0);
                self.pollen_status = if value <= 0.0 {
                    format!("{} set the flywheel target to zero; the flywheel is coasting down.", button)
                } else {
                    format!("{} selected flywheel target: {:.0} ticks/s.", button, value)
                };
            } else {
                self.pollen_status = format!("{} button has no resolvable outtake setVelocity() command.", button);
            }
        } else {
            self.pollen_status = format!(
                "Detected button action: {action}. This command is not yet simulated."
            );
        }
    }

    fn detected_shooter_command(&self) -> Option<(String, String)> {
        let source = self.selected_op_mode.and_then(|i| self.op_modes.get(i))?.source.as_str();
        find_outtake_commands(source).into_iter().find(|(_, _, kind, value)| kind == "setVelocity" && value.trim() != "0").map(|(_, _, kind, value)| (kind, value))
    }

    fn detected_shooter_power(&self) -> Option<f32> {
        let (kind, value) = self.detected_shooter_command()?;
        if kind != "setPower" { return None; }
        value.trim().parse::<f32>().ok()
    }

    fn intake_pollen(&mut self) {
        if self.carried_elements.len() >= MAX_POLLEN as usize {
            self.pollen_status = "Robot is full (4/4 total elements).".to_owned(); return;
        }
        let h = self.heading_deg.to_radians();
        let nearest = self.field_pollen.iter().enumerate().filter_map(|(i, e)| {
            if e.scored { return None; }
            let dx=e.x-self.robot_x; let dy=e.y-self.robot_y;
            // Inverse of the field transform used to draw the robot:
            // local +Y points toward the front of the chassis.
            let lx=dx*h.cos()+dy*h.sin();
            // Slightly wider/deeper trigger zone than the rendered intake bar:
            // any scoring element physically touching the front intake is eligible.
            // World +Y is up-field, matching the robot drawing transform.
            // The old subtraction inverted forward detection, so the intake
            // only worked reliably when the robot was turned sideways.
            let ly=dx*h.sin()+dy*h.cos();
            // Give the intake a forgiving capture envelope around its front bar.
            let cx=lx.clamp(-8.5,8.5);
            let cy=ly.clamp(6.5,12.5);
            let d=(lx-cx).hypot(ly-cy);
            (d<=e.radius+1.5).then_some((i,d))
        }).min_by(|a,b| a.1.total_cmp(&b.1));
        if let Some((i,_))=nearest {
            let e=self.field_pollen[i];
            let blocked_by_red = e.hive == Some(HiveSide::Red)
                && point_in_hive_cell((e.x, e.y), hive_pair_centers(HiveSide::Red, self.red_hive_tipped).0);
            let blocked_by_blue = e.hive == Some(HiveSide::Blue)
                && point_in_hive_cell((e.x, e.y), hive_pair_centers(HiveSide::Blue, self.blue_hive_tipped).0);
            if blocked_by_red || blocked_by_blue {
                self.pollen_status="Hive elements are inaccessible until that hive tips.".to_owned(); return;
            }
            if e.from_flower && self.flower_intake_cooldown>0.0 {
                self.pollen_status=format!("Flower intake cooling down: {:.1}s.",self.flower_intake_cooldown); return;
            }
            self.field_pollen.remove(i);
            self.carried_elements.push(CarriedElement { kind: e.kind, hive: e.hive });
            if e.from_flower { self.flower_intake_cooldown=FLOWER_INTAKE_COOLDOWN_SECONDS; }
            let name=match e.kind {ElementKind::Pollen=>"Pollen",ElementKind::Nectar=>"Nectar"};
            self.pollen_status=format!("{name} collected. Carrying {}/4 total elements.",self.carried_elements.len());
        } else { self.pollen_status="No scoring element is touching the intake.".to_owned(); }
    }

    fn shoot_pollen(&mut self) {
        let source=self.selected_op_mode.and_then(|i|self.op_modes.get(i)).map(|m|m.source.as_str()).unwrap_or("");
        let program=crate::ftc_interpreter::interpret(source);
        let has_intake=program.motors.iter().any(|m|matches!(&m.role,crate::ftc_interpreter::MotorRole::Intake));
        let has_feeder=program.motors.iter().any(|m|matches!(&m.role,crate::ftc_interpreter::MotorRole::Feeder));
        if has_intake&&!self.intake_running {self.pollen_status="Shot blocked: activate the detected intake first.".to_owned();return;}
        if has_feeder&&!self.feeder_running {self.pollen_status="Shot blocked: activate the detected feeder first.".to_owned();return;}
        if self.carried_elements.is_empty() {self.pollen_status="No scoring elements onboard to shoot.".to_owned();return;}
        if self.pollen_projectile.is_some()||self.shot_cooldown>0.0{return;}
        let target=self.flywheel_target_velocity;
        if target<=0.0 {self.pollen_status="No positive setVelocity() target was resolved from the selected OpMode.".to_owned();return;}
        let min=(target*0.25).max(100.0);
        if self.flywheel_velocity<min {self.pollen_status=format!("Flywheel spinning up: {:.0}/{:.0} ticks/s (shots become available at {:.0}).",self.flywheel_velocity,target,min);return;}
        // FIFO: the first collected element is the first one launched.
        let cargo=self.carried_elements.remove(0);
        let kind=cargo.kind;
        const SPREAD:[f32;7]=[-2.2,1.1,2.0,-0.8,0.4,-1.5,1.6];
        let spread=SPREAD[self.shot_sequence as usize%SPREAD.len()].to_radians();
        self.shot_sequence=self.shot_sequence.wrapping_add(1);
        let heading=self.heading_deg.to_radians();
        let launch=heading+spread;
        let speed=(self.flywheel_velocity*self.shooter_velocity_scale).clamp(3.0,100.0);
        self.pollen_projectile=Some(PollenProjectile{x:self.robot_x+heading.sin()*(ROBOT_SIZE_IN*0.5+1.0),y:self.robot_y+heading.cos()*(ROBOT_SIZE_IN*0.5+1.0),vx:launch.sin()*speed,vy:launch.cos()*speed,elapsed:0.0,kind,hive:cargo.hive});
        let loss=(0.30+(target/3000.0)*0.20).clamp(0.30,0.55);
        self.flywheel_velocity*=1.0-loss; self.shot_cooldown=0.28;
        let name=match kind {ElementKind::Pollen=>"Pollen",ElementKind::Nectar=>"Nectar"};
        self.pollen_status=format!("{name} fired FIFO. {} element(s) remain.",self.carried_elements.len());
    }

    fn update_loose_pollen_physics(&mut self, dt: f32) {
        // Loose pollen has light inertia, floor friction, wall collisions, and
        // can be pushed by the robot instead of passing through its chassis.
        let robot_speed = self.forward_velocity.hypot(self.strafe_velocity);
        let heading = self.heading_deg.to_radians();
        for pollen in &mut self.field_pollen {
            // Scored elements are permanently secured in the hive: they remain visible,
            // but cannot be pushed by the robot or collected again.
            if pollen.scored { continue; }

            // Pollen that belongs to a flower stays anchored in its stack.
            if pollen.from_flower {
                pollen.vx = 0.0;
                pollen.vy = 0.0;
                continue;
            }

            pollen.x += pollen.vx * dt;
            pollen.y += pollen.vy * dt;
            let friction = (1.0 - 2.8 * dt).max(0.0);
            pollen.vx *= friction;
            pollen.vy *= friction;

            // Treat the robot as its actual rotated 18x18-inch square,
            // rather than a 12.73-inch-radius circle that overstates its hitbox.
            // Nectar staged inside an untipped hive is behind the hive's
            // scoring-only barrier; the robot chassis must pass through it.
            let inside_untipped_hive =
                pollen.hive == Some(HiveSide::Red)
                    && point_in_hive_cell((pollen.x, pollen.y), hive_pair_centers(HiveSide::Red, self.red_hive_tipped).0)
                || pollen.hive == Some(HiveSide::Blue)
                    && point_in_hive_cell((pollen.x, pollen.y), hive_pair_centers(HiveSide::Blue, self.blue_hive_tipped).0);
            if inside_untipped_hive { continue; }

            let dx = pollen.x - self.robot_x;
            let dy = pollen.y - self.robot_y;
            let local_x = dx * heading.cos() + dy * heading.sin();
            let local_y = dx * heading.sin() - dy * heading.cos();
            let closest_x = local_x.clamp(-ROBOT_SIZE_IN * 0.5, ROBOT_SIZE_IN * 0.5);
            let closest_y = local_y.clamp(-ROBOT_SIZE_IN * 0.5, ROBOT_SIZE_IN * 0.5);
            let away_x = local_x - closest_x;
            let away_y = local_y - closest_y;
            let distance = away_x.hypot(away_y);
            if distance < pollen.radius {
                let (local_nx, local_ny) = if distance > 0.001 {
                    (away_x / distance, away_y / distance)
                } else if local_x.abs() > local_y.abs() {
                    (local_x.signum(), 0.0)
                } else {
                    (0.0, local_y.signum().copysign(1.0))
                };
                let nx = local_nx * heading.cos() + local_ny * heading.sin();
                let ny = local_nx * heading.sin() - local_ny * heading.cos();
                let overlap = pollen.radius - distance;
                pollen.x += nx * overlap;
                pollen.y += ny * overlap;
                let push_speed = robot_speed.max(2.0);
                pollen.vx = pollen.vx * 0.25 + nx * push_speed * 0.85;
                pollen.vy = pollen.vy * 0.25 + ny * push_speed * 0.85;
            }

            // Invisible hive perimeter walls affect scoring elements only.
            // They keep the starting Nectar inside each untipped hive while
            // still allowing the robot to drive across the same area.
            let (red_untipped, red_tipped) = hive_pair_centers(HiveSide::Red, self.red_hive_tipped);
            let (blue_untipped, blue_tipped) = hive_pair_centers(HiveSide::Blue, self.blue_hive_tipped);
            collide_element_with_hive(pollen, red_untipped, HiveSide::Red, false);
            collide_element_with_hive(pollen, red_tipped, HiveSide::Red, true);
            collide_element_with_hive(pollen, blue_untipped, HiveSide::Blue, false);
            collide_element_with_hive(pollen, blue_tipped, HiveSide::Blue, true);
        }

        // Resolve overlaps between loose pollen pieces with a soft, damped bounce.
        for i in 0..self.field_pollen.len() {
            for j in (i + 1)..self.field_pollen.len() {
                let (left, right) = self.field_pollen.split_at_mut(j);
                let a = &mut left[i];
                let b = &mut right[0];
                // Flower-stack pollen is fixed, and scored elements are
                // secured in the hive. Neither can collide with loose elements.
                if a.from_flower || b.from_flower || a.scored || b.scored {
                    continue;
                }
                let dx = b.x - a.x;
                let dy = b.y - a.y;
                let distance = dx.hypot(dy);
                let minimum = a.radius + b.radius;
                if distance > 0.001 && distance < minimum {
                    let nx = dx / distance;
                    let ny = dy / distance;
                    let overlap = (minimum - distance) * 0.5;
                    a.x -= nx * overlap; a.y -= ny * overlap;
                    b.x += nx * overlap; b.y += ny * overlap;
                    let relative = (a.vx - b.vx) * nx + (a.vy - b.vy) * ny;
                    if relative > 0.0 {
                        a.vx -= nx * relative * 0.55; a.vy -= ny * relative * 0.55;
                        b.vx += nx * relative * 0.55; b.vy += ny * relative * 0.55;
                    }
                }
            }
        }

        // Keep every loose pollen piece fully inside the field. Bounce it
        // gently off the boundary instead of allowing it to leave the play area.
        for pollen in &mut self.field_pollen {
            let min = pollen.radius;
            let max = FIELD_SIZE_IN - pollen.radius;
            if pollen.x < min {
                pollen.x = min;
                pollen.vx = pollen.vx.abs() * 0.35;
            } else if pollen.x > max {
                pollen.x = max;
                pollen.vx = -pollen.vx.abs() * 0.35;
            }
            if pollen.y < min {
                pollen.y = min;
                pollen.vy = pollen.vy.abs() * 0.35;
            } else if pollen.y > max {
                pollen.y = max;
                pollen.vy = -pollen.vy.abs() * 0.35;
            }
        }
    }

    fn resolve_robot_wall_collisions(&mut self) {
        // Keep the rotated chassis corners inside the 144-inch field. Correcting
        // the center after each heading update makes the robot slide/push away
        // from a wall while turning rather than rotating through it.
        let h = self.heading_deg.to_radians();
        let corners = [(-9.0_f32,-9.0_f32),(9.0,-9.0),(9.0,9.0),(-9.0,9.0)];
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for (x,y) in corners {
            let wx = x * h.cos() + y * h.sin();
            let wy = -x * h.sin() + y * h.cos();
            min_x = min_x.min(wx); max_x = max_x.max(wx);
            min_y = min_y.min(wy); max_y = max_y.max(wy);
        }
        // Field coordinates run from 0 to 144. The chassis edge, not
        // its center, must stay within those boundaries.
        self.robot_x += (-(self.robot_x + min_x)).max(0.0);
        self.robot_x -= (self.robot_x + max_x - FIELD_SIZE_IN).max(0.0);
        self.robot_y += (-(self.robot_y + min_y)).max(0.0);
        self.robot_y -= (self.robot_y + max_y - FIELD_SIZE_IN).max(0.0);
    }

    fn update_pollen(&mut self, dt: f32) {
        if let Some(mut projectile) = self.pollen_projectile {
            projectile.elapsed += dt;
            let previous_x = projectile.x;
            let previous_y = projectile.y;
            projectile.x += projectile.vx * dt;
            projectile.y += projectile.vy * dt;

            // Calibrated tipping load: 8 pollen-equivalent units. Each Nectar is
            // weighted as 1.6 Pollen, so 3 starting Nectar + 4 Pollen tips;
            // 5 Nectar alone or 8 Pollen alone also reaches the threshold.
            // Test the entire movement segment, not only its endpoint, so a
            // fast projectile cannot skip over a hive between simulation frames.
            let red_untipped = hive_pair_centers(HiveSide::Red, self.red_hive_tipped).0;
            let blue_untipped = hive_pair_centers(HiveSide::Blue, self.blue_hive_tipped).0;
            let in_red_cell = segment_intersects_hive_cell((previous_x, previous_y), (projectile.x, projectile.y), red_untipped);
            let in_blue_cell = segment_intersects_hive_cell((previous_x, previous_y), (projectile.x, projectile.y), blue_untipped);
            // Nectar retains its originating alliance. It cannot score in the
            // opposing alliance's cell, and its color/ownership is never rewritten.
            let valid_red_score = in_red_cell && projectile.hive != Some(HiveSide::Blue);
            let valid_blue_score = in_blue_cell && projectile.hive != Some(HiveSide::Red);
            if valid_red_score || valid_blue_score {
                let (side, center) = if valid_red_score {(HiveSide::Red,red_untipped)} else {(HiveSide::Blue,blue_untipped)};
                let (pollen_count, nectar_count) = match side {
                    HiveSide::Red => {
                        match projectile.kind { ElementKind::Pollen => self.red_hive_pollen=self.red_hive_pollen.saturating_add(1), ElementKind::Nectar => self.red_hive_extra_nectar=self.red_hive_extra_nectar.saturating_add(1) }
                        (self.red_hive_pollen,self.red_hive_extra_nectar)
                    }
                    HiveSide::Blue => {
                        match projectile.kind { ElementKind::Pollen => self.blue_hive_pollen=self.blue_hive_pollen.saturating_add(1), ElementKind::Nectar => self.blue_hive_extra_nectar=self.blue_hive_extra_nectar.saturating_add(1) }
                        (self.blue_hive_pollen,self.blue_hive_extra_nectar)
                    }
                };

                // Place the visible scored piece fully inside the scoring cell,
                // even when the projectile first crossed the boundary at its edge.
                // Scored pieces remain visible but are no longer physical loose pieces.
                let scored_radius = match projectile.kind {
                    ElementKind::Pollen => POLLEN_RADIUS_IN,
                    ElementKind::Nectar => NECTAR_RADIUS_IN,
                };
                let (min_x, max_x, min_y, max_y) = hive_cell_bounds(center);
                self.field_pollen.push(Pollen {
                    x: projectile.x.clamp(min_x + scored_radius, max_x - scored_radius),
                    y: projectile.y.clamp(min_y + scored_radius, max_y - scored_radius),
                    radius: scored_radius,
                    from_flower: false,
                    scored: true,
                    kind: projectile.kind,
                    hive: Some(side),
                    vx: 0.0,
                    vy: 0.0,
                });

                // The three starting Nectar pieces exist only in each alliance's
                // initially active cell, and are counted only for its first tip.
                let starting_nectar_load = match side {
                    HiveSide::Red if self.red_hive_tip_count == 0 => HIVE_STARTING_NECTAR as f32 * HIVE_NECTAR_LOAD_UNITS,
                    HiveSide::Blue if self.blue_hive_tip_count == 0 => HIVE_STARTING_NECTAR as f32 * HIVE_NECTAR_LOAD_UNITS,
                    _ => 0.0,
                };
                let load=starting_nectar_load+nectar_count as f32*HIVE_NECTAR_LOAD_UNITS+pollen_count as f32*HIVE_POLLEN_LOAD_UNITS;
                self.pollen_projectile=None;
                if load>=HIVE_TIP_LOAD_UNITS {
                    match side {
                        HiveSide::Red => {
                            self.red_hive_tipped = !self.red_hive_tipped;
                            self.red_hive_tip_count += 1;
                            self.red_hive_pollen = 0;
                            self.red_hive_extra_nectar = 0;
                        }
                        HiveSide::Blue => {
                            self.blue_hive_tipped = !self.blue_hive_tipped;
                            self.blue_hive_tip_count += 1;
                            self.blue_hive_pollen = 0;
                            self.blue_hive_extra_nectar = 0;
                        }
                    }
                    // The old active cell becomes the tipped cell. Release its
                    // previously scored pieces so they can collide with and be
                    // collected by the robot again.
                    for element in &mut self.field_pollen {
                        if element.scored && element.hive == Some(side) {
                            element.scored = false;
                        }
                    }
                    self.pollen_status=format!("{} hive tipped. Its opposite fixed cell is now untipped.",if side==HiveSide::Red{"Red"}else{"Blue"});
                } else {
                    self.pollen_status=format!("{} hive: {:.1}/8.0 load units toward tipping.",if side==HiveSide::Red{"Red"}else{"Blue"},load);
                }
                return;
            }
            if projectile.elapsed >= SHOT_FLIGHT_SECONDS
                || projectile.x < POLLEN_RADIUS_IN
                || projectile.x > FIELD_SIZE_IN - POLLEN_RADIUS_IN
                || projectile.y < POLLEN_RADIUS_IN
                || projectile.y > FIELD_SIZE_IN - POLLEN_RADIUS_IN
            {
                let landed = Pollen {
                    x: projectile.x.clamp(POLLEN_RADIUS_IN, FIELD_SIZE_IN - POLLEN_RADIUS_IN),
                    y: projectile.y.clamp(POLLEN_RADIUS_IN, FIELD_SIZE_IN - POLLEN_RADIUS_IN),
                    radius: match projectile.kind { ElementKind::Pollen => POLLEN_RADIUS_IN, ElementKind::Nectar => NECTAR_RADIUS_IN },
                    from_flower: false,
                    scored: false,
                    kind: projectile.kind,
                    hive: projectile.hive,
                    vx: projectile.vx * 0.18,
                    vy: projectile.vy * 0.18,
                };
                self.field_pollen.push(landed);
                self.pollen_projectile = None;
                self.pollen_status = format!("{} landed back on the field. {} element(s) onboard.", match projectile.kind { ElementKind::Pollen => "Pollen", ElementKind::Nectar => "Nectar" }, self.carried_elements.len());
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
            "pollen" | "pollencount" | "pollen_count" | "robot.pollen" => return format!("{}/4 total elements", self.carried_elements.len()),
            "shootervelocity" | "shooter_velocity" | "outtaker.getvelocity()" | "shooter.getvelocity()" => {
                return self.detected_shooter_command()
                    .map(|(kind, value)| format!("{kind}: {value}"))
                    .unwrap_or_else(|| "No shooter command found".to_owned());
            },
            "shooterpower" | "shooter_power" | "outtaker.getpower()" | "shooter.getpower()" => {
                return self.detected_shooter_power()
                    .map(|value| format!("{value:.2}"))
                    .unwrap_or_else(|| "Unavailable".to_owned());
            },
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
            if key.contains("outtaker") || key.contains("shooter") {
                return self.detected_shooter_command().map(|(kind, value)| format!("{kind}: {value}")).unwrap_or_else(|| "No shooter command found".to_owned());
            }
            let motor = key.split('.').next().unwrap_or("");
            let index = if motor.contains("frontleft") { Some(0) }
                else if motor.contains("frontright") { Some(1) }
                else if motor.contains("backleft") { Some(2) }
                else if motor.contains("backright") { Some(3) } else { None };
            if let Some(i) = index { return format!("{:.1} in/s", self.drive_wheel_velocity[i]); }
            return format!("{} (no matching simulated motor)", expression);
        }
        if key.contains(".getpower()") {
            if key.contains("outtaker") || key.contains("shooter") {
                return format!("{:.2}", self.detected_shooter_power().unwrap_or(0.0));
            }
            let motor = key.split('.').next().unwrap_or("");
            let index = if motor.contains("frontleft") { Some(0) }
                else if motor.contains("frontright") { Some(1) }
                else if motor.contains("backleft") { Some(2) }
                else if motor.contains("backright") { Some(3) } else { None };
            if let Some(i) = index { return format!("{:.2}", self.drive_wheel_power[i]); }
        }

        format!("{} (expression not evaluated)", expression)
    }

    fn update_simulation(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.05);
        self.last_frame = now;
        self.flower_intake_cooldown = (self.flower_intake_cooldown - dt).max(0.0);
        self.intake_cycle_timer = (self.intake_cycle_timer - dt).max(0.0);
        if self.intake_running && self.intake_cycle_timer <= 0.0 {
            self.intake_pollen();
            self.intake_cycle_timer = 0.4;
        } else if !self.intake_running {
            self.intake_cycle_timer = 0.0;
        }
        let flywheel_rate = if self.flywheel_target_velocity > self.flywheel_velocity { 1800.0 } else { 900.0 };
        self.flywheel_velocity = approach(self.flywheel_velocity, self.flywheel_target_velocity, flywheel_rate * dt);
        self.shot_cooldown = (self.shot_cooldown - dt).max(0.0);
        if self.teleop_active && self.feeder_running && self.shot_cooldown <= 0.0 {
            self.shoot_pollen();
        }
        self.update_pollen(dt);
        self.update_loose_pollen_physics(dt);
        if self.pollen_projectile.is_some() || self.flower_intake_cooldown > 0.0 || self.intake_running || self.flywheel_velocity != self.flywheel_target_velocity || self.shot_cooldown > 0.0 {
            ctx.request_repaint();
        }

        if self.teleop_active {
            self.poll_controller();

            // Mecanum inverse kinematics: mix chassis commands into four wheel powers.
            let f = self.stick_forward;
            let s = self.stick_strafe;
            let r = self.stick_turn;
            let mut wheels = [f + s + r, f - s - r, f - s + r, f + s - r];
            let largest = wheels.iter().fold(1.0_f32, |m, p| m.max(p.abs()));
            for power in &mut wheels { *power /= largest; }
            self.drive_wheel_power = wheels;
            self.drive_wheel_velocity = wheels.map(|power| power * MAX_SPEED_IN_S);

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
                .clamp(9.0, FIELD_SIZE_IN - 9.0);
            self.resolve_robot_wall_collisions();
            self.heading_deg = (self.heading_deg + self.turn_velocity * dt).rem_euclid(360.0);
            self.resolve_robot_wall_collisions();
            self.elapsed_seconds += dt;
            ctx.request_repaint();
        } else if self.is_running {
            let h = self.heading_deg.to_radians();
            self.robot_x = (self.robot_x + h.sin() * self.drive_speed * dt)
                .clamp(ROBOT_SIZE_IN / 2.0, FIELD_SIZE_IN - ROBOT_SIZE_IN / 2.0);
            self.robot_y = (self.robot_y + h.cos() * self.drive_speed * dt)
                .clamp(9.0, FIELD_SIZE_IN - 9.0);
            self.resolve_robot_wall_collisions();
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

        // Highlight the exact 22 x 14 inch rectangles used by the scoring
        // collision test. Only the currently untipped cell for each alliance
        // is scoreable; the paired tipped cell is intentionally not outlined.
        let score_zones = [
            (hive_pair_centers(HiveSide::Red, self.red_hive_tipped).0, Color32::from_rgb(235, 65, 75), "RED SCORE ZONE"),
            (hive_pair_centers(HiveSide::Blue, self.blue_hive_tipped).0, Color32::from_rgb(65, 145, 255), "BLUE SCORE ZONE"),
        ];
        for (zone_center, color, label) in score_zones {
            let (min_x, max_x, min_y, max_y) = hive_cell_bounds(zone_center);
            let min = Pos2::new(
                field.left() + min_x * scale,
                field.bottom() - max_y * scale,
            );
            let max = Pos2::new(
                field.left() + max_x * scale,
                field.bottom() - min_y * scale,
            );
            let zone_rect = Rect::from_min_max(min, max);
            painter.rect_filled(zone_rect, 0.0, Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 35));
            painter.rect_stroke(zone_rect, 0.0, Stroke::new(2.0_f32, color), egui::StrokeKind::Inside);
            painter.text(
                Pos2::new(zone_rect.center().x, zone_rect.top() + 9.0),
                Align2::CENTER_TOP,
                label,
                FontId::proportional(10.0),
                color,
            );
        }

        // Scored elements remain at their original radius and landing position.

        // Draw smaller pollen first so larger pieces are layered over them.
        let mut pollen_draw_order = self.field_pollen.iter().collect::<Vec<_>>();
        pollen_draw_order.sort_by(|a, b| a.radius.total_cmp(&b.radius));
        for pollen in pollen_draw_order {
            let position = Pos2::new(
                field.left() + pollen.x * scale,
                field.bottom() - pollen.y * scale,
            );
            let radius = pollen.radius * scale;
            let color = match pollen.kind {
                ElementKind::Pollen => Color32::from_rgb(255, 218, 35),
                ElementKind::Nectar => match pollen.hive {
                    Some(HiveSide::Red) => Color32::from_rgb(235, 55, 65),
                    Some(HiveSide::Blue) => Color32::from_rgb(55, 125, 245),
                    None => Color32::from_rgb(185, 195, 210),
                },
            };
            painter.circle_filled(position,radius,color);
            painter.circle_stroke(position,radius,Stroke::new(1.0_f32,Color32::from_rgb(65,75,90)));
        }

        if let Some(projectile) = self.pollen_projectile {
            let flight = (projectile.elapsed / SHOT_FLIGHT_SECONDS).clamp(0.0, 1.0);
            let arc_height = 18.0 * 4.0 * flight * (1.0 - flight);
            let position = Pos2::new(
                field.left() + projectile.x * scale,
                field.bottom() - projectile.y * scale - arc_height * scale,
            );
            let radius=match projectile.kind {ElementKind::Pollen=>POLLEN_RADIUS_IN,ElementKind::Nectar=>NECTAR_RADIUS_IN};
            let color=match projectile.kind {
                ElementKind::Pollen=>Color32::from_rgb(255,235,90),
                ElementKind::Nectar=>match projectile.hive {
                    Some(HiveSide::Red)=>Color32::from_rgb(235,55,65),
                    Some(HiveSide::Blue)=>Color32::from_rgb(55,125,245),
                    None=>Color32::from_rgb(185,195,210),
                },
            };
            painter.circle_filled(position,radius*scale,color);
            painter.circle_stroke(position,radius*scale,Stroke::new(1.2_f32,Color32::WHITE));
        }

        let center = Pos2::new(field.left() + self.robot_x * scale, field.bottom() - self.robot_y * scale);
        let h = self.heading_deg.to_radians();
        // Local robot coordinates: +X is right, +Y is forward. Heading zero faces up-field.
        let transform = |x: f32, y: f32| -> Pos2 {
            let right_x = h.cos();
            let right_y = h.sin();
            let forward_x = h.sin();
            let forward_y = -h.cos();
            Pos2::new(
                center.x + (x * right_x + y * forward_x) * scale,
                center.y + (x * right_y + y * forward_y) * scale,
            )
        };

        // Chamfered chassis, rotated as one rigid body with the robot heading.
        let chassis_outline = [
            (-7.0, -9.0), (7.0, -9.0), (9.0, -7.0), (9.0, 7.0),
            (7.0, 9.0), (-7.0, 9.0), (-9.0, 7.0), (-9.0, -7.0),
        ].iter().map(|(x, y)| transform(*x, *y)).collect::<Vec<_>>();
        painter.add(egui::Shape::convex_polygon(
            chassis_outline, Color32::from_rgb(42, 53, 65),
            Stroke::new(1.8_f32, Color32::from_rgb(190, 211, 224)),
        ));

        // Four visible drive wheels along the chassis sides.
        for x in [-9.2, 9.2] {
            for y in [-5.5, 5.5] {
                let wheel = [
                    transform(x - 1.0, y - 2.8), transform(x + 1.0, y - 2.8),
                    transform(x + 1.0, y + 2.8), transform(x - 1.0, y + 2.8),
                ];
                painter.add(egui::Shape::convex_polygon(
                    wheel.to_vec(), Color32::from_rgb(18, 22, 28),
                    Stroke::new(1.0_f32, Color32::from_rgb(125, 145, 160)),
                ));
                painter.line_segment(
                    [transform(x - 0.7, y), transform(x + 0.7, y)],
                    Stroke::new(1.0_f32, Color32::from_rgb(90, 110, 125)),
                );
            }
        }

        // Center electronics / battery plate.
        let deck = [
            transform(-5.8, -5.8), transform(5.8, -5.8),
            transform(5.8, 5.8), transform(-5.8, 5.8),
        ];
        painter.add(egui::Shape::convex_polygon(
            deck.to_vec(), Color32::from_rgb(65, 88, 105),
            Stroke::new(1.0_f32, Color32::from_rgb(130, 165, 185)),
        ));
        // Center-mounted blue ball / mechanism marker.
        painter.circle_filled(center, 2.0 * scale, Color32::from_rgb(80, 145, 220));

        // Front intake bar makes the robot's forward direction immediately visible.
        let intake = [
            transform(-5.8, 8.0), transform(5.8, 8.0),
            transform(5.8, 9.8), transform(-5.8, 9.8),
        ];
        painter.add(egui::Shape::convex_polygon(
            intake.to_vec(), Color32::from_rgb(240, 177, 54),
            Stroke::new(1.0_f32, Color32::WHITE),
        ));
        painter.text(rect.left_top() + Vec2::new(7.0, 7.0), Align2::LEFT_TOP,
            "BIOBUZZ FIELD", FontId::proportional(12.0), Color32::LIGHT_GRAY);
        painter.text(rect.right_bottom() - Vec2::new(7.0, 7.0), Align2::RIGHT_BOTTOM,
            "144 in × 144 in", FontId::proportional(11.0), Color32::LIGHT_GRAY);
    }
}



// Find FTC motor variables initialized through hardwareMap.get(...).
// Supports common Java and Kotlin one-line declarations/assignments.
fn find_gamepad_actions(source: &str) -> Vec<(String, String)> {
    crate::ftc_interpreter::interpret(source).button_actions
}

fn find_outtake_commands(source: &str) -> Vec<(String, String, String, String)> {
    let program = crate::ftc_interpreter::interpret(source);
    let motors = program.motors.clone();
    program.commands.into_iter().filter_map(|command| {
        let motor = motors.iter().find(|m| m.variable == command.motor)?;
        matches!(&motor.role, crate::ftc_interpreter::MotorRole::Outtake)
            .then_some((motor.variable.clone(), motor.hardware_name.clone(), command.method, command.expression))
    }).collect()
}

fn hive_pair_centers(side: HiveSide, first_cell_tipped: bool) -> ((f32,f32),(f32,f32)) {
    let (a,b)=match side {HiveSide::Red=>(RED_HIVE_A,RED_HIVE_B),HiveSide::Blue=>(BLUE_HIVE_A,BLUE_HIVE_B)};
    if first_cell_tipped {(b,a)} else {(a,b)}
}

fn hive_cell_bounds(center: (f32, f32)) -> (f32, f32, f32, f32) {
    let min_x = center.0 - HIVE_CELL_HALF_WIDTH_IN;
    let max_x = center.0 + HIVE_CELL_HALF_WIDTH_IN;
    let mut min_y = center.1 - HIVE_CELL_HALF_DEPTH_IN;
    let mut max_y = center.1 + HIVE_CELL_HALF_DEPTH_IN;

    // The four field-specific scoring rectangles have one shortened Y edge.
    if center == RED_HIVE_A || center == BLUE_HIVE_B {
        min_y += 1.0; // Bottom edge inset by one inch.
    } else if center == RED_HIVE_B || center == BLUE_HIVE_A {
        max_y -= 1.0; // Top edge inset by one inch.
    }
    (min_x, max_x, min_y, max_y)
}

fn point_in_hive_cell(point: (f32, f32), center: (f32, f32)) -> bool {
    let (min_x, max_x, min_y, max_y) = hive_cell_bounds(center);
    point.0 >= min_x && point.0 <= max_x && point.1 >= min_y && point.1 <= max_y
}

fn segment_intersects_hive_cell(start: (f32, f32), end: (f32, f32), center: (f32, f32)) -> bool {
    let (min_x, max_x, min_y, max_y) = hive_cell_bounds(center);
    segment_intersects_bounds(start, end, min_x, max_x, min_y, max_y)
}

fn segment_intersects_bounds(
    start: (f32, f32),
    end: (f32, f32),
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
) -> bool {
    // Liang-Barsky segment clipping against an axis-aligned hive cell.
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let mut t_min: f32 = 0.0;
    let mut t_max: f32 = 1.0;
    for (p, q) in [
        (-dx, start.0 - min_x),
        ( dx, max_x - start.0),
        (-dy, start.1 - min_y),
        ( dy, max_y - start.1),
    ] {
        if p.abs() < f32::EPSILON {
            if q < 0.0 { return false; }
        } else {
            let t = q / p;
            if p < 0.0 { t_min = t_min.max(t); }
            else { t_max = t_max.min(t); }
            if t_min > t_max { return false; }
        }
    }
    true
}

fn collide_element_with_hive(element: &mut Pollen, center: (f32, f32), side: HiveSide, tipped: bool) {
    if tipped { return; }

    let (min_x, max_x, min_y, max_y) = hive_cell_bounds(center);

    if element.hive == Some(side) && point_in_hive_cell((element.x, element.y), center) {
        // Preloaded Nectar stays inside its own fixed cell until that cell tips.
        let min_x = min_x + element.radius;
        let max_x = max_x - element.radius;
        let min_y = min_y + element.radius;
        let max_y = max_y - element.radius;
        if element.x < min_x { element.x = min_x; element.vx = element.vx.abs() * 0.35; }
        else if element.x > max_x { element.x = max_x; element.vx = -element.vx.abs() * 0.35; }
        if element.y < min_y { element.y = min_y; element.vy = element.vy.abs() * 0.35; }
        else if element.y > max_y { element.y = max_y; element.vy = -element.vy.abs() * 0.35; }
        return;
    }

    // All other loose scoring elements collide with the outside of the
    // invisible 22 x 14 inch hive perimeter. The robot does not collide.
    let closest_x = element.x.clamp(min_x, max_x);
    let closest_y = element.y.clamp(min_y, max_y);
    let dx = element.x - closest_x;
    let dy = element.y - closest_y;
    let distance = dx.hypot(dy);
    if distance < element.radius {
        if distance > 0.001 {
            let nx = dx / distance;
            let ny = dy / distance;
            let overlap = element.radius - distance;
            element.x += nx * overlap;
            element.y += ny * overlap;
            let into_wall = element.vx * nx + element.vy * ny;
            if into_wall < 0.0 {
                element.vx -= 1.35 * into_wall * nx;
                element.vy -= 1.35 * into_wall * ny;
            }
        } else {
            // Center landed inside the rectangle: push it through the
            // nearest face so it cannot become trapped inside the wall.
            let distances = [
                (element.x - min_x, -1.0_f32, 0.0_f32),
                (max_x - element.x, 1.0, 0.0),
                (element.y - min_y, 0.0, -1.0),
                (max_y - element.y, 0.0, 1.0),
            ];
            let (depth, nx, ny) = distances.into_iter().min_by(|a,b| a.0.total_cmp(&b.0)).unwrap();
            let push = depth + element.radius;
            element.x += nx * push;
            element.y += ny * push;
            let into_wall = element.vx * nx + element.vy * ny;
            if into_wall < 0.0 {
                element.vx -= 1.35 * into_wall * nx;
                element.vy -= 1.35 * into_wall * ny;
            }
        }
    }
}

fn starting_field_pollen() -> Vec<Pollen> {
    let mut pollen = Vec::with_capacity(24);

    // Each Flower has four overlapping pollen pieces. The progressively smaller
    // circles make the stack look like pollen being drawn into the intake.
    let flower_radii = [1.4_f32, 1.05, 0.75];
    let edge_offset = flower_radii[0];
    let flower_positions = [
        (96.5, edge_offset), // Bottom: move inward by the largest pollen radius
        (FIELD_SIZE_IN - edge_offset, 96.5), // Right
        (FIELD_SIZE_IN - 96.5, FIELD_SIZE_IN - edge_offset), // Top, mirrored across 144
        (edge_offset, FIELD_SIZE_IN - 96.5), // Left, mirrored across 144
    ];
    for (x, y) in flower_positions {
        for radius in flower_radii {
            pollen.push(Pollen { x, y, radius, from_flower: true, scored: false, kind: ElementKind::Pollen, hive: None, vx: 0.0, vy: 0.0 });
        }
    }

    // Four pollen in each alliance Garden, staged in opposite corners.
    for offset in [0.0_f32, 4.5, 9.0, 13.5] {
        pollen.push(Pollen { x: offset, y: POLLEN_RADIUS_IN, radius: POLLEN_RADIUS_IN, from_flower: false, scored: false, kind: ElementKind::Pollen, hive: None, vx: 0.0, vy: 0.0 });
        pollen.push(Pollen { x: FIELD_SIZE_IN - offset, y: FIELD_SIZE_IN - POLLEN_RADIUS_IN, radius: POLLEN_RADIUS_IN, from_flower: false, scored: false, kind: ElementKind::Pollen, hive: None, vx: 0.0, vy: 0.0 });
    }

    // At match start, only the initially untipped cells contain their
    // three starting Nectar: Red at (57,59), Blue at (86,85).
    for (offset_x, offset_y) in [(-1.8_f32, -1.0_f32), (1.8, -1.0), (0.0, 2.0)] {
        for (center, side) in [(RED_HIVE_A,HiveSide::Red),(BLUE_HIVE_A,HiveSide::Blue)] {
            pollen.push(Pollen{x:center.0+offset_x,y:center.1+offset_y,radius:NECTAR_RADIUS_IN,from_flower:false,scored:false,kind:ElementKind::Nectar,hive:Some(side),vx:0.0,vy:0.0});
        }
    }

    // The opposite cells receive their starting Nectar only when they become untipped.
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
        let keyboard_intake = ctx.input(|input| input.key_pressed(egui::Key::E));
        let keyboard_shoot = ctx.input(|input| input.key_pressed(egui::Key::Space));
        if keyboard_intake { self.intake_pollen(); }
        if keyboard_shoot { self.shoot_pollen(); }

        let pressed_buttons = if self.teleop_active { self.poll_controller() } else { Vec::new() };
        let actions = self.detected_gamepad_actions();
        self.intake_running = pressed_buttons.iter().any(|button| actions.iter().any(|(mapped, action)| mapped == button && action.contains("intake")));
        self.feeder_running = pressed_buttons.iter().any(|button| actions.iter().any(|(mapped, action)| mapped == button && action.contains("feeder")));
        let newly_pressed = pressed_buttons.iter()
            .filter(|button| !self.previous_gamepad_buttons.contains(button))
            .cloned()
            .collect::<Vec<_>>();
        self.previous_gamepad_buttons = pressed_buttons;
        for button in newly_pressed {
            if let Some((_, action)) = actions.iter().find(|(mapped, _)| mapped == &button) {
                self.run_gamepad_action(&button, action);
            }
        }
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
            let controller_options = self.gamepad.as_ref().map(|gilrs| {
                gilrs.gamepads()
                    .filter(|(_, gamepad)| gamepad.is_connected())
                    .map(|(id, gamepad)| (id, format!("{} (ID {:?})", gamepad.name(), id)))
                    .collect::<Vec<_>>()
            }).unwrap_or_default();
            if controller_options.is_empty() {
                ui.label("No controllers detected.");
            } else {
                let selected_label = self.selected_gamepad
                    .and_then(|selected| controller_options.iter().find(|(id, _)| *id == selected).map(|(_, name)| name.clone()))
                    .unwrap_or_else(|| "Select controller".to_owned());
                egui::ComboBox::from_id_salt("controller_select")
                    .selected_text(selected_label)
                    .show_ui(ui, |ui| {
                        for (id, name) in &controller_options {
                            if ui.selectable_label(self.selected_gamepad == Some(*id), name).clicked() {
                                self.selected_gamepad = Some(*id);
                                self.previous_gamepad_buttons.clear();
                                self.stick_forward = 0.0;
                                self.stick_strafe = 0.0;
                                self.stick_turn = 0.0;
                            }
                        }
                    });
            }
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
            ui.collapsing("Drive motor power / velocity", |ui| {
                for (name, i) in [("Front left", 0), ("Front right", 1), ("Back left", 2), ("Back right", 3)] {
                    ui.label(format!("{name}: power {:+.2} · velocity {:+.1} in/s",
                        self.drive_wheel_power[i], self.drive_wheel_velocity[i]));
                }
            });
            ui.add_space(8.0);
            ui.separator();
            ui.heading("Pollen");
            ui.strong(format!("Carrying: {}/4", self.carried_elements.len()));
            let pc=self.carried_elements.iter().filter(|item|item.kind==ElementKind::Pollen).count();
            let nc=self.carried_elements.iter().filter(|item|item.kind==ElementKind::Nectar).count();
            ui.label(format!("Pollen: {pc} · Nectar: {nc}"));
            ui.label(format!("Shoot order: {}",self.carried_elements.iter().map(|item|match item.kind{ElementKind::Pollen=>"P",ElementKind::Nectar=>"N"}).collect::<Vec<_>>().join(" → ")));
            ui.label(format!("Flywheel: {:.0}/{:.0} ticks/s", self.flywheel_velocity, self.flywheel_target_velocity));
            ui.add(egui::Slider::new(&mut self.shooter_velocity_scale, 0.005..=0.06).text("Shooter calibration (in/s per tick/s)"));
            ui.label(format!("Intake: {} · Feeder: {}", if self.intake_running { "active" } else { "off" }, if self.feeder_running { "active" } else { "off" }));
            ui.label(&self.pollen_status);
            if let Some(mode) = self.selected_op_mode.and_then(|i| self.op_modes.get(i)) {
                let outtakes = find_outtake_commands(&mode.source);
                if outtakes.is_empty() {
                    ui.label("No outtake motor commands detected. Expected names such as OuttakeL / OuttakeR.");
                } else {
                    ui.label("Automatically detected outtake motors:");
                    for (variable, hardware, kind, value) in outtakes {
                        ui.label(format!("{variable} (hardware \"{hardware}\"): {kind}({value})"));
                    }
                }
            } else {
                ui.label("Select an OpMode to detect outtake motors automatically.");
            }
            ui.label("Outtake commands are read from the selected OpMode. Projectile speed still requires wheel/gearing calibration.");
            ui.horizontal(|ui| {
                if ui.button("Intake pollen [E]").clicked() {
                    self.intake_pollen();
                }
                if ui.button("Shoot pollen [Space]").clicked() {
                    self.shoot_pollen();
                }
            });
            ui.label("Move the front intake close to a yellow pollen ball, then intake.");
            ui.collapsing("Detected gamepad button actions", |ui| {
                let actions = self.detected_gamepad_actions();
                if actions.is_empty() {
                    ui.label("No gamepad1 button if-blocks detected in the selected OpMode.");
                } else {
                    for (button, action) in actions {
                        let label = match button.as_str() {
                            "south" => "Bottom face button (A / Cross)",
                            "east" => "Right face button (B / Circle)",
                            "west" => "Left face button (X / Square)",
                            "north" => "Top face button (Y / Triangle)",
                            other => other,
                        };
                        ui.label(format!("{label} → {action}"));
                    }
                    ui.label("Button actions are checked while TeleOp is running.");
                }
            });
            ui.collapsing("FTC interpreter: detected hardware and commands", |ui| {
                if let Some(mode) = self.selected_op_mode.and_then(|i| self.op_modes.get(i)) {
                    let program = crate::ftc_interpreter::interpret(&mode.source);
                    if program.motors.is_empty() {
                        ui.label("No supported hardwareMap motor declarations found.");
                    }
                    for motor in &program.motors {
                        let role = match &motor.role {
                            crate::ftc_interpreter::MotorRole::Drive(index) => ["Front left", "Front right", "Back left", "Back right"][*index],
                            crate::ftc_interpreter::MotorRole::Intake => "Intake",
                            crate::ftc_interpreter::MotorRole::Feeder => "Feeder",
                            crate::ftc_interpreter::MotorRole::Outtake => "Outtake",
                            crate::ftc_interpreter::MotorRole::Other => "Other motor",
                        };
                        ui.label(format!("{} -> {} ({})", motor.variable, motor.hardware_name, role));
                    }
                    ui.separator();
                    for command in &program.commands {
                        let button = command.button.as_deref().unwrap_or("always / no button block");
                        ui.label(format!("{}: {}({}) [{}]", command.motor, command.method, command.expression, button));
                    }
                    for diagnostic in &program.diagnostics { ui.label(diagnostic); }
                } else {
                    ui.label("Import and select an OpMode to interpret its FTC hardware and commands.");
                }
            });
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
