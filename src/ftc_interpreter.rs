//! Small interpreter for the FTC Java subset used by the visualizer.
//! This intentionally models FTC hardware mappings, OpMode button conditions,
//! numeric variables, and DcMotor/DcMotorEx commands. It does not execute Android code.

#[derive(Clone, Debug, PartialEq)]
pub struct Motor {
    pub variable: String,
    pub hardware_name: String,
    pub role: MotorRole,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MotorRole { Drive(usize), Intake, Feeder, Outtake, Other }

#[derive(Clone, Debug, PartialEq)]
pub struct MotorCommand {
    pub motor: String,
    pub method: String,
    pub expression: String,
    pub value: Option<f32>,
    pub button: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Program {
    pub motors: Vec<Motor>,
    pub commands: Vec<MotorCommand>,
    pub button_actions: Vec<(String, String)>,
    pub diagnostics: Vec<String>,
}

pub fn interpret(source: &str) -> Program {
    let clean = strip_comments(source);
    let mut program = Program::default();
    let tokens = tokenize(&clean);

    // Parse Java assignments and Kotlin-style hardwareMap.get declarations.
    for eq in 1..tokens.len().saturating_sub(3) {
        if !tokens[eq].punct("=") || !tokens[eq + 1].ident_is("hardwareMap") { continue; }
        let variable = tokens[..eq].iter().rev().find_map(|t| t.ident()).unwrap_or("").to_owned();
        if variable.is_empty() { continue; }
        let end = (eq + 2..tokens.len()).find(|&n| tokens[n].punct(";") || tokens[n].punct(")")).unwrap_or(tokens.len());
        let Some(name) = tokens[eq + 2..end].iter().find(|t| t.string).map(|t| t.text.clone()) else { continue; };
        if !program.motors.iter().any(|m| m.variable == variable) {
            let role = classify_motor(&variable, &name);
            program.motors.push(Motor { variable, hardware_name: name, role });
        }
    }

    // Find if(gamepad1.<button>) blocks; braces are balanced from tokens, so
    // nested if statements remain associated with the physical button condition.
    let mut button_blocks: Vec<(String, usize, usize)> = Vec::new();
    for n in 0..tokens.len() {
        if !tokens[n].ident_is("if") { continue; }
        let Some(open) = tokens.get(n + 1..).and_then(|tail| tail.iter().position(|t| t.punct("(")).map(|p| n + 1 + p)) else { continue; };
        let Some(close) = matching(&tokens, open, "(", ")") else { continue; };
        let condition = tokens[open + 1..close].iter().map(|t| t.text.as_str()).collect::<String>();
        if let Some(button) = canonical_button(&condition) {
            let Some(body_open) = tokens.get(close + 1..).and_then(|tail| tail.iter().position(|t| t.punct("{")).map(|p| close + 1 + p)) else { continue; };
            if let Some(body_close) = matching(&tokens, body_open, "{", "}") {
                button_blocks.push((button, body_open, body_close));
            }
        }
    }

    for motor in &program.motors {
        let mut n = 0;
        while n < tokens.len() {
            if tokens[n].ident_is(&motor.variable) {
                let search_end = (n + 7).min(tokens.len());
                let method_at = (n + 1..search_end).find(|&p| {
                    tokens[p].punct(".") && tokens.get(p + 1).and_then(Token::ident).is_some_and(|name|
                        matches!(name, "setPower" | "setVelocity" | "setTargetPosition" | "setMode" | "setDirection" | "setZeroPowerBehavior"))
                });
                if let Some(dot) = method_at {
                    let method = tokens[dot + 1].text.clone();
                    if tokens.get(dot + 2).is_some_and(|t| t.punct("(")) {
                        if let Some(end) = matching(&tokens, dot + 2, "(", ")") {
                            let expression = tokens[dot + 3..end].iter().map(|t| t.text.as_str()).collect::<String>();
                            let button = button_blocks.iter()
                                .filter(|(_, start, finish)| n > *start && n < *finish)
                                .min_by_key(|(_, start, finish)| finish - start)
                                .map(|(b, _, _)| b.clone());
                            let value = resolve_number(source, &expression);
                            program.commands.push(MotorCommand { motor: motor.variable.clone(), method, expression, value, button });
                            n = end;
                        }
                    }
                }
            }
            n += 1;
        }
    }

    for (button, _start, _end) in &button_blocks {
        let mut has_intake = false;
        let mut has_feeder = false;
        let mut has_outtake = false;
        for command in program.commands.iter().filter(|command| command.button.as_deref() == Some(button)) {
            if let Some(motor) = program.motors.iter().find(|motor| motor.variable == command.motor) {
                match &motor.role {
                    MotorRole::Intake => has_intake = true,
                    MotorRole::Feeder => has_feeder = true,
                    MotorRole::Outtake => has_outtake = true,
                    _ => {}
                }
            }
        }
        let action = if has_intake { "intake" }
            else if has_feeder { "feeder" }
            else if has_outtake { "outtake/shooter" }
            else { "other FTC action" };
        if let Some(existing) = program.button_actions.iter_mut().find(|(b, _)| b == button) {
            existing.1 = action.to_owned();
        } else {
            program.button_actions.push((button.clone(), action.to_owned()));
        }
    }

    if program.motors.is_empty() {
        program.diagnostics.push("No hardwareMap motor declarations were recognized.".to_owned());
    }
    program
}

pub fn resolve_number(source: &str, expression: &str) -> Option<f32> {
    resolve_expression(source, expression.trim(), 0)
}

fn resolve_expression(source: &str, expression: &str, depth: usize) -> Option<f32> {
    if depth > 12 { return None; }
    let mut parser = NumericParser { source, chars: expression.chars().collect(), position: 0, depth };
    let value = parser.parse_sum()?;
    parser.skip_space();
    if parser.position == parser.chars.len() { Some(value) } else { None }
}

struct NumericParser<'a> {
    source: &'a str,
    chars: Vec<char>,
    position: usize,
    depth: usize,
}

impl NumericParser<'_> {
    fn skip_space(&mut self) {
        while self.position < self.chars.len() && self.chars[self.position].is_whitespace() {
            self.position += 1;
        }
    }

    fn consume(&mut self, expected: char) -> bool {
        self.skip_space();
        if self.chars.get(self.position) == Some(&expected) {
            self.position += 1;
            true
        } else { false }
    }

    fn parse_sum(&mut self) -> Option<f32> {
        let mut value = self.parse_product()?;
        loop {
            if self.consume('+') { value += self.parse_product()?; }
            else if self.consume('-') { value -= self.parse_product()?; }
            else { return Some(value); }
        }
    }

    fn parse_product(&mut self) -> Option<f32> {
        let mut value = self.parse_unary()?;
        loop {
            if self.consume('*') { value *= self.parse_unary()?; }
            else if self.consume('/') {
                let divisor = self.parse_unary()?;
                if divisor == 0.0 { return None; }
                value /= divisor;
            } else { return Some(value); }
        }
    }

    fn parse_unary(&mut self) -> Option<f32> {
        if self.consume('-') { return Some(-self.parse_unary()?); }
        if self.consume('+') { return self.parse_unary(); }
        self.parse_atom()
    }

    fn parse_atom(&mut self) -> Option<f32> {
        self.skip_space();
        if self.consume('(') {
            let value = self.parse_sum()?;
            if !self.consume(')') { return None; }
            return Some(value);
        }
        self.skip_space();
        let start = self.position;
        while self.position < self.chars.len() {
            let c = self.chars[self.position];
            if c.is_ascii_alphanumeric() || c == '_' || c == '.' {
                self.position += 1;
            } else { break; }
        }
        if start == self.position { return None; }
        let token = self.chars[start..self.position].iter().collect::<String>();
        if let Ok(value) = token.parse::<f32>() { return Some(value); }
        resolve_variable(self.source, &token, self.depth + 1)
    }
}

fn resolve_variable(source: &str, variable: &str, depth: usize) -> Option<f32> {
    if depth > 12 { return None; }
    let tokens = tokenize(&strip_comments(source));
    let mut result = None;
    for i in 0..tokens.len().saturating_sub(2) {
        if tokens[i].ident_is(variable) && tokens[i + 1].punct("=") {
            let mut end = i + 2;
            while end < tokens.len() && !tokens[end].punct(";") && !tokens[end].punct("}") { end += 1; }
            let expression = tokens[i + 2..end].iter().map(|t| t.text.as_str()).collect::<String>();
            result = resolve_expression(source, &expression, depth + 1);
        }
    }
    result
}

fn classify_motor(variable: &str, hardware: &str) -> MotorRole {
    let s = format!("{} {}", variable, hardware).to_ascii_lowercase().replace('_', "");
    if s.contains("frontleft") || s.contains("leftfront") { MotorRole::Drive(0) }
    else if s.contains("frontright") || s.contains("rightfront") { MotorRole::Drive(1) }
    else if s.contains("backleft") || s.contains("leftback") || s.contains("rearleft") { MotorRole::Drive(2) }
    else if s.contains("backright") || s.contains("rightback") || s.contains("rearright") { MotorRole::Drive(3) }
    else if s.contains("intake") { MotorRole::Intake }
    else if s.contains("feeder") || s.contains("conveyor") { MotorRole::Feeder }
    else if s.contains("outtake") || s.contains("shooter") || s.contains("flywheel") { MotorRole::Outtake }
    else { MotorRole::Other }
}

fn canonical_button(condition: &str) -> Option<String> {
    let c = condition.to_ascii_lowercase();
    if !c.contains("gamepad1.") { return None; }
    for (button, aliases) in [
        ("south", ["a", "cross"]), ("east", ["b", "circle"]),
        ("west", ["x", "square"]), ("north", ["y", "triangle"]),
        ("left_bumper", ["left_bumper", "leftbumper"]),
        ("right_bumper", ["right_bumper", "rightbumper"]),
        ("left_trigger", ["left_trigger", "lefttrigger"]),
        ("right_trigger", ["right_trigger", "righttrigger"]),
        ("dpad_up", ["dpad_up", "dpadup"]), ("dpad_down", ["dpad_down", "dpaddown"]),
        ("dpad_left", ["dpad_left", "dpadleft"]), ("dpad_right", ["dpad_right", "dpadright"]),
        ("start", ["start", "options"]), ("back", ["back", "share"]),
    ] {
        if aliases.iter().any(|a| c.contains(&format!("gamepad1.{a}"))) { return Some(button.to_owned()); }
    }
    None
}

#[derive(Clone, Debug)]
struct Token { text: String, string: bool }
impl Token {
    fn punct(&self, p: &str) -> bool { !self.string && self.text == p }
    fn ident(&self) -> Option<&str> { if !self.string && self.text.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') { Some(&self.text) } else { None } }
    fn ident_is(&self, s: &str) -> bool { self.ident() == Some(s) }
}

fn tokenize(source: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut chars = source.char_indices().peekable();
    while let Some((_, c)) = chars.next() {
        if c.is_whitespace() { continue; }
        if c == '"' || c == '\'' {
            let quote = c;
            let mut value = String::new();
            while let Some((_, ch)) = chars.next() {
                if ch == '\\' { if let Some((_, escaped)) = chars.next() { value.push(escaped); } }
                else if ch == quote { break; }
                else { value.push(ch); }
            }
            out.push(Token { text: value, string: true });
        } else if c.is_ascii_alphanumeric() || c == '_' {
            let mut value = String::from(c);
            while let Some((_, next)) = chars.peek() {
                if next.is_ascii_alphanumeric() || *next == '_' { value.push(*next); chars.next(); } else { break; }
            }
            out.push(Token { text: value, string: false });
        } else {
            out.push(Token { text: c.to_string(), string: false });
        }
    }
    out
}

fn matching(tokens: &[Token], start: usize, open: &str, close: &str) -> Option<usize> {
    let mut depth = 0;
    for (i, token) in tokens.iter().enumerate().skip(start) {
        if token.punct(open) { depth += 1; }
        if token.punct(close) { depth -= 1; if depth == 0 { return Some(i); } }
    }
    None
}

fn strip_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut string = false;
    let mut character = false;
    while let Some(c) = chars.next() {
        if string || character {
            out.push(c);
            if c == '\\' { if let Some(n) = chars.next() { out.push(n); } }
            else if (string && c == '"') || (character && c == '\'') { string = false; character = false; }
        } else if c == '"' { string = true; out.push(c); }
        else if c == '\'' { character = true; out.push(c); }
        else if c == '/' && chars.peek() == Some(&'/') {
            while let Some(n) = chars.next() { if n == '\n' { out.push('\n'); break; } }
        } else if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut prev = ' ';
            while let Some(n) = chars.next() { if prev == '*' && n == '/' { break; } prev = n; }
        } else { out.push(c); }
    }
    out
}

/// A sampled field-space route extracted from common PedroPathing path builders.
/// This is a visualizer-side subset, not a general Java virtual machine.
#[derive(Clone, Debug, PartialEq)]
pub struct AutonomousMechanismAction {
    pub role: MotorRole,
    pub method: String,
    pub value: Option<f32>,
    /// Minimum flywheel speed required before this action can activate.
    pub minimum_flywheel_velocity: Option<f32>,
    /// Original Java if-condition guarding this motor command, when present.
    pub condition: Option<String>,
    /// Java waitUntil predicate that must become true before this action runs.
    pub wait_until: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AutonomousPathSegment {
    pub waypoints: Vec<(f32, f32)>,
    pub headings: Vec<Option<f32>>,
    pub wait_after_ms: u64,
    pub actions_after: Vec<AutonomousMechanismAction>,
}

#[derive(Default)]
pub struct AutonomousPlan {
    pub start_pose: Option<(f32, f32, f32)>,
    pub waypoints: Vec<(f32, f32)>,
    /// Optional PedroPathing heading in radians for each sampled waypoint.
    pub waypoint_headings: Vec<Option<f32>>,
    /// True when a supported PedroPathing path expression was parsed, even if it has zero length.
    pub path_found: bool,
    pub startup_actions: Vec<AutonomousMechanismAction>,
    pub post_path_actions: Vec<AutonomousMechanismAction>,
    /// Duration of waitMs(...) commands after the path and mechanism actions.
    pub post_path_wait_ms: u64,
    /// Ordered path segments, each with its own following actions and delay.
    pub path_segments: Vec<AutonomousPathSegment>,
    pub follow_path_called: bool,
    pub diagnostics: Vec<String>,
}

/// Extracts common PedroPathing Pose, BezierLine, and BezierCurve declarations.
pub fn interpret_autonomous(source: &str) -> AutonomousPlan {
    let clean = strip_comments(source);
    let tokens = tokenize(&clean);
    let mut plan = AutonomousPlan::default();
    plan.follow_path_called = tokens.windows(2).any(|pair|
        pair[0].ident_is("followPath") && pair[1].punct("(")
    );

    // PoseFactory.degrees().of(x, y, heading) is common in newer
    // PedroPathing projects. Store named poses so Paths.line(...).linear(...)
    // can resolve the identifiers used by the autonomous.
    let uses_degree_factory = tokens.windows(3).any(|w|
        w[0].ident_is("degrees") && w[1].punct("(") && w[2].punct(")")
    );
    let mut named_poses: Vec<(String, (f32, f32, f32))> = Vec::new();

    for i in 0..tokens.len().saturating_sub(2) {
        if tokens[i].ident_is("Pose") && tokens.get(i + 1).is_some_and(|t| t.punct("(")) {
            let Some(end) = matching(&tokens, i + 1, "(", ")") else { continue; };
            let args = split_arguments(&tokens[i + 2..end]);
            if args.len() >= 2 {
                if let (Some(x), Some(y)) = (
                    resolve_number(source, &tokens_to_text(args[0])),
                    resolve_number(source, &tokens_to_text(args[1])),
                ) {
                    let heading = args.get(2).and_then(|arg| resolve_angle(source, arg)).unwrap_or(0.0);
                    plan.start_pose = Some((x, y, heading));
                    break;
                }
            }
        }

        // Find declarations of the form: Pose name = pf.of(x, y, heading)
        if !tokens[i].ident_is("Pose") { continue; }
        let Some(name) = tokens.get(i + 1).and_then(Token::ident) else { continue; };
        if !tokens.get(i + 2).is_some_and(|t| t.punct("=")) { continue; }
        let Some(of_at) = (i + 3..(i + 12).min(tokens.len())).find(|&p| tokens[p].ident_is("of")) else { continue; };
        if !tokens.get(of_at + 1).is_some_and(|t| t.punct("(")) { continue; }
        let Some(end) = matching(&tokens, of_at + 1, "(", ")") else { continue; };
        let args = split_arguments(&tokens[of_at + 2..end]);
        if args.len() < 3 { continue; }
        let (Some(x), Some(y), Some(mut heading)) = (
            resolve_number(source, &tokens_to_text(args[0])),
            resolve_number(source, &tokens_to_text(args[1])),
            resolve_number(source, &tokens_to_text(args[2])),
        ) else { continue; };
        if uses_degree_factory { heading = heading.to_radians(); }
        let pose = (x, y, heading);
        named_poses.push((name.to_owned(), pose));
        if plan.start_pose.is_none() || name.to_ascii_lowercase().contains("start") {
            plan.start_pose = Some(pose);
        }
    }

    // Older PedroPathing constructor-style paths remain supported.
    for i in 0..tokens.len().saturating_sub(2) {
        let is_line = tokens[i].ident_is("BezierLine");
        let is_curve = tokens[i].ident_is("BezierCurve");
        if !(is_line || is_curve) || !tokens.get(i + 1).is_some_and(|t| t.punct("(")) { continue; }
        let Some(end) = matching(&tokens, i + 1, "(", ")") else { continue; };
        let body = &tokens[i + 2..end];
        let mut points = Vec::new();
        let mut p = 0;
        while p + 2 < body.len() {
            if body[p].ident_is("Point") && body[p + 1].punct("(") {
                if let Some(point_end) = matching(body, p + 1, "(", ")") {
                    let args = split_arguments(&body[p + 2..point_end]);
                    if args.len() >= 2 {
                        let x = resolve_number(source, &tokens_to_text(args[0]));
                        let y = resolve_number(source, &tokens_to_text(args[1]));
                        if let (Some(x), Some(y)) = (x, y) { points.push((x, y)); }
                    }
                    p = point_end;
                }
            }
            p += 1;
        }
        if points.len() >= 2 { plan.path_found = true; }
        append_sampled_path(&mut plan.waypoints, &mut plan.waypoint_headings, &points, if is_line { 12 } else { 24 });
    }

    // Newer API: Paths.line(startPose, endPose).linear(startPose, endPose).
    // Resolve pose identifiers from their PoseFactory declarations.
    for i in 0..tokens.len().saturating_sub(2) {
        if !tokens[i].ident_is("linear") || !tokens.get(i + 1).is_some_and(|t| t.punct("(")) { continue; }
        let Some(end) = matching(&tokens, i + 1, "(", ")") else { continue; };
        let args = split_arguments(&tokens[i + 2..end]);
        let mut poses = Vec::new();
        for arg in args {
            let name = tokens_to_text(arg);
            if let Some((_, pose)) = named_poses.iter().find(|(n, _)| n == &name) {
                poses.push(*pose);
            }
        }
        if poses.len() >= 2 {
            plan.path_found = true;
            append_sampled_pose_path(&mut plan.waypoints, &mut plan.waypoint_headings, &poses, 24);
        }
    }

    // Resolve path helper methods independently so each follow(...) in the
    // sequential command retains its own geometry and position in the routine.
    let mut named_paths: Vec<(String, Vec<(f32, f32)>, Vec<Option<f32>>)> = Vec::new();
    let java_program = crate::java_interpreter::parse_java_program(source);
    for method in java_program.methods.iter().filter(|method| method.return_type == "Path") {
        let method_tokens = tokenize(&strip_comments(&method.body_source));
        let Some(linear_at) = (0..method_tokens.len().saturating_sub(1)).find(|&n|
            method_tokens[n].ident_is("linear") && method_tokens[n + 1].punct("(")
        ) else { continue; };
        let Some(linear_end) = matching(&method_tokens, linear_at + 1, "(", ")") else { continue; };
        let args = split_arguments(&method_tokens[linear_at + 2..linear_end]);
        let mut poses = Vec::new();
        for arg in args {
            let pose_name = tokens_to_text(arg);
            if let Some((_, pose)) = named_poses.iter().find(|(name, _)| name == &pose_name) {
                poses.push(*pose);
            }
        }
        if poses.len() >= 2 {
            let mut points = Vec::new();
            let mut headings = Vec::new();
            append_sampled_pose_path(&mut points, &mut headings, &poses, 24);
            named_paths.push((method.name.clone(), points, headings));
        }
    }

    // Interpret the sequential command in source order. Waits and mechanism
    // commands are attached to the path that immediately precedes them.
    let method_actions = extract_instant_method_actions(source, &tokens);
    let command_tree = crate::java_interpreter::parse_autonomous_command_tree(source);
    let mut path_segments: Vec<AutonomousPathSegment> = Vec::new();
    let mut startup_actions = Vec::new();
    let mut path_seen = false;

    fn apply_command_node(
        node: &crate::java_interpreter::CommandNode,
        source: &str,
        named_paths: &[(String, Vec<(f32, f32)>, Vec<Option<f32>>)],
        method_actions: &[(String, Vec<AutonomousMechanismAction>)],
        path_segments: &mut Vec<AutonomousPathSegment>,
        startup_actions: &mut Vec<AutonomousMechanismAction>,
        path_seen: &mut bool,
        parallel_group: bool,
    ) {
        use crate::java_interpreter::CommandNode;
        match node {
            CommandNode::Group { kind, children } => {
                let is_parallel = kind == "parallel";
                if is_parallel {
                    // Parallel children start together. A path child is not
                    // expected alongside a wait in the common FTC command API;
                    // mechanism actions are invoked at group start and the
                    // group duration is the longest child wait.
                    let mut longest_wait = 0_u64;
                    for child in children {
                        if let CommandNode::Call { name, args } = child {
                            if name == "waitMs" {
                                if let Some(ms) = args.first().and_then(|v| resolve_number(source, v)) {
                                    longest_wait = longest_wait.max(ms.max(0.0) as u64);
                                }
                            } else if name == "follow" {
                                let path_name = args.get(1).and_then(|arg| {
                                    let trimmed = arg.trim();
                                    let open = trimmed.find('(')?;
                                    trimmed[..open].trim().rsplit('.').next()
                                });
                                if let Some((_, points, headings)) = path_name.and_then(|n|
                                    named_paths.iter().find(|(path, _, _)| path == n)
                                ) {
                                    *path_seen = true;
                                    path_segments.push(AutonomousPathSegment {
                                        waypoints: points.clone(),
                                        headings: headings.clone(),
                                        wait_after_ms: longest_wait,
                                        actions_after: Vec::new(),
                                    });
                                }
                            } else {
                                attach_command_action(name, method_actions, path_segments, startup_actions, *path_seen);
                            }
                        } else {
                            apply_command_node(child, source, named_paths, method_actions,
                                path_segments, startup_actions, path_seen, true);
                        }
                    }
                    if let Some(segment) = path_segments.last_mut().filter(|_| *path_seen) {
                        segment.wait_after_ms = segment.wait_after_ms.max(longest_wait);
                    }
                } else {
                    for child in children {
                        apply_command_node(child, source, named_paths, method_actions,
                            path_segments, startup_actions, path_seen, parallel_group);
                    }
                }
            }
            CommandNode::Call { name, args } if name == "follow" => {
                *path_seen = true;
                let path_name = args.get(1).and_then(|arg| {
                    let trimmed = arg.trim();
                    let open = trimmed.find('(')?;
                    trimmed[..open].trim().rsplit('.').next()
                });
                if let Some((_, points, headings)) = path_name.and_then(|n|
                    named_paths.iter().find(|(path, _, _)| path == n)
                ) {
                    path_segments.push(AutonomousPathSegment {
                        waypoints: points.clone(),
                        headings: headings.clone(),
                        wait_after_ms: 0,
                        actions_after: Vec::new(),
                    });
                }
            }
            CommandNode::Call { name, args } if name == "waitMs" => {
                if let Some(ms) = args.first().and_then(|v| resolve_number(source, v)) {
                    if let Some(segment) = path_segments.last_mut() {
                        segment.wait_after_ms = segment.wait_after_ms.saturating_add(ms.max(0.0) as u64);
                    }
                }
            }
            CommandNode::Call { name, .. } => {
                attach_command_action(name, method_actions, path_segments, startup_actions, *path_seen);
            }
            CommandNode::Raw(_) => {}
        }
    }

    fn attach_command_action(
        name: &str,
        method_actions: &[(String, Vec<AutonomousMechanismAction>)],
        path_segments: &mut Vec<AutonomousPathSegment>,
        startup_actions: &mut Vec<AutonomousMechanismAction>,
        path_seen: bool,
    ) {
        if let Some(actions) = method_actions.iter().find(|(method, _)| method == name).map(|(_, actions)| actions) {
            if let Some(segment) = path_segments.last_mut().filter(|_| path_seen) {
                segment.actions_after.extend(actions.clone());
            } else {
                startup_actions.extend(actions.clone());
            }
        }
    }

    if let Some(tree) = command_tree {
        apply_command_node(&tree, source, &named_paths, &method_actions,
            &mut path_segments, &mut startup_actions, &mut path_seen, false);
    }
    if !path_segments.is_empty() {
        plan.path_found = true;
        plan.waypoints.clear();
        plan.waypoint_headings.clear();
        for segment in &path_segments {
            plan.waypoints.extend(segment.waypoints.iter().copied());
            plan.waypoint_headings.extend(segment.headings.iter().copied());
        }
        plan.path_segments = path_segments;
    }
    plan.startup_actions = startup_actions;
    plan.follow_path_called |= !plan.path_segments.is_empty();
    if let Some(last) = plan.path_segments.last() {
        plan.post_path_actions = last.actions_after.clone();
        plan.post_path_wait_ms = last.wait_after_ms;
    }

    // Static-imported PedroCommands.follow(follower, path()) is the newer
    // Ivy command form; schedule(...) starts it in OpMode.start().
    plan.follow_path_called |= tokens.iter().enumerate().any(|(i, token)|
        token.ident_is("follow") && tokens.get(i + 1).is_some_and(|t| t.punct("("))
    );

    if let Some((x, y, _)) = plan.start_pose {
        if plan.waypoints.first().is_some_and(|p| (p.0-x).hypot(p.1-y) < 0.5) {
            plan.waypoints.remove(0);
            if !plan.waypoint_headings.is_empty() { plan.waypoint_headings.remove(0); }
        }
    }
    if !plan.path_found {
        plan.diagnostics.push("No supported PedroPathing paths were found. Expected Bezier constructors or Paths.line(...).linear(...).".to_owned());
    } else if plan.waypoints.is_empty() {
        plan.diagnostics.push("A PedroPathing path was recognized, but it has zero length because its poses share the same coordinates.".to_owned());
    }
    if !plan.follow_path_called {
        plan.diagnostics.push("No PedroPathing follow(...) or follower.followPath(...) command was detected; route extraction is preview-only.".to_owned());
    }
    plan
}

fn extract_instant_method_actions(source: &str, _tokens: &[Token]) -> Vec<(String, Vec<AutonomousMechanismAction>)> {
    let parsed = crate::java_interpreter::parse_java_program(source);
    let mut result = Vec::new();
    for method in parsed.methods.iter().filter(|method| method.return_type == "Command") {
        let body = tokenize(&strip_comments(&method.body_source));
        let mut actions = Vec::new();
        for p in 0..body.len().saturating_sub(4) {
            if !body[p].ident_is("instant") || !body[p + 1].punct("(") { continue; }
            let Some(instant_end) = matching(&body, p + 1, "(", ")") else { continue; };
            let Some(lambda_open) = (p + 2..instant_end).find(|&n| body[n].punct("{")) else { continue; };
            let Some(lambda_end) = matching(&body, lambda_open, "{", "}") else { continue; };
            for command_at in lambda_open + 1..lambda_end {
                let Some(variable) = body[command_at].ident() else { continue; };
                let mut member_at = command_at + 1;
                while member_at < lambda_end && member_at <= command_at + 3 && body[member_at].punct(")") { member_at += 1; }
                if !body.get(member_at).is_some_and(|t| t.punct(".")) { continue; }
                let Some(command) = body.get(member_at + 1).and_then(Token::ident) else { continue; };
                if command != "setPower" && command != "setVelocity" { continue; }
                if !body.get(member_at + 2).is_some_and(|t| t.punct("(")) { continue; }
                let Some(end) = matching(&body, member_at + 2, "(", ")") else { continue; };
                let expression = tokens_to_text(&body[member_at + 3..end]);
                let role = classify_motor(variable, variable);
                if matches!(role, MotorRole::Other | MotorRole::Drive(_)) { continue; }
                let minimum_flywheel_velocity = minimum_flywheel_velocity_for_command(&body, lambda_open, lambda_end, command_at, source);
                let condition = condition_for_command(&body, lambda_open, lambda_end, command_at);
                let wait_until = wait_until_condition(&body);
                actions.push(AutonomousMechanismAction { role, method: command.to_owned(), value: resolve_number(source, &expression), minimum_flywheel_velocity, condition, wait_until });
            }
        }
        if !actions.is_empty() { result.push((method.name.clone(), actions)); }
    }
    result
}

fn wait_until_condition(tokens: &[Token]) -> Option<String> {
    for i in 0..tokens.len().saturating_sub(2) {
        if !tokens[i].ident_is("waitUntil") || !tokens.get(i + 1).is_some_and(|t| t.punct("(")) {
            continue;
        }
        let end = matching(tokens, i + 1, "(", ")")?;
        let args = split_arguments(&tokens[i + 2..end]);
        let first = args.first()?;
        let arrow = first.iter().position(|token| token.punct("->"))?;
        let predicate = tokens_to_text(&first[arrow + 1..]);
        if !predicate.trim().is_empty() { return Some(predicate); }
    }
    None
}

fn condition_for_command(
    tokens: &[Token],
    lambda_open: usize,
    lambda_end: usize,
    command_at: usize,
) -> Option<String> {
    for i in lambda_open + 1..command_at {
        if !tokens[i].ident_is("if") || !tokens.get(i + 1).is_some_and(|t| t.punct("(")) { continue; }
        let condition_end = matching(tokens, i + 1, "(", ")")?;
        let body_open = (condition_end + 1..lambda_end).find(|&n| tokens[n].punct("{"))?;
        let body_end = matching(tokens, body_open, "{", "}")?;
        if command_at > body_open && command_at < body_end {
            return Some(tokens_to_text(&tokens[i + 2..condition_end]));
        }
    }
    None
}

fn minimum_flywheel_velocity_for_command(
    tokens: &[Token],
    lambda_open: usize,
    lambda_end: usize,
    command_at: usize,
    source: &str,
) -> Option<f32> {
    let mut required = None;
    for i in lambda_open + 1..command_at {
        if !tokens[i].ident_is("if") || !tokens.get(i + 1).is_some_and(|t| t.punct("(")) {
            continue;
        }
        let Some(condition_end) = matching(tokens, i + 1, "(", ")") else { continue; };
        let Some(body_open) = (condition_end + 1..lambda_end).find(|&n| tokens[n].punct("{")) else { continue; };
        let Some(body_end) = matching(tokens, body_open, "{", "}") else { continue; };
        if command_at <= body_open || command_at >= body_end {
            continue;
        }
        let condition = &tokens[i + 2..condition_end];
        if !condition.iter().any(|token| token.ident_is("getVelocity")) {
            continue;
        }
        for pair in condition.windows(2) {
            if pair[0].punct(">") {
                if let Some(value) = resolve_number(source, &pair[1].text) {
                    required = Some(required.map_or(value, |old: f32| old.max(value)));
                }
            }
        }
    }
    required
}

fn append_sampled_path(waypoints: &mut Vec<(f32, f32)>, headings: &mut Vec<Option<f32>>, points: &[(f32, f32)], samples: usize) {
    if points.len() < 2 { return; }
    for step in 0..=samples {
        let t = step as f32 / samples as f32;
        let point = if points.len() == 2 {
            (points[0].0 * (1.0 - t) + points[1].0 * t,
             points[0].1 * (1.0 - t) + points[1].1 * t)
        } else if points.len() == 3 {
            let u = 1.0 - t;
            (u*u*points[0].0 + 2.0*u*t*points[1].0 + t*t*points[2].0,
             u*u*points[0].1 + 2.0*u*t*points[1].1 + t*t*points[2].1)
        } else {
            let u = 1.0 - t;
            (u*u*u*points[0].0 + 3.0*u*u*t*points[1].0 + 3.0*u*t*t*points[2].0 + t*t*t*points[3].0,
             u*u*u*points[0].1 + 3.0*u*u*t*points[1].1 + 3.0*u*t*t*points[2].1 + t*t*t*points[3].1)
        };
        if waypoints.last().is_none_or(|last| (last.0-point.0).hypot(last.1-point.1) > 0.05) {
            waypoints.push(point);
            headings.push(None);
        }
    }
}

fn append_sampled_pose_path(
    waypoints: &mut Vec<(f32, f32)>,
    headings: &mut Vec<Option<f32>>,
    poses: &[(f32, f32, f32)],
    samples: usize,
) {
    if poses.len() < 2 { return; }
    for segment in poses.windows(2) {
        let start = segment[0];
        let end = segment[1];
        let mut delta = (end.2 - start.2 + std::f32::consts::PI)
            .rem_euclid(2.0 * std::f32::consts::PI) - std::f32::consts::PI;
        if delta <= -std::f32::consts::PI { delta = std::f32::consts::PI; }
        for step in 0..=samples {
            let t = step as f32 / samples as f32;
            let point = (start.0 + (end.0 - start.0) * t, start.1 + (end.1 - start.1) * t);
            if waypoints.last().is_none_or(|last| (last.0-point.0).hypot(last.1-point.1) > 0.05) {
                waypoints.push(point);
                headings.push(Some(start.2 + delta * t));
            }
        }
    }
}

fn split_arguments(tokens: &[Token]) -> Vec<&[Token]> {
    let mut args = Vec::new();
    let mut start = 0;
    let mut depth = 0_i32;
    for (i, token) in tokens.iter().enumerate() {
        if token.punct("(") || token.punct("[") { depth += 1; }
        else if token.punct(")") || token.punct("]") { depth -= 1; }
        else if token.punct(",") && depth == 0 {
            args.push(&tokens[start..i]);
            start = i + 1;
        }
    }
    if start < tokens.len() { args.push(&tokens[start..]); }
    args
}

fn tokens_to_text(tokens: &[Token]) -> String {
    tokens.iter().map(|t| t.text.as_str()).collect::<String>()
}

fn resolve_angle(source: &str, tokens: &[Token]) -> Option<f32> {
    let expression = tokens_to_text(tokens);
    if expression.contains("toRadians") {
        let start = expression.find("toRadians")? + "toRadians".len();
        let inner = expression[start..].trim_start_matches('(').trim_end_matches(')');
        return resolve_number(source, inner).map(f32::to_radians);
    }
    if expression.contains("PI") {
        if expression.contains("PI/2") || expression.contains("PI / 2") { return Some(std::f32::consts::FRAC_PI_2); }
        if expression.contains("PI") { return Some(std::f32::consts::PI); }
    }
    resolve_number(source, &expression)
}
