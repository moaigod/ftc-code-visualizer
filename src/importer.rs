use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct TelemetryItem {
    pub caption: String,
    pub expression: String,
}

#[derive(Clone, Debug)]
pub struct OpModeInfo {
    pub name: String,
    pub kind: String,
    pub language: String,
    pub path: PathBuf,
    pub source: String,
    pub telemetry: Vec<TelemetryItem>,
}

#[derive(Debug)]
pub struct ProjectScan {
    pub project_name: String,
    pub source_root: PathBuf,
    pub op_modes: Vec<OpModeInfo>,
    pub source_files: usize,
}

pub fn scan_project(project_root: &Path) -> Result<ProjectScan, String> {
    let team_code = project_root.join("TeamCode").join("src").join("main").join("java");
    let source_root = if team_code.is_dir() {
        team_code
    } else {
        let conventional = project_root.join("src").join("main").join("java");
        if conventional.is_dir() {
            conventional
        } else {
            return Err(
                "Could not find TeamCode/src/main/java. Select the root folder of your FTC Android Studio project."
                    .to_owned(),
            );
        }
    };

    let mut files = Vec::new();
    collect_source_files(&source_root, &mut files)?;
    let source_files = files.len();
    let mut loaded_sources = Vec::with_capacity(files.len());
    for path in files {
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
        loaded_sources.push((path, source));
    }

    // Hardware declarations may live in a dedicated RobotConfig.java file.
    // Append those source files to each OpMode's interpreter input so selecting
    // an OpMode does not require the declarations to be physically in that file.
    let hardware_config_sources: Vec<(PathBuf, String)> = loaded_sources.iter()
        .filter(|(_, source)| source.contains("ftcCV.hardware.motor"))
        .map(|(path, source)| (path.clone(), source.clone()))
        .collect();

    let mut op_modes = Vec::new();
    for (path, source) in loaded_sources {
        if let Some(mut info) = parse_op_mode(path.clone(), source) {
            for (config_path, config_source) in &hardware_config_sources {
                if config_path != &path && !info.source.contains(config_source) {
                    info.source.push_str("\n\\n// Imported ftcCV hardware declarations from ");
                    info.source.push_str(&config_path.display().to_string());
                    info.source.push_str("\n");
                    info.source.push_str(config_source);
                    info.source.push_str("\n");
                }
            }
            op_modes.push(info);
        }
    }

    op_modes.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    Ok(ProjectScan {
        project_name: project_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("FTC project")
            .to_owned(),
        source_root,
        op_modes,
        source_files,
    })
}

fn collect_source_files(directory: &Path, output: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("Could not open {}: {error}", directory.display()))?;

    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_source_files(&path, output)?;
        } else if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("java" | "kt")
        ) {
            output.push(path);
        }
    }

    Ok(())
}

fn parse_op_mode(path: PathBuf, source: String) -> Option<OpModeInfo> {
    let annotation_line = source.lines().find(|line| {
        let line = line.trim();
        line.starts_with("@TeleOp") || line.starts_with("@Autonomous")
    })?;

    let kind = if annotation_line.contains("@Autonomous") {
        "Autonomous"
    } else {
        "TeleOp"
    };

    let name = annotation_name(annotation_line)
        .or_else(|| find_class_name(&source))
        .or_else(|| path.file_stem()?.to_str().map(str::to_owned))?;

    let language = match path.extension().and_then(|extension| extension.to_str()) {
        Some("kt") => "Kotlin",
        _ => "Java",
    };

    Some(OpModeInfo {
        name,
        kind: kind.to_owned(),
        language: language.to_owned(),
        path,
        telemetry: parse_telemetry(&source),
        source,
    })
}

fn annotation_name(line: &str) -> Option<String> {
    let name_position = line.find("name")?;
    let after_name = &line[name_position + 4..];
    let quote_start = after_name.find('"')?;
    let after_quote = &after_name[quote_start + 1..];
    let quote_end = after_quote.find('"')?;
    Some(after_quote[..quote_end].to_owned())
}

fn find_class_name(source: &str) -> Option<String> {
    for line in source.lines() {
        let line = line.trim();
        for keyword in ["class ", "object "] {
            if let Some(position) = line.find(keyword) {
                let remainder = &line[position + keyword.len()..];
                let name: String = remainder
                    .chars()
                    .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
                    .collect();
                if !name.is_empty() {
                    return Some(name);
                }
            }
        }
    }
    None
}


fn parse_telemetry(source: &str) -> Vec<TelemetryItem> {
    let mut items = Vec::new();
    for line in source.lines() {
        let line = line.trim();
        if let Some(start) = line.find("telemetry.addData(") {
            let args = &line[start + "telemetry.addData(".len()..];
            if let Some(end) = args.find(");") {
                let args = &args[..end];
                if let Some(comma) = args.find(',') {
                    let caption = args[..comma].trim().trim_matches('"').trim_matches('\'').to_owned();
                    let expression = args[comma + 1..].trim().to_owned();
                    if !caption.is_empty() && !expression.is_empty() {
                        items.push(TelemetryItem { caption, expression });
                    }
                }
            }
        } else if line.contains("telemetry.addLine(") {
            let start = line.find("telemetry.addLine(").unwrap() + "telemetry.addLine(".len();
            if let Some(end) = line[start..].find(");") {
                let value = line[start..start + end].trim().trim_matches('"').trim_matches('\'').to_owned();
                if !value.is_empty() {
                    items.push(TelemetryItem { caption: value, expression: String::new() });
                }
            }
        }
    }
    items
}
