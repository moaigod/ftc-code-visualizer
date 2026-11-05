//! Small structural interpreter for the command-expression subset used by FTC OpModes.
//! It parses nested command calls into an AST instead of relying on top-level string matches.

#[derive(Clone, Debug, PartialEq)]
pub enum CommandNode {
    Group { kind: String, children: Vec<CommandNode> },
    Call { name: String, args: Vec<String> },
    Raw(String),
}

/// Parse the first sequential(...) command expression, preserving nested
/// sequential/parallel groups and the exact source order of their children.
pub fn parse_autonomous_command_tree(source: &str) -> Option<CommandNode> {
    let clean = strip_comments(source);
    let bytes = clean.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if is_ident_start(bytes[i]) {
            let start = i;
            i += 1;
            while i < bytes.len() && is_ident_continue(bytes[i]) { i += 1; }
            let name = &clean[start..i];
            let mut open = i;
            while open < bytes.len() && bytes[open].is_ascii_whitespace() { open += 1; }
            if name == "sequential" && open < bytes.len() && bytes[open] == b'(' {
                let (node, _) = parse_call(&clean, start)?;
                return Some(node);
            }
        } else {
            i += 1;
        }
    }
    None
}

fn parse_call(source: &str, start: usize) -> Option<(CommandNode, usize)> {
    let bytes = source.as_bytes();
    let mut name_end = start;
    while name_end < bytes.len() && is_ident_continue(bytes[name_end]) { name_end += 1; }
    let name = source.get(start..name_end)?.to_owned();
    let mut open = name_end;
    while open < bytes.len() && bytes[open].is_ascii_whitespace() { open += 1; }
    if bytes.get(open) != Some(&b'(') { return None; }
    let close = matching_delimiter(source, open, b'(', b')')?;
    let raw_args = split_top_level(&source[open + 1..close]);
    let args = raw_args.into_iter().map(str::trim).filter(|s| !s.is_empty()).collect::<Vec<_>>();
    if name == "sequential" || name == "parallel" {
        let children = args.iter().map(|arg| parse_expression(arg)).collect();
        Some((CommandNode::Group { kind: name, children }, close + 1))
    } else {
        Some((CommandNode::Call { name, args }, close + 1))
    }
}

fn parse_expression(expression: &str) -> CommandNode {
    let trimmed = expression.trim();
    let Some((node, end)) = parse_call(trimmed, 0) else {
        return CommandNode::Raw(trimmed.to_owned());
    };
    if end == trimmed.len() { node } else { CommandNode::Raw(trimmed.to_owned()) }
}

fn split_top_level(source: &str) -> Vec<&str> {
    let bytes = source.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0;
    let mut paren = 0_i32;
    let mut brace = 0_i32;
    let mut bracket = 0_i32;
    let mut quote = 0_u8;
    let mut escaped = false;
    for (i, &b) in bytes.iter().enumerate() {
        if quote != 0 {
            if escaped { escaped = false; }
            else if b == b'\\' { escaped = true; }
            else if b == quote { quote = 0; }
            continue;
        }
        if b == b'"' || b == b'\\'' { quote = b; continue; }
        match b {
            b'(' => paren += 1, b')' => paren -= 1,
            b'{' => brace += 1, b'}' => brace -= 1,
            b'[' => bracket += 1, b']' => bracket -= 1,
            b',' if paren == 0 && brace == 0 && bracket == 0 => {
                parts.push(&source[start..i]); start = i + 1;
            }
            _ => {}
        }
    }
    if start < source.len() { parts.push(&source[start..]); }
    parts
}

fn matching_delimiter(source: &str, start: usize, open: u8, close: u8) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut depth = 0_i32;
    let mut quote = 0_u8;
    let mut escaped = false;
    for i in start..bytes.len() {
        let b = bytes[i];
        if quote != 0 {
            if escaped { escaped = false; }
            else if b == b'\\' { escaped = true; }
            else if b == quote { quote = 0; }
            continue;
        }
        if b == b'"' || b == b'\\'' { quote = b; continue; }
        if b == open { depth += 1; }
        if b == close {
            depth -= 1;
            if depth == 0 { return Some(i); }
        }
    }
    None
}

fn strip_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let bytes = source.as_bytes();
    let mut i = 0;
    let mut quote = 0_u8;
    let mut escaped = false;
    while i < bytes.len() {
        let b = bytes[i];
        if quote != 0 {
            out.push(b as char);
            if escaped { escaped = false; }
            else if b == b'\\' { escaped = true; }
            else if b == quote { quote = 0; }
            i += 1;
        } else if b == b'"' || b == b'\\'' {
            quote = b; out.push(b as char); i += 1;
        } else if b == b'/' && bytes.get(i + 1) == Some(&b'/') {
            while i < bytes.len() && bytes[i] != b'\\n' { i += 1; }
        } else if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') { i += 1; }
            i = (i + 2).min(bytes.len());
        } else {
            out.push(b as char); i += 1;
        }
    }
    out
}

fn is_ident_start(b: u8) -> bool { b.is_ascii_alphabetic() || b == b'_' }
fn is_ident_continue(b: u8) -> bool { b.is_ascii_alphanumeric() || b == b'_' }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_parallel_is_preserved() {
        let tree = parse_autonomous_command_tree(
            "Command autoRoutine() { return sequential(startup(), follow(follower, score()), parallel(shoot(), waitMs(3000)), follow(follower, pickup())); }"
        ).unwrap();
        let CommandNode::Group { kind, children } = tree else { panic!("expected group"); };
        assert_eq!(kind, "sequential");
        assert_eq!(children.len(), 4);
        assert!(matches!(&children[2], CommandNode::Group { kind, children }
            if kind == "parallel" && children.len() == 2));
    }
}


#[derive(Clone, Debug, PartialEq)]
pub struct JavaProgram {
    pub methods: Vec<JavaMethod>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct JavaMethod {
    pub return_type: String,
    pub name: String,
    pub parameters: Vec<String>,
    pub body: JavaStatement,
    /// Original source inside the method braces. Kept for expression evaluation
    /// and for the FTC-specific hardware interpreter.
    pub body_source: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum JavaStatement {
    Block(Vec<JavaStatement>),
    If {
        condition: String,
        then_branch: Box<JavaStatement>,
        else_branch: Option<Box<JavaStatement>>,
    },
    Return(Option<String>),
    Expression(String),
    Empty,
    Unsupported(String),
}

#[derive(Clone, Debug)]
struct JavaToken {
    text: String,
    start: usize,
    end: usize,
    kind: JavaTokenKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JavaTokenKind { Identifier, Number, String, Symbol }

/// Parse method declarations and their statement structure from Java source.
/// This is deliberately a subset parser: expressions remain source strings,
/// while blocks, if/else, return, and expression statements are structural.
pub fn parse_java_program(source: &str) -> JavaProgram {
    let clean = strip_comments(source);
    let tokens = lex_java(&clean);
    let mut methods = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        if tokens[i].kind != JavaTokenKind::Identifier
            || matches!(tokens[i].text.as_str(), "if" | "for" | "while" | "switch" | "catch" | "synchronized" | "new")
            || tokens.get(i + 1).is_none_or(|t| t.text != "(")
        {
            i += 1;
            continue;
        }
        let Some(params_end) = match_java(&tokens, i + 1, "(", ")") else { i += 1; continue; };
        let mut body_open = params_end + 1;
        while body_open < tokens.len()
            && matches!(tokens[body_open].text.as_str(), "throws" | ",")
        {
            body_open += 1;
        }
        // Skip a throws clause and its declared exception names.
        if tokens.get(body_open).is_some_and(|t| t.text == "throws") {
            body_open += 1;
            while body_open < tokens.len() && tokens[body_open].text != "{" && tokens[body_open].text != ";" {
                body_open += 1;
            }
        }
        if tokens.get(body_open).is_none_or(|t| t.text != "{") {
            i += 1;
            continue;
        }
        let Some(body_end) = match_java(&tokens, body_open, "{", "}") else { i += 1; continue; };
        let Some(return_token) = i.checked_sub(1).and_then(|p| tokens.get(p)) else { i += 1; continue; };
        if return_token.kind != JavaTokenKind::Identifier && return_token.text != ">" && return_token.text != "]" {
            i += 1;
            continue;
        }
        // Calls are preceded by '.' or 'return'; a declaration has a type before
        // the method name and is not a member invocation.
        if i >= 2 && matches!(tokens[i - 2].text.as_str(), "." | "new" | "return") {
            i += 1;
            continue;
        }
        let return_type = return_token.text.clone();
        let parameters = split_java_args(&tokens[i + 2..params_end], &clean);
        let body_source = clean[tokens[body_open].end..tokens[body_end].start].to_owned();
        let body = parse_java_block(&tokens, body_open, body_end, &clean);
        methods.push(JavaMethod {
            return_type,
            name: tokens[i].text.clone(),
            parameters,
            body,
            body_source,
        });
        i = body_end + 1;
    }
    JavaProgram { methods }
}

fn lex_java(source: &str) -> Vec<JavaToken> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() { i += 1; continue; }
        let start = i;
        let b = bytes[i];
        let kind;
        if b == b'"' || b == b'\\'' {
            let quote = b;
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\\\' { i = (i + 2).min(bytes.len()); }
                else if bytes[i] == quote { i += 1; break; }
                else { i += 1; }
            }
            kind = JavaTokenKind::String;
        } else if b.is_ascii_alphabetic() || b == b'_' || b == b'$' {
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'$') { i += 1; }
            kind = JavaTokenKind::Identifier;
        } else if b.is_ascii_digit() {
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'.' || bytes[i] == b'_') { i += 1; }
            kind = JavaTokenKind::Number;
        } else {
            i += 1;
            if i < bytes.len() && matches!((b, bytes[i]), (b'=', b'=') | (b'!', b'=') | (b'<', b'=') | (b'>', b'=') | (b'&', b'&') | (b'|', b'|') | (b'+', b'+') | (b'-', b'-') | (b'-', b'>')) { i += 1; }
            kind = JavaTokenKind::Symbol;
        }
        tokens.push(JavaToken { text: source[start..i].to_owned(), start, end: i, kind });
    }
    tokens
}

fn match_java(tokens: &[JavaToken], start: usize, open: &str, close: &str) -> Option<usize> {
    let mut depth = 0_i32;
    for (i, token) in tokens.iter().enumerate().skip(start) {
        if token.text == open { depth += 1; }
        else if token.text == close {
            depth -= 1;
            if depth == 0 { return Some(i); }
        }
    }
    None
}

fn split_java_args(tokens: &[JavaToken], source: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut start = 0;
    let (mut paren, mut brace, mut bracket) = (0_i32, 0_i32, 0_i32);
    for (i, token) in tokens.iter().enumerate() {
        match token.text.as_str() {
            "(" => paren += 1, ")" => paren -= 1,
            "{" => brace += 1, "}" => brace -= 1,
            "[" => bracket += 1, "]" => bracket -= 1,
            "," if paren == 0 && brace == 0 && bracket == 0 => {
                args.push(source[tokens[start].start..token.start].trim().to_owned());
                start = i + 1;
            }
            _ => {}
        }
    }
    if start < tokens.len() {
        args.push(source[tokens[start].start..tokens.last().unwrap().end].trim().to_owned());
    }
    args
}

fn parse_java_block(tokens: &[JavaToken], open: usize, close: usize, source: &str) -> JavaStatement {
    let mut statements = Vec::new();
    let mut i = open + 1;
    while i < close {
        let (statement, next) = parse_java_statement(tokens, i, close, source);
        statements.push(statement);
        i = next.max(i + 1);
    }
    JavaStatement::Block(statements)
}

fn parse_java_statement(tokens: &[JavaToken], start: usize, limit: usize, source: &str) -> (JavaStatement, usize) {
    if start >= limit { return (JavaStatement::Empty, limit); }
    if tokens[start].text == ";" { return (JavaStatement::Empty, start + 1); }
    if tokens[start].text == "{" {
        if let Some(end) = match_java(tokens, start, "{", "}") {
            return (parse_java_block(tokens, start, end, source), end + 1);
        }
    }
    if tokens[start].text == "if" && tokens.get(start + 1).is_some_and(|t| t.text == "(") {
        if let Some(condition_end) = match_java(tokens, start + 1, "(", ")") {
            let condition = source[tokens[start + 1].end..tokens[condition_end].start].trim().to_owned();
            let (then_branch, after_then) = parse_java_statement(tokens, condition_end + 1, limit, source);
            if tokens.get(after_then).is_some_and(|t| t.text == "else") {
                let (else_branch, after_else) = parse_java_statement(tokens, after_then + 1, limit, source);
                return (JavaStatement::If {
                    condition, then_branch: Box::new(then_branch),
                    else_branch: Some(Box::new(else_branch)),
                }, after_else);
            }
            return (JavaStatement::If { condition, then_branch: Box::new(then_branch), else_branch: None }, after_then);
        }
    }
    let is_return = tokens[start].text == "return";
    let mut i = start;
    let (mut paren, mut brace, mut bracket) = (0_i32, 0_i32, 0_i32);
    while i < limit {
        match tokens[i].text.as_str() {
            "(" => paren += 1, ")" => paren -= 1,
            "{" => brace += 1, "}" => { if brace == 0 { break; } brace -= 1; },
            "[" => bracket += 1, "]" => bracket -= 1,
            ";" if paren == 0 && brace == 0 && bracket == 0 => break,
            _ => {}
        }
        i += 1;
    }
    let end = if i < limit && tokens[i].text == ";" { i } else { i.saturating_sub(1) };
    if end < start { return (JavaStatement::Empty, (i + 1).min(limit)); }
    let text = source[tokens[start].start..tokens[end].end].trim().to_owned();
    let statement = if is_return {
        let value = text.strip_prefix("return").map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned);
        JavaStatement::Return(value)
    } else {
        JavaStatement::Expression(text)
    };
    (statement, if i < limit && tokens[i].text == ";" { i + 1 } else { i.max(start + 1) })
}

#[cfg(test)]
mod java_ast_tests {
    use super::*;

    #[test]
    fn parses_method_and_nested_if_else() {
        let source = r#"
            private Command shoot() {
                return instant(() -> {
                    if (flywheel.getVelocity() > 1100) {
                        feeder.setPower(1);
                    } else {
                        feeder.setPower(0);
                    }
                });
            }
        "#;
        let program = parse_java_program(source);
        let method = program.methods.iter().find(|m| m.name == "shoot").unwrap();
        assert_eq!(method.return_type, "Command");
        assert!(method.body_source.contains("getVelocity"));
        assert!(matches!(&method.body, JavaStatement::Block(statements)
            if matches!(statements.first(), Some(JavaStatement::Return(Some(_))))));
    }

    #[test]
    fn ignores_method_calls_and_finds_multiple_methods() {
        let source = r#"
            private void init() { follower.update(); }
            private Command startup() { return instant(() -> intake.setPower(1)); }
            private Command routine() { return sequential(startup(), waitMs(20)); }
        "#;
        let program = parse_java_program(source);
        assert!(program.methods.iter().any(|m| m.name == "init"));
        assert!(program.methods.iter().any(|m| m.name == "startup"));
        assert!(program.methods.iter().any(|m| m.name == "routine"));
    }
}


/// Evaluate a Java boolean expression made from comparisons, &&, ||, !,
/// parentheses, and numeric operands. The caller supplies live values for
/// hardware reads and variables, keeping this parser independent of the UI.
pub fn evaluate_boolean<F>(expression: &str, resolve_number: &F) -> Option<bool>
where
    F: Fn(&str) -> Option<f32>,
{
    let expr = strip_outer_parentheses(expression.trim());
    if expr.is_empty() { return None; }
    if let Some((left, right)) = split_boolean_operator(expr, "||") {
        return Some(evaluate_boolean(left, resolve_number)? || evaluate_boolean(right, resolve_number)?);
    }
    if let Some((left, right)) = split_boolean_operator(expr, "&&") {
        return Some(evaluate_boolean(left, resolve_number)? && evaluate_boolean(right, resolve_number)?);
    }
    if let Some(rest) = expr.strip_prefix('!') {
        return Some(!evaluate_boolean(rest, resolve_number)?);
    }
    if expr == "true" { return Some(true); }
    if expr == "false" { return Some(false); }

    for operator in [">=", "<=", "==", "!=", ">", "<"] {
        if let Some((left, right)) = split_boolean_operator(expr, operator) {
            let a = resolve_number(left.trim())?;
            let b = resolve_number(right.trim())?;
            return Some(match operator {
                ">=" => a >= b, "<=" => a <= b, "==" => (a-b).abs() < f32::EPSILON,
                "!=" => (a-b).abs() >= f32::EPSILON, ">" => a > b, "<" => a < b,
                _ => return None,
            });
        }
    }
    resolve_number(expr).map(|value| value != 0.0)
}

fn strip_outer_parentheses(mut expression: &str) -> &str {
    loop {
        let bytes = expression.as_bytes();
        if bytes.first() != Some(&b'(') || bytes.last() != Some(&b')') { return expression; }
        let mut depth = 0_i32;
        let mut quote = 0_u8;
        let mut escaped = false;
        let mut closes_at_end = false;
        for (i, &b) in bytes.iter().enumerate() {
            if quote != 0 {
                if escaped { escaped = false; }
                else if b == b'\\\\' { escaped = true; }
                else if b == quote { quote = 0; }
                continue;
            }
            if b == b'"' || b == b'\\\\'' { quote = b; continue; }
            if b == b'(' { depth += 1; }
            if b == b')' {
                depth -= 1;
                if depth == 0 { closes_at_end = i == bytes.len()-1; break; }
            }
        }
        if !closes_at_end { return expression; }
        expression = expression[1..expression.len()-1].trim();
    }
}

fn split_boolean_operator<'a>(expression: &'a str, operator: &str) -> Option<(&'a str, &'a str)> {
    let bytes = expression.as_bytes();
    let op = operator.as_bytes();
    let mut depth = 0_i32;
    let mut quote = 0_u8;
    let mut escaped = false;
    let mut i = 0;
    while i + op.len() <= bytes.len() {
        let b = bytes[i];
        if quote != 0 {
            if escaped { escaped = false; }
            else if b == b'\\\\' { escaped = true; }
            else if b == quote { quote = 0; }
            i += 1; continue;
        }
        if b == b'"' || b == b'\\\\'' { quote = b; i += 1; continue; }
        if b == b'(' { depth += 1; i += 1; continue; }
        if b == b')' { depth -= 1; i += 1; continue; }
        if depth == 0 && &bytes[i..i+op.len()] == op {
            // A single > or < inside >= / <= is not the requested operator.
            if (operator == ">" && bytes.get(i+1) == Some(&b'=')) ||
               (operator == "<" && bytes.get(i+1) == Some(&b'=')) { i += 1; continue; }
            return Some((&expression[..i], &expression[i+op.len()..]));
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod boolean_tests {
    use super::*;

    #[test]
    fn evaluates_live_hardware_conditions() {
        let condition = "((DcMotorEx)OuttakeL).getVelocity() > 1100 || ((DcMotorEx)OuttakeR).getVelocity() > 1100";
        let result = evaluate_boolean(condition, &|value| {
            if value.contains("getVelocity") && value.contains("OuttakeL") { Some(1090.0) }
            else if value.contains("getVelocity") && value.contains("OuttakeR") { Some(1125.0) }
            else { value.trim().parse::<f32>().ok() }
        });
        assert_eq!(result, Some(true));
        assert_eq!(evaluate_boolean("(2 > 3) || (5 >= 5)", &|v| v.parse().ok()), Some(true));
    }
}
