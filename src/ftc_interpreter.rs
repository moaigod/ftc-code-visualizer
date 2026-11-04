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

    for (button, start, end) in &button_blocks {
        let body = &tokens[*start..=*end];
        let roles = body.iter().filter_map(|t| t.ident()).collect::<Vec<_>>();
        let action = if roles.iter().any(|x| x.to_ascii_lowercase().contains("intake")) {
            "intake"
        } else if roles.iter().any(|x| x.to_ascii_lowercase().contains("outtake") || x.to_ascii_lowercase().contains("shooter")) {
            "outtake/shooter"
        } else if roles.iter().any(|x| x.to_ascii_lowercase().contains("feeder")) {
            "feeder"
        } else { "other FTC action" };
        if !program.button_actions.iter().any(|(b, _)| b == button) {
            program.button_actions.push((button.clone(), action.to_owned()));
        }
    }

    if program.motors.is_empty() {
        program.diagnostics.push("No hardwareMap motor declarations were recognized.".to_owned());
    }
    program
}

pub fn resolve_number(source: &str, expression: &str) -> Option<f32> {
    let expr = expression.trim();
    if let Ok(v) = expr.parse::<f32>() { return Some(v); }
    let clean = strip_comments(source);
    let tokens = tokenize(&clean);
    for i in 0..tokens.len().saturating_sub(2) {
        if tokens[i].ident_is(expr) && tokens[i + 1].punct("=") {
            let mut end = i + 2;
            while end < tokens.len() && !tokens[end].punct(";") { end += 1; }
            let value = tokens[i + 2..end].iter().map(|t| t.text.as_str()).collect::<String>();
            if let Ok(v) = value.parse::<f32>() { return Some(v); }
        }
    }
    None
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
