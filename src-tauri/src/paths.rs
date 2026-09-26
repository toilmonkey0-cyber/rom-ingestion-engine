use std::path::{Component, Path, PathBuf};

/// Collapse `.` and `..` without touching the filesystem.
pub fn lexical_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// True when `candidate` stays inside `root` after lexical `..` resolution.
pub fn is_lexically_within(root: &Path, candidate: &Path) -> bool {
    let root = lexical_normalize(root);
    let candidate = lexical_normalize(candidate);
    if root.as_os_str().is_empty() {
        return false;
    }
    candidate.starts_with(&root)
}

/// True when both paths exist and the candidate canonicalizes inside the root.
pub fn existing_path_within(root: &Path, candidate: &Path) -> bool {
    let Ok(root) = root.canonicalize() else {
        return false;
    };
    let Ok(candidate) = candidate.canonicalize() else {
        return false;
    };
    candidate.starts_with(&root)
}

fn is_reserved_windows_name(name: &str) -> bool {
    let base = name.split('.').next().unwrap_or(name);
    matches!(
        base.to_ascii_uppercase().as_str(),
        "CON" | "PRN" | "AUX" | "NUL"
            | "COM1" | "COM2" | "COM3" | "COM4" | "COM5" | "COM6" | "COM7" | "COM8" | "COM9"
            | "LPT1" | "LPT2" | "LPT3" | "LPT4" | "LPT5" | "LPT6" | "LPT7" | "LPT8" | "LPT9"
    )
}

/// File stem safe for Windows and for a path that must not escape its directory.
pub fn sanitize_file_stem(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        match c {
            '&' | '*' | '/' | ':' | '\\' | '<' | '>' | '?' | '|' | '"' => out.push('_'),
            c if c.is_control() => out.push('_'),
            _ => out.push(c),
        }
    }
    let trimmed = out.trim().trim_end_matches(['.', ' ']);
    let collapsed = trimmed.replace("..", "_");
    if collapsed.is_empty() || is_reserved_windows_name(&collapsed) {
        "game".to_string()
    } else {
        collapsed
    }
}

/// chdman subcommand for a disc descriptor. GD-ROM images use `createdvd`.
pub fn chdman_command_for_input(path: &Path) -> Result<&'static str, String> {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("gdi") => Ok("createdvd"),
        Some("cue") | Some("iso") | Some("img") => Ok("createcd"),
        Some(other) => Err(format!("unsupported disc descriptor .{other}")),
        None => Err("disc descriptor has no extension".to_string()),
    }
}
