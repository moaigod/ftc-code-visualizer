use std::time::Instant;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, TextureHandle, Vec2};
use gilrs::{Axis, Button, Gilrs};
use crate::importer::{self, OpModeInfo};

const FIELD_SIZE_IN: f32 = 144.0;
const ROBOT_SIZE_IN: f32 = 18.0;
const MAX_SPEED_IN_S: f32 = 48.0;
const MAX_TURN_DEG_S: f32 = 270.0;
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
    vz: f32,
    z: f32,
    elapsed: f32,
    kind: ElementKind,
    hive: Option<HiveSide>,
    can_score_red: bool,
    can_score_blue: bool,
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
    pollen_projectiles: Vec<PollenProjectile>,
    pollen_status: String,
    drive_wheel_power: [f32; 4],
    drive_wheel_velocity: [f32; 4],
    previous_gamepad_buttons: Vec<String>,
    flower_intake_cooldown: f32,
    intake_running: bool,
    feeder_running: bool,
    intake_power: f32,
    feeder_power: f32,
    flywheel_velocity: f32,
    flywheel_target_velocity: f32,
    ramp_angle_deg: f32,
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
    autonomous_waypoints: Vec<(f32, f32)>,
    autonomous_headings: Vec<Option<f32>>,
    autonomous_waypoint_index: usize,
    autonomous_segments: Vec<crate::ftc_interpreter::AutonomousPathSegment>,
    autonomous_segment_index: usize,
    autonomous_post_actions_applied: bool,
    autonomous_action_index: usize,
    autonomous_action_delay_started: bool,
    autonomous_wait_remaining: f32,
    autonomous_waiting_for_condition: bool,
    autonomous_running: bool,
    autonomous_status: String,
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
            pollen_projectiles: Vec::new(),
            pollen_status: "4 preloaded pollen onboard.".to_owned(),
            drive_wheel_power: [0.0; 4],
            drive_wheel_velocity: [0.0; 4],
            previous_gamepad_buttons: Vec::new(),
            flower_intake_cooldown: 0.0,
            intake_running: false,
            feeder_running: false,
            intake_power: 0.0,
            feeder_power: 0.0,
            flywheel_velocity: 0.0,
            flywheel_target_velocity: 0.0,
            ramp_angle_deg: 35.0,
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
            autonomous_waypoints: Vec::new(),
            autonomous_headings: Vec::new(),
            autonomous_waypoint_index: 0,
            autonomous_segments: Vec::new(),
            autonomous_segment_index: 0,
            autonomous_post_actions_applied: false,
            autonomous_action_index: 0,
            autonomous_action_delay_started: false,
            autonomous_wait_remaining: 0.0,
            autonomous_waiting_for_condition: false,
            autonomous_running: false,
            autonomous_status: "No autonomous program running.".to_owned(),
        }
    }
}

impl VisualizerApp {
    fn reset(&mut self) {
        // Restore the simulated match to the same initial state every time.
        // Keep imported project selection and user configuration (test speed,
        // shooter scale, controller selection) intact.
        self.robot_x = 24.0;
        self.robot_y = 24.0;
        self.heading_deg = 0.0;
        self.elapsed_seconds = 0.0;
        self.is_running = false;
        self.teleop_active = false;
        self.autonomous_running = false;

        self.stick_forward = 0.0;
        self.stick_strafe = 0.0;
        self.stick_turn = 0.0;
        self.forward_velocity = 0.0;
        self.strafe_velocity = 0.0;
        self.turn_velocity = 0.0;
        self.drive_wheel_power = [0.0; 4];
        self.drive_wheel_velocity = [0.0; 4];
        self.previous_gamepad_buttons.clear();

        self.carried_elements = vec![
            CarriedElement { kind: ElementKind::Pollen, hive: None };
            MAX_POLLEN as usize
        ];
        self.field_pollen = starting_field_pollen();
        self.pollen_projectiles.clear();
        self.pollen_status = "4 preloaded pollen onboard.".to_owned();
        self.flower_intake_cooldown = 0.0;
        self.intake_running = false;
        self.feeder_running = false;
        self.intake_power = 0.0;
        self.feeder_power = 0.0;
        self.flywheel_velocity = 0.0;
        self.flywheel_target_velocity = 0.0;
        self.shot_cooldown = 0.0;
        self.intake_cycle_timer = 0.0;
        self.shot_sequence = 0;

        self.red_hive_pollen = 0;
        self.blue_hive_pollen = 0;
        self.red_hive_extra_nectar = 0;
        self.blue_hive_extra_nectar = 0;
        self.red_hive_tipped = false;
        self.blue_hive_tipped = false;
        self.red_hive_tip_count = 0;
        self.blue_hive_tip_count = 0;

        self.autonomous_waypoints.clear();
        self.autonomous_headings.clear();
        self.autonomous_waypoint_index = 0;
        self.autonomous_segments.clear();
        self.autonomous_segment_index = 0;
        self.autonomous_post_actions_applied = false;
        self.autonomous_action_index = 0;
        self.autonomous_action_delay_started = false;
        self.autonomous_wait_remaining = 0.0;
        self.autonomous_waiting_for_condition = false;
        self.autonomous_status = "Simulation reset to its initial state.".to_owned();
        self.last_frame = Instant::now();
    }

    fn run_autonomous(&mut self) {
        let Some(mode) = self.selected_op_mode.and_then(|i| self.op_modes.get(i)) else {
            self.autonomous_status = "Select an @Autonomous OpMode first.".to_owned();
            return;
        };
        if mode.kind != "Autonomous" {
            self.autonomous_status = "The selected OpMode is not annotated @Autonomous.".to_owned();
            return;
        }
        let plan = crate::ftc_interpreter::interpret_autonomous(&mode.source);
        if plan.waypoints.is_empty() && !plan.path_found {
            self.autonomous_status = plan.diagnostics.join(" ");
            return;
        }
        if !plan.follow_path_called {
            self.autonomous_status = plan.diagnostics.join(" ");
            return;
        }

        // A rerun starts a fresh match: restore the field, robot, cargo,
        // scoring state, mechanisms, timers, and path execution before
        // applying this OpMode's configured starting pose and startup actions.
        self.reset();

        if let Some((x, y, heading_rad)) = plan.start_pose {
            self.robot_x = x.clamp(9.0, FIELD_SIZE_IN - 9.0);
            self.robot_y = y.clamp(9.0, FIELD_SIZE_IN - 9.0);
            // PedroPathing heading is radians from +X; the field view uses
            // zero degrees toward +Y and positive rotation toward +X.
            self.heading_deg = (90.0 - heading_rad.to_degrees()).rem_euclid(360.0);
        }
        self.autonomous_segments = plan.path_segments.clone();
        self.autonomous_segment_index = 0;
        if let Some(first) = self.autonomous_segments.first() {
            self.autonomous_waypoints = first.waypoints.clone();
            self.autonomous_headings = first.headings.clone();
        } else {
            self.autonomous_waypoints = plan.waypoints;
            self.autonomous_headings = plan.waypoint_headings;
        }
        self.intake_running = false;
        self.feeder_running = false;
        self.intake_power = 0.0;
        self.feeder_power = 0.0;
        self.flywheel_target_velocity = 0.0;
        for action in &plan.startup_actions {
            let value = action.value.unwrap_or(0.0);
            match action.role {
                crate::ftc_interpreter::MotorRole::Intake if action.method == "setPower" => {
                    self.intake_power = value.abs().clamp(0.0, 1.0);
                    self.intake_running = self.intake_power > 0.0;
                }
                crate::ftc_interpreter::MotorRole::Feeder if action.method == "setPower" => {
                    self.feeder_power = value.abs().clamp(0.0, 1.0);
                    self.feeder_running = self.feeder_power > 0.0;
                }
                crate::ftc_interpreter::MotorRole::Outtake if action.method == "setVelocity" => {
                    self.flywheel_target_velocity = self.flywheel_target_velocity.max(value.max(0.0));
                }
                _ => {}
            }
        }
        self.autonomous_waypoint_index = 0;
        self.autonomous_post_actions_applied = false;
        self.autonomous_action_index = 0;
        self.autonomous_action_delay_started = false;
        self.autonomous_wait_remaining = 0.0;
        self.autonomous_waiting_for_condition = false;
        self.autonomous_running = true;
        self.is_running = false;
        self.teleop_active = false;
        self.forward_velocity = 0.0;
        self.strafe_velocity = 0.0;
        self.turn_velocity = 0.0;
        self.elapsed_seconds = 0.0;
        self.last_frame = Instant::now();
        self.autonomous_status = format!(
            "Running interpreted FTC autonomous: {} sampled path points. Java command groups, supported method actions, and waits are simulated; Android SDK calls are not executed.",
            self.autonomous_waypoints.len()
        );
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

    fn detected_motor_power_for_button(
        &self,
        button: &str,
        role: crate::ftc_interpreter::MotorRole,
    ) -> Option<f32> {
        let source = self.selected_op_mode
            .and_then(|i| self.op_modes.get(i))
            .map(|mode| mode.source.as_str())?;
        let program = crate::ftc_interpreter::interpret(source);
        program.commands.iter()
            .filter(|command| {
                command.button.as_deref() == Some(button)
                    && command.method == "setPower"
                    && program.motors.iter().any(|motor| {
                        motor.variable == command.motor && motor.role == role
                    })
            })
            .filter_map(|command| command.value)
            .last()
            .map(|value| value.abs().clamp(0.0, 1.0))
    }

    fn run_gamepad_action(&mut self, button: &str, action: &str) {
        let normalized = action.to_ascii_lowercase();
        if normalized.contains("intake") {
            self.intake_power = self.detected_motor_power_for_button(
                button,
                crate::ftc_interpreter::MotorRole::Intake,
            ).unwrap_or(1.0);
            self.intake_running = self.intake_power > 0.0;
        } else if normalized.contains("feeder") {
            self.feeder_running = true;
            self.feeder_power = 1.0;
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
            // Intake capture zone starts at the robot's front edge and
            // extends outward only: 75% of the previous width and 50% height.
            let cx=lx.clamp(-6.375,6.375);
            let cy=ly.clamp(9.0,12.0);
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
        if self.shot_cooldown>0.0{return;}
        let target=self.flywheel_target_velocity;
        if target<=0.0 {self.pollen_status="No positive setVelocity() target was resolved from the selected OpMode.".to_owned();return;}
        let min=(target*0.25).max(100.0);
        if self.flywheel_velocity<min {self.pollen_status=format!("Flywheel spinning up: {:.0}/{:.0} ticks/s (shots become available at {:.0}).",self.flywheel_velocity,target,min);return;}
        let heading=self.heading_deg.to_radians();
        let launch_x=self.robot_x+heading.sin()*(ROBOT_SIZE_IN*0.5+1.0);
        let launch_y=self.robot_y+heading.cos()*(ROBOT_SIZE_IN*0.5+1.0);
        let red_center=hive_pair_centers(HiveSide::Red,self.red_hive_tipped).0;
        let blue_center=hive_pair_centers(HiveSide::Blue,self.blue_hive_tipped).0;
        let can_score_red=point_in_scoring_cone((launch_x,launch_y),red_center,HiveSide::Red,self.red_hive_tipped);
        let can_score_blue=point_in_scoring_cone((launch_x,launch_y),blue_center,HiveSide::Blue,self.blue_hive_tipped);
        // Shots may be fired from anywhere, but only shots launched inside
        // the matching scoring cone are allowed to score.
        // FIFO: the first collected element is the first one launched.
        let cargo=self.carried_elements.remove(0);
        let kind=cargo.kind;
        const SPREAD:[f32;7]=[-2.2,1.1,2.0,-0.8,0.4,-1.5,1.6];
        let spread=SPREAD[self.shot_sequence as usize%SPREAD.len()].to_radians();
        self.shot_sequence=self.shot_sequence.wrapping_add(1);
        let launch=heading+spread;
        // Outtake velocity sets the base launch speed. A steeper ramp
        // reduces the effective exit speed, then redirects it vertically.
        let base_speed=(self.flywheel_velocity*0.0400).clamp(3.0,115.0);
        let angle=self.ramp_angle_deg.to_radians().clamp(0.0,1.40);
        let angle_fraction = (self.ramp_angle_deg / 80.0).clamp(0.0, 1.0);
        let speed = base_speed * (1.0 - 0.25 * angle_fraction);
        let horizontal_speed=speed*angle.cos();
        let vertical_speed=speed*angle.sin();
        self.pollen_projectiles.push(PollenProjectile{x:launch_x,y:launch_y,vx:launch.sin()*horizontal_speed,vy:launch.cos()*horizontal_speed,vz:vertical_speed,z:0.0,elapsed:0.0,kind,hive:cargo.hive,can_score_red,can_score_blue});
        let loss=(0.20+(target/3000.0)*0.25).clamp(0.20,0.45);
        self.flywheel_velocity*=1.0-loss;
        // Both mechanisms contribute to the feed rate. Averaging their
        // normalized powers avoids letting the slower motor completely
        // bottleneck the shooter, while increasing either motor's power
        // increases the overall firing cadence. Ramp angle only affects flight.
        let intake_rate = if has_intake { self.intake_power } else { 1.0 };
        let feeder_rate = if has_feeder { self.feeder_power } else { 1.0 };
        let feed_rate = ((intake_rate + feeder_rate) * 0.5).clamp(0.1, 1.0);
        self.shot_cooldown = 0.16 / feed_rate;
        let name=match kind {ElementKind::Pollen=>"Pollen",ElementKind::Nectar=>"Nectar"};
        self.pollen_status=format!("{name} fired FIFO. {} element(s) remain.",self.carried_elements.len());
    }

    fn update_loose_pollen_physics(&mut self, dt: f32) {
        // Loose pollen has light inertia, floor friction, wall collisions, and
        // can be pushed by the robot instead of passing through its chassis.
        let robot_speed = self.forward_velocity.hypot(self.strafe_velocity);
        let heading = self.heading_deg.to_radians();
        for pollen in &mut self.field_pollen {
            // Scored elements remain inside their active scoring cell, but can
            // drift, bounce off the hive walls, and collide with other scored pieces.
            if pollen.scored {
                pollen.x += pollen.vx * dt;
                pollen.y += pollen.vy * dt;
                let friction = (1.0 - 1.7 * dt).max(0.0);
                pollen.vx *= friction;
                pollen.vy *= friction;
                let center = match pollen.hive {
                    Some(HiveSide::Red) => hive_pair_centers(HiveSide::Red, self.red_hive_tipped).0,
                    Some(HiveSide::Blue) => hive_pair_centers(HiveSide::Blue, self.blue_hive_tipped).0,
                    None => (pollen.x, pollen.y),
                };
                let (min_x, max_x, min_y, max_y) = hive_cell_bounds(center);
                let min_x = min_x + pollen.radius;
                let max_x = max_x - pollen.radius;
                let min_y = min_y + pollen.radius;
                let max_y = max_y - pollen.radius;
                if pollen.x < min_x {
                    pollen.x = min_x;
                    pollen.vx = pollen.vx.abs() * 0.45;
                } else if pollen.x > max_x {
                    pollen.x = max_x;
                    pollen.vx = -pollen.vx.abs() * 0.45;
                }
                if pollen.y < min_y {
                    pollen.y = min_y;
                    pollen.vy = pollen.vy.abs() * 0.45;
                } else if pollen.y > max_y {
                    pollen.y = max_y;
                    pollen.vy = -pollen.vy.abs() * 0.45;
                }
                continue;
            }

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

            collide_element_with_robot(pollen, self.robot_x, self.robot_y, heading, robot_speed);

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
                // Flower-stack pollen is fixed. Scored pieces collide with
                // other scored pieces in the same hive, but not with loose field pieces.
                if a.from_flower || b.from_flower {
                    continue;
                }
                if a.scored || b.scored {
                    let same_scored_hive = a.scored && b.scored && a.hive.is_some() && a.hive == b.hive;
                    if !same_scored_hive { continue; }
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

        // Re-confine scored elements after pairwise collision resolution so
        // even a crowded hive cannot push them outside its scoring rectangle.
        for pollen in &mut self.field_pollen {
            if !pollen.scored { continue; }
            let center = match pollen.hive {
                Some(HiveSide::Red) => hive_pair_centers(HiveSide::Red, self.red_hive_tipped).0,
                Some(HiveSide::Blue) => hive_pair_centers(HiveSide::Blue, self.blue_hive_tipped).0,
                None => continue,
            };
            let (min_x, max_x, min_y, max_y) = hive_cell_bounds(center);
            pollen.x = pollen.x.clamp(min_x + pollen.radius, max_x - pollen.radius);
            pollen.y = pollen.y.clamp(min_y + pollen.radius, max_y - pollen.radius);
        }

        // Keep every loose scoring element inside the field, then resolve
        // robot contact again. Boundary correction can otherwise push an
        // element back through the chassis when it is trapped against a wall.
        for pollen in &mut self.field_pollen {
            if pollen.scored { continue; }
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

            if !pollen.from_flower {
                let inside_untipped_hive =
                    (pollen.hive == Some(HiveSide::Red)
                        && point_in_hive_cell((pollen.x, pollen.y), hive_pair_centers(HiveSide::Red, self.red_hive_tipped).0))
                    || (pollen.hive == Some(HiveSide::Blue)
                        && point_in_hive_cell((pollen.x, pollen.y), hive_pair_centers(HiveSide::Blue, self.blue_hive_tipped).0));
                if !inside_untipped_hive {
                    collide_element_with_robot(pollen, self.robot_x, self.robot_y, self.heading_deg.to_radians(), robot_speed);
                }
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
        let mut active_projectiles = Vec::with_capacity(self.pollen_projectiles.len());
        for mut projectile in std::mem::take(&mut self.pollen_projectiles) {
            projectile.elapsed += dt;
            let previous_x = projectile.x;
            let previous_y = projectile.y;
            projectile.x += projectile.vx * dt;
            projectile.y += projectile.vy * dt;
            projectile.z += projectile.vz * dt - 0.5 * 32.174 * dt * dt;
            projectile.vz -= 32.174 * dt;

            // Check the full movement segment so fast projectiles cannot skip a hive.
            let red_untipped = hive_pair_centers(HiveSide::Red, self.red_hive_tipped).0;
            let blue_untipped = hive_pair_centers(HiveSide::Blue, self.blue_hive_tipped).0;
            let in_red_cell = projectile.can_score_red
                && segment_intersects_hive_cell((previous_x, previous_y), (projectile.x, projectile.y), red_untipped);
            let in_blue_cell = projectile.can_score_blue
                && segment_intersects_hive_cell((previous_x, previous_y), (projectile.x, projectile.y), blue_untipped);
            let valid_red_score = in_red_cell && projectile.hive != Some(HiveSide::Blue);
            let valid_blue_score = in_blue_cell && projectile.hive != Some(HiveSide::Red);

            if valid_red_score || valid_blue_score {
                let (side, center) = if valid_red_score { (HiveSide::Red, red_untipped) } else { (HiveSide::Blue, blue_untipped) };
                let (pollen_count, nectar_count) = match side {
                    HiveSide::Red => {
                        match projectile.kind {
                            ElementKind::Pollen => self.red_hive_pollen = self.red_hive_pollen.saturating_add(1),
                            ElementKind::Nectar => self.red_hive_extra_nectar = self.red_hive_extra_nectar.saturating_add(1),
                        }
                        (self.red_hive_pollen, self.red_hive_extra_nectar)
                    }
                    HiveSide::Blue => {
                        match projectile.kind {
                            ElementKind::Pollen => self.blue_hive_pollen = self.blue_hive_pollen.saturating_add(1),
                            ElementKind::Nectar => self.blue_hive_extra_nectar = self.blue_hive_extra_nectar.saturating_add(1),
                        }
                        (self.blue_hive_pollen, self.blue_hive_extra_nectar)
                    }
                };

                let scored_radius = match projectile.kind { ElementKind::Pollen => POLLEN_RADIUS_IN, ElementKind::Nectar => NECTAR_RADIUS_IN };
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

                let starting_nectar_load = match side {
                    HiveSide::Red if self.red_hive_tip_count == 0 => HIVE_STARTING_NECTAR as f32 * HIVE_NECTAR_LOAD_UNITS,
                    HiveSide::Blue if self.blue_hive_tip_count == 0 => HIVE_STARTING_NECTAR as f32 * HIVE_NECTAR_LOAD_UNITS,
                    _ => 0.0,
                };
                let load = starting_nectar_load + nectar_count as f32 * HIVE_NECTAR_LOAD_UNITS
                    + pollen_count as f32 * HIVE_POLLEN_LOAD_UNITS;

                if load >= HIVE_TIP_LOAD_UNITS {
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
                    // The scoring pieces are released from the hive and
                    // scatter outward instead of remaining frozen in the cell.
                    let released_count = self.field_pollen.iter()
                        .filter(|element| element.scored && element.hive == Some(side))
                        .count()
                        .max(1);
                    let phase = (self.red_hive_tip_count + self.blue_hive_tip_count) as f32 * 0.37;
                    let mut released_index = 0usize;
                    for element in &mut self.field_pollen {
                        if element.scored && element.hive == Some(side) {
                            let angle = phase + std::f32::consts::TAU
                                * released_index as f32 / released_count as f32;
                            let speed = 13.0 + (released_index % 4) as f32 * 2.8;
                            element.scored = false;
                            element.vx = angle.cos() * speed;
                            element.vy = angle.sin() * speed;
                            released_index += 1;
                        }
                    }
                    self.pollen_status = format!("{} hive tipped. Scoring elements spilled out and scattered.", if side == HiveSide::Red { "Red" } else { "Blue" });
                } else {
                    self.pollen_status = format!("{} hive: {:.1}/8.0 load units toward tipping.", if side == HiveSide::Red { "Red" } else { "Blue" }, load);
                }
                continue;
            }

            if (projectile.elapsed > 0.0 && projectile.z <= 0.0)
                || projectile.x < POLLEN_RADIUS_IN
                || projectile.x > FIELD_SIZE_IN - POLLEN_RADIUS_IN
                || projectile.y < POLLEN_RADIUS_IN
                || projectile.y > FIELD_SIZE_IN - POLLEN_RADIUS_IN
            {
                self.field_pollen.push(Pollen {
                    x: projectile.x.clamp(POLLEN_RADIUS_IN, FIELD_SIZE_IN - POLLEN_RADIUS_IN),
                    y: projectile.y.clamp(POLLEN_RADIUS_IN, FIELD_SIZE_IN - POLLEN_RADIUS_IN),
                    radius: match projectile.kind { ElementKind::Pollen => POLLEN_RADIUS_IN, ElementKind::Nectar => NECTAR_RADIUS_IN },
                    from_flower: false,
                    scored: false,
                    kind: projectile.kind,
                    hive: projectile.hive,
                    vx: projectile.vx * 0.18,
                    vy: projectile.vy * 0.18,
                });
                self.pollen_status = format!("{} landed back on the field. {} element(s) onboard.", match projectile.kind { ElementKind::Pollen => "Pollen", ElementKind::Nectar => "Nectar" }, self.carried_elements.len());
            } else {
                active_projectiles.push(projectile);
            }
        }
        self.pollen_projectiles = active_projectiles;
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
            // Both outtake motors are represented by the shared simulated flywheel.
            // Match the variable name anywhere in the expression so Java casts such
            // as ((DcMotorEx)OuttakeL).getVelocity() are supported.
            if key.contains("outtake") || key.contains("shooter") || key.contains("flywheel") {
                return format!("{:.1} ticks/s", self.flywheel_velocity);
            }
            let index = if key.contains("frontleft") { Some(0) }
                else if key.contains("frontright") { Some(1) }
                else if key.contains("backleft") { Some(2) }
                else if key.contains("backright") { Some(3) } else { None };
            if let Some(i) = index { return format!("{:.1} in/s", self.drive_wheel_velocity[i]); }
            return format!("{} (no matching simulated motor)", expression);
        }
        if key.contains(".getpower()") {
            // Mechanism power is represented as the active command's power.
            if key.contains("intake") {
                return format!("{:.2}", if self.intake_running { 1.0 } else { 0.0 });
            }
            if key.contains("feeder") {
                return format!("{:.2}", if self.feeder_running { 1.0 } else { 0.0 });
            }
            if key.contains("outtake") || key.contains("shooter") || key.contains("flywheel") {
                return format!("{:.2}", if self.flywheel_target_velocity > 0.0 { 1.0 } else { 0.0 });
            }
            let index = if key.contains("frontleft") { Some(0) }
                else if key.contains("frontright") { Some(1) }
                else if key.contains("backleft") { Some(2) }
                else if key.contains("backright") { Some(3) } else { None };
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
            self.intake_cycle_timer = 0.4 / self.intake_power.clamp(0.1, 1.0);
        } else if !self.intake_running {
            self.intake_cycle_timer = 0.0;
        }
        let flywheel_rate = if self.flywheel_target_velocity > self.flywheel_velocity { 1800.0 } else { 900.0 };
        self.flywheel_velocity = approach(self.flywheel_velocity, self.flywheel_target_velocity, flywheel_rate * dt);
        self.shot_cooldown = (self.shot_cooldown - dt).max(0.0);
        if self.feeder_running && self.shot_cooldown <= 0.0 {
            self.shoot_pollen();
        }
        self.update_pollen(dt);
        self.update_loose_pollen_physics(dt);
        if !self.pollen_projectiles.is_empty() || self.flower_intake_cooldown > 0.0 || self.intake_running || self.flywheel_velocity != self.flywheel_target_velocity || self.shot_cooldown > 0.0 {
            ctx.request_repaint();
        }

        if self.autonomous_running {
            // Follow a short look-ahead target instead of stopping at every
            // sampled curve point. This avoids the visible stop/start motion
            // caused by snapping to each waypoint independently.
            while self.autonomous_waypoint_index < self.autonomous_waypoints.len() {
                let point = self.autonomous_waypoints[self.autonomous_waypoint_index];
                let distance = (point.0 - self.robot_x).hypot(point.1 - self.robot_y);
                if distance < 2.5 {
                    self.autonomous_waypoint_index += 1;
                } else {
                    break;
                }
            }

            if let Some(target) = self.autonomous_waypoints.get(self.autonomous_waypoint_index).copied() {
                let dx = target.0 - self.robot_x;
                let dy = target.1 - self.robot_y;
                let distance = dx.hypot(dy);
                // Ramp speed gradually so the robot does not jump to full
                // speed on the first simulation frame.
                self.forward_velocity = (self.forward_velocity + 85.0 * dt).min(34.0);
                let step = (self.forward_velocity * dt).min(distance);
                if distance > 0.001 {
                    self.robot_x += dx / distance * step;
                    self.robot_y += dy / distance * step;
                    let desired_heading = self.autonomous_headings
                        .get(self.autonomous_waypoint_index)
                        .and_then(|heading| *heading)
                        .map(|heading_rad| (90.0 - heading_rad.to_degrees()).rem_euclid(360.0))
                        .unwrap_or_else(|| dx.atan2(dy).to_degrees().rem_euclid(360.0));
                    let heading_delta = (desired_heading - self.heading_deg + 540.0).rem_euclid(360.0) - 180.0;
                    self.heading_deg = (self.heading_deg + heading_delta * (1.0 - (-14.0 * dt).exp())).rem_euclid(360.0);
                }
                self.resolve_robot_wall_collisions();
                self.elapsed_seconds += dt;
                ctx.request_repaint();
            } else {
                if let Some(segment) = self.autonomous_segments.get(self.autonomous_segment_index).cloned() {
                    if !self.autonomous_post_actions_applied {
                        // Execute post-path mechanism commands in source order.
                        // A waitMs between two method calls delays the second
                        // action instead of being moved to the end of the path.
                        let mut blocked = false;
                        while let Some(action) = segment.actions_after.get(self.autonomous_action_index) {
                            if action.delay_before_ms > 0 && !self.autonomous_action_delay_started {
                                self.autonomous_wait_remaining = action.delay_before_ms as f32 / 1000.0;
                                self.autonomous_action_delay_started = true;
                            }
                            if self.autonomous_action_delay_started {
                                self.autonomous_wait_remaining = (self.autonomous_wait_remaining - dt).max(0.0);
                                if self.autonomous_wait_remaining > 0.0 {
                                    blocked = true;
                                    break;
                                }
                                self.autonomous_action_delay_started = false;
                            }

                            let current_velocity = self.flywheel_velocity;
                            let resolve_condition = |condition: &str| {
                                crate::java_interpreter::evaluate_boolean(condition, &|operand| {
                                    let key = operand.to_ascii_lowercase();
                                    if key.contains("getvelocity")
                                        && (key.contains("outtake") || key.contains("shooter") || key.contains("flywheel"))
                                    {
                                        Some(current_velocity)
                                    } else {
                                        operand.trim().parse::<f32>().ok()
                                    }
                                }).unwrap_or(false)
                            };
                            if let Some(wait_until) = action.wait_until.as_deref() {
                                if !resolve_condition(wait_until) {
                                    self.autonomous_waiting_for_condition = true;
                                    self.autonomous_status = "Waiting for an autonomous waitUntil condition.".to_owned();
                                    blocked = true;
                                    break;
                                }
                            }
                            let condition_passes = action.condition.as_deref()
                                .map(&resolve_condition)
                                .unwrap_or_else(|| action.minimum_flywheel_velocity
                                    .is_none_or(|minimum| current_velocity >= minimum));
                            if !condition_passes {
                                blocked = true;
                                break;
                            }

                            let value = action.value.unwrap_or(0.0);
                            match action.role {
                                crate::ftc_interpreter::MotorRole::Intake if action.method == "setPower" => {
                                    self.intake_power = value.abs().clamp(0.0, 1.0);
                                    self.intake_running = self.intake_power > 0.0;
                                }
                                crate::ftc_interpreter::MotorRole::Feeder if action.method == "setPower" => {
                                    self.feeder_power = value.abs().clamp(0.0, 1.0);
                                    self.feeder_running = self.feeder_power > 0.0;
                                }
                                crate::ftc_interpreter::MotorRole::Outtake if action.method == "setVelocity" => {
                                    self.flywheel_target_velocity = self.flywheel_target_velocity.max(value.max(0.0));
                                }
                                _ => {}
                            }
                            self.autonomous_waiting_for_condition = false;
                            self.autonomous_action_index += 1;
                        }

                        if !blocked && self.autonomous_action_index >= segment.actions_after.len() {
                            self.autonomous_waiting_for_condition = false;
                            self.autonomous_post_actions_applied = true;
                            self.autonomous_wait_remaining = segment.wait_after_ms as f32 / 1000.0;
                        } else if blocked {
                            ctx.request_repaint();
                        }
                    }

                    if self.autonomous_post_actions_applied {
                        self.autonomous_wait_remaining = (self.autonomous_wait_remaining - dt).max(0.0);
                        if self.autonomous_wait_remaining <= 0.0 {
                            self.autonomous_segment_index += 1;
                            self.autonomous_post_actions_applied = false;
                            self.autonomous_action_index = 0;
                            self.autonomous_action_delay_started = false;
                            if let Some(next) = self.autonomous_segments.get(self.autonomous_segment_index) {
                                self.autonomous_waypoints = next.waypoints.clone();
                                self.autonomous_headings = next.headings.clone();
                                self.autonomous_waypoint_index = 0;
                                self.forward_velocity = 0.0;
                                self.autonomous_status = format!(
                                    "Starting autonomous path {} of {}.",
                                    self.autonomous_segment_index + 1,
                                    self.autonomous_segments.len()
                                );
                                ctx.request_repaint();
                            } else {
                                self.autonomous_running = false;
                                self.forward_velocity = 0.0;
                                self.strafe_velocity = 0.0;
                                self.autonomous_status = "Autonomous sequence complete. All paths and waits finished.".to_owned();
                            }
                        } else {
                            self.autonomous_status = format!(
                                "Waiting at the end of path {}: {:.1} seconds remaining.",
                                self.autonomous_segment_index + 1,
                                self.autonomous_wait_remaining
                            );
                            ctx.request_repaint();
                        }
                    }
                } else {
                    self.autonomous_running = false;
                    self.autonomous_status = "Autonomous path complete.".to_owned();
                }
            }
        } else if self.teleop_active {
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
            self.turn_velocity = approach(self.turn_velocity, target_turn * MAX_TURN_DEG_S, 540.0 * dt);

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

        // Draw the legal launch areas. Each 120-degree cone points from its
        // active scoring cell toward the center of the field and expands to
        // the field walls.
        for (side, cone_center, color) in [
            (HiveSide::Red, hive_pair_centers(HiveSide::Red, self.red_hive_tipped).0, Color32::from_rgb(235, 65, 75)),
            (HiveSide::Blue, hive_pair_centers(HiveSide::Blue, self.blue_hive_tipped).0, Color32::from_rgb(65, 145, 255)),
        ] {
            let world_points = scoring_cone_polygon(cone_center, side, match side { HiveSide::Red => self.red_hive_tipped, HiveSide::Blue => self.blue_hive_tipped });
            let screen_points = world_points.into_iter().map(|(x, y)| {
                Pos2::new(field.left() + x * scale, field.bottom() - y * scale)
            }).collect::<Vec<_>>();
            painter.add(egui::Shape::convex_polygon(
                screen_points,
                Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 48),
                Stroke::new(1.5_f32, Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 190)),
            ));
        }

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

        // Draw the remaining autonomous route as a thin path overlay.
        if self.autonomous_running {
            let mut route_points = Vec::with_capacity(self.autonomous_waypoints.len() + 1);
            route_points.push((self.robot_x, self.robot_y));
            route_points.extend(self.autonomous_waypoints.iter().skip(self.autonomous_waypoint_index).copied());
            for pair in route_points.windows(2) {
                let a = Pos2::new(field.left() + pair[0].0 * scale, field.bottom() - pair[0].1 * scale);
                let b = Pos2::new(field.left() + pair[1].0 * scale, field.bottom() - pair[1].1 * scale);
                painter.line_segment([a, b], Stroke::new(1.6_f32, Color32::from_rgb(90, 230, 150)));
            }
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

        for projectile in &self.pollen_projectiles {
            let arc_height = projectile.z.max(0.0);
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

        // Collision hitboxes: the robot uses a rotated 18x18 in square.
        let robot_hitbox = [
            transform(-9.0, -9.0), transform(9.0, -9.0),
            transform(9.0, 9.0), transform(-9.0, 9.0),
        ];
        painter.add(egui::Shape::closed_line(
            robot_hitbox.to_vec(),
            Stroke::new(2.0_f32, Color32::from_rgb(255, 75, 75)),
        ));

        // Intake hitbox core: 12.75 in wide and 3 in deep, entirely
        // outside the chassis. The collision check also allows for element radius.
        let intake_hitbox = [
            (-6.375, 9.0), (6.375, 9.0),
            (6.375, 12.0), (-6.375, 12.0),
        ].iter().map(|(x, y)| transform(*x, *y)).collect::<Vec<_>>();
        painter.add(egui::Shape::closed_line(
            intake_hitbox,
            Stroke::new(2.0_f32, Color32::from_rgb(65, 220, 235)),
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

fn point_in_scoring_cone(point: (f32, f32), center: (f32, f32), side: HiveSide, tipped: bool) -> bool {
    let (mut axis_x, mut axis_y) = match side {
        HiveSide::Red => (0.0_f32, -1.0_f32),
        HiveSide::Blue => (0.0_f32, 1.0_f32),
    };
    if tipped { axis_x = -axis_x; axis_y = -axis_y; }
    let dx = point.0 - center.0;
    let dy = point.1 - center.1;
    let forward = dx * axis_x + dy * axis_y;
    let lateral = -dx * axis_y + dy * axis_x;
    forward >= 0.0
        && lateral.abs() <= forward * 3.0_f32.sqrt()
        && point.0 >= 0.0 && point.0 <= FIELD_SIZE_IN
        && point.1 >= 0.0 && point.1 <= FIELD_SIZE_IN
}

fn scoring_cone_polygon(center: (f32, f32), side: HiveSide, tipped: bool) -> Vec<(f32, f32)> {
    let mut axis_angle = match side {
        HiveSide::Red => -std::f32::consts::FRAC_PI_2,
        HiveSide::Blue => std::f32::consts::FRAC_PI_2,
    };
    if tipped { axis_angle += std::f32::consts::PI; }
    let half_angle = std::f32::consts::PI / 3.0;
    let mut points = vec![center];

    // Sample the 120-degree fan. Each ray ends at the first field wall it hits,
    // so the filled polygon follows the field perimeter rather than stopping
    // at an artificial circular arc.
    for step in 0..=48 {
        let angle = axis_angle - half_angle
            + (2.0 * half_angle) * step as f32 / 48.0;
        let dx = angle.cos();
        let dy = angle.sin();
        let mut distance = f32::INFINITY;
        if dx > 0.0 { distance = distance.min((FIELD_SIZE_IN - center.0) / dx); }
        if dx < 0.0 { distance = distance.min((0.0 - center.0) / dx); }
        if dy > 0.0 { distance = distance.min((FIELD_SIZE_IN - center.1) / dy); }
        if dy < 0.0 { distance = distance.min((0.0 - center.1) / dy); }
        points.push((center.0 + dx * distance, center.1 + dy * distance));
    }
    points
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

fn collide_element_with_robot(
    element: &mut Pollen,
    robot_x: f32,
    robot_y: f32,
    heading: f32,
    robot_speed: f32,
) {
    let dx = element.x - robot_x;
    let dy = element.y - robot_y;
    let local_x = dx * heading.cos() + dy * heading.sin();
    let local_y = dx * heading.sin() - dy * heading.cos();
    let closest_x = local_x.clamp(-ROBOT_SIZE_IN * 0.5, ROBOT_SIZE_IN * 0.5);
    let closest_y = local_y.clamp(-ROBOT_SIZE_IN * 0.5, ROBOT_SIZE_IN * 0.5);
    let away_x = local_x - closest_x;
    let away_y = local_y - closest_y;
    let distance = away_x.hypot(away_y);
    if distance < element.radius {
        let (local_nx, local_ny) = if distance > 0.001 {
            (away_x / distance, away_y / distance)
        } else if local_x.abs() > local_y.abs() {
            (local_x.signum(), 0.0)
        } else {
            (0.0, 1.0)
        };
        let nx = local_nx * heading.cos() + local_ny * heading.sin();
        let ny = local_nx * heading.sin() - local_ny * heading.cos();
        let overlap = element.radius - distance;
        element.x += nx * (overlap + 0.02);
        element.y += ny * (overlap + 0.02);
        let push_speed = robot_speed.max(2.0);
        let into_robot = element.vx * nx + element.vy * ny;
        if into_robot < 0.0 {
            element.vx -= nx * into_robot * 1.25;
            element.vy -= ny * into_robot * 1.25;
        }
        element.vx += nx * push_speed * 0.85;
        element.vy += ny * push_speed * 0.85;
    }
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
    let mut pollen = Vec::with_capacity(30);

    // Each Flower has four overlapping pollen pieces. The progressively smaller
    // circles make the stack look like pollen being drawn into the intake.
    let flower_radii = [1.4_f32, 1.05, 0.75, 0.5];
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
        // Controller state owns these mechanisms only during TeleOp.
        // In autonomous, run_autonomous() and the command interpreter set
        // these values; overwriting them with an empty controller state here
        // would immediately cancel every autonomous setPower() action.
        if self.teleop_active {
            let active_intake_button = pressed_buttons.iter().find(|button| {
                actions.iter().any(|(mapped, action)| mapped == *button && action.contains("intake"))
            }).cloned();
            self.intake_running = active_intake_button.is_some();
            self.intake_power = active_intake_button.as_deref()
                .and_then(|button| self.detected_motor_power_for_button(
                    button,
                    crate::ftc_interpreter::MotorRole::Intake,
                ))
                .unwrap_or(if self.intake_running { 1.0 } else { 0.0 });
            self.feeder_running = pressed_buttons.iter().any(|button| {
                actions.iter().any(|(mapped, action)| mapped == button && action.contains("feeder"))
            });
            self.feeder_power = if self.feeder_running { 1.0 } else { 0.0 };
        }
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
        egui::SidePanel::left("controls").resizable(true).default_width(290.0).show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("sidebar_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
            ui.heading("OpMode");
            if let Some(name) = &self.project_name {
                ui.label(format!("Project: {name}"));
                let mut clicked = None;
                egui::ComboBox::from_id_salt("opmode_select")
                    .selected_text(self.selected_op_mode
                        .and_then(|i| self.op_modes.get(i))
                        .map(|mode| format!("{} · {}", mode.kind, mode.name))
                        .unwrap_or_else(|| "Select an OpMode".to_owned()))
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        for (i, mode) in self.op_modes.iter().enumerate() {
                            if ui.selectable_label(self.selected_op_mode == Some(i),
                                format!("{} · {}", mode.kind, mode.name)).clicked() {
                                clicked = Some(i);
                            }
                        }
                    });
                if let Some(i) = clicked {
                    self.selected_op_mode = Some(i);
                    self.show_source = true;
                    self.autonomous_running = false;
                    self.teleop_active = false;
                    self.is_running = false;
                }
            } else {
                ui.label("Import an FTC project to select an OpMode.");
            }

            let selected_kind = self.selected_op_mode
                .and_then(|i| self.op_modes.get(i))
                .map(|mode| mode.kind.clone());
            let run_label = match selected_kind.as_deref() {
                Some("Autonomous") if self.autonomous_running => "Stop Autonomous",
                Some("Autonomous") => "Run Autonomous",
                Some("TeleOp") if self.teleop_active => "Stop TeleOp",
                Some("TeleOp") => "Run TeleOp",
                _ => "Select an OpMode to run",
            };
            if ui.add_enabled(selected_kind.is_some(), egui::Button::new(run_label)).clicked() {
                match selected_kind.as_deref() {
                    Some("Autonomous") if self.autonomous_running => {
                        self.autonomous_running = false;
                        self.forward_velocity = 0.0;
                        self.strafe_velocity = 0.0;
                        self.turn_velocity = 0.0;
                        self.autonomous_status = "Autonomous stopped.".to_owned();
                    }
                    Some("Autonomous") => {
                        self.teleop_active = false;
                        self.is_running = false;
                        self.run_autonomous();
                    }
                    Some("TeleOp") if self.teleop_active => {
                        self.teleop_active = false;
                        self.stick_forward = 0.0;
                        self.stick_strafe = 0.0;
                        self.stick_turn = 0.0;
                        self.forward_velocity = 0.0;
                        self.strafe_velocity = 0.0;
                        self.turn_velocity = 0.0;
                    }
                    Some("TeleOp") => {
                        self.autonomous_running = false;
                        self.is_running = false;
                        self.reset();
                        self.teleop_active = true;
                        self.last_frame = Instant::now();
                    }
                    _ => {}
                }
            }
            if let Some("Autonomous") = selected_kind.as_deref() {
                ui.label(&self.autonomous_status);
                if self.autonomous_running {
                    ui.label(format!("Path point: {}/{}", self.autonomous_waypoint_index + 1, self.autonomous_waypoints.len()));
                }
            }

            ui.separator();
            ui.collapsing("Robot position", |ui| {
                ui.label("Position (inches)");
            ui.add(egui::Slider::new(&mut self.robot_x, 0.0..=FIELD_SIZE_IN).text("X"));
            ui.add(egui::Slider::new(&mut self.robot_y, 0.0..=FIELD_SIZE_IN).text("Y"));
            ui.add(egui::Slider::new(&mut self.heading_deg, 0.0..=360.0).text("Heading").suffix("°"));
            });
            ui.collapsing("Simulation settings", |ui| {
                ui.add(egui::Slider::new(&mut self.drive_speed, 0.0..=60.0).text("Test speed").suffix(" in/s"));
                if ui.button("Reset simulation").clicked() { self.reset(); }
            });
            ui.collapsing("TeleOp controller", |ui| {
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
            ui.label(format!("Input: F {:.2} · S {:.2} · R {:.2}",
                self.stick_forward, self.stick_strafe, self.stick_turn));
            ui.collapsing("Drive motor power / velocity", |ui| {
                for (name, i) in [("Front left", 0), ("Front right", 1), ("Back left", 2), ("Back right", 3)] {
                    ui.label(format!("{name}: power {:+.2} · velocity {:+.1} in/s",
                        self.drive_wheel_power[i], self.drive_wheel_velocity[i]));
                }
            });
            });
            ui.collapsing("Pollen and mechanisms", |ui| {
            ui.strong(format!("Carrying: {}/4", self.carried_elements.len()));
            let pc=self.carried_elements.iter().filter(|item|item.kind==ElementKind::Pollen).count();
            let nc=self.carried_elements.iter().filter(|item|item.kind==ElementKind::Nectar).count();
            ui.label(format!("Pollen: {pc} · Nectar: {nc}"));
            ui.label(format!("Shoot order: {}",self.carried_elements.iter().map(|item|match item.kind{ElementKind::Pollen=>"P",ElementKind::Nectar=>"N"}).collect::<Vec<_>>().join(" → ")));
            ui.label(format!("Flywheel: {:.0}/{:.0} ticks/s", self.flywheel_velocity, self.flywheel_target_velocity));
            ui.add(egui::Slider::new(&mut self.ramp_angle_deg, 0.0..=80.0).text("Ramp angle").suffix("°"));
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
            ui.label("Ramp angle controls the projectile's launch arc and horizontal travel. Flywheel speed is converted using a fixed simulation scale.");
            ui.horizontal(|ui| {
                if ui.button("Intake pollen [E]").clicked() {
                    self.intake_pollen();
                }
                if ui.button("Shoot pollen [Space]").clicked() {
                    self.shoot_pollen();
                }
            });
            ui.label("Move the front intake close to a yellow pollen ball, then intake.");
            });
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
            ui.collapsing("OpMode telemetry", |ui| {
                if let Some(index) = self.selected_op_mode {
                    if let Some(mode) = self.op_modes.get(index) {
                        if mode.telemetry.is_empty() {
                            ui.label("No telemetry.addData() or telemetry.addLine() calls detected in this OpMode.");
                        } else {
                            egui::ScrollArea::vertical().id_salt("telemetry_scroll").max_height(220.0).auto_shrink([false, false]).show(ui, |ui| {
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
            });


            ui.add_space(12.0);
            ui.collapsing("FTC project", |ui| {
                if ui.button("Import FTC project folder").clicked() { self.import_project(); }
                ui.label(&self.import_status);
                if self.project_name.is_some() {
                    ui.label(format!("{} OpModes detected", self.op_modes.len()));
                    if self.op_modes.is_empty() {
                        ui.label("No @TeleOp or @Autonomous classes were detected.");
                    }
                }
            });
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
