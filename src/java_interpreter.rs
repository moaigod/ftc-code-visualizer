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
