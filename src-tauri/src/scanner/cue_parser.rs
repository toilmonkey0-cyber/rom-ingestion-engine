use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use crate::scanner::ScannerError;

static CUE_FILE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)^\s*FILE\s+(?:"([^"]+)"|'([^']+)'|(\S+))"#).expect("valid cue FILE regex")
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CueSheet {
    pub files: Vec<String>,
}

impl CueSheet {
    pub fn parse(content: &str) -> Self {
        Self {
            files: parse_cue_references(content),
        }
    }
}

/// Parses file references from a CUE sheet.
/// Matches `FILE "<name>" <TYPE>` or `FILE '<name>' <TYPE>` or unquoted `FILE <name> <TYPE>`.
/// Skips comment lines beginning with `REM`.
pub fn parse_cue_references(content: &str) -> Vec<String> {
    let mut refs = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.to_ascii_uppercase().starts_with("REM ") || trimmed.eq_ignore_ascii_case("REM") {
            continue;
        }
        if let Some(cap) = CUE_FILE_RE.captures(trimmed) {
            if let Some(m) = cap.get(1).or_else(|| cap.get(2)).or_else(|| cap.get(3)) {
                refs.push(m.as_str().to_string());
            }
        }
    }

    refs
}

/// Parses file references from a Dreamcast GDI sheet.
/// The first non-empty line indicates the track count, and subsequent lines contain track definitions:
/// `<track_num> <starting_lba> <track_type> <sector_size> <filename> <offset>`
pub fn parse_gdi_references(content: &str) -> Vec<String> {
    let mut tracks = Vec::new();
    let mut lines = content.lines().map(|l| l.trim()).filter(|l| !l.is_empty());

    // First line is track count
    let _track_count: Option<usize> = lines.next().and_then(|l| l.parse().ok());

    for line in lines {
        if line.starts_with(';') || line.starts_with('#') {
            continue; // comment
        }

        // Check for quoted filename first
        if let Some(start_quote) = line.find('"') {
            if let Some(end_quote) = line[start_quote + 1..].find('"') {
                let filename = &line[start_quote + 1..start_quote + 1 + end_quote];
                tracks.push(filename.to_string());
                continue;
            }
        }

        // Check for single quotes
        if let Some(start_quote) = line.find('\'') {
            if let Some(end_quote) = line[start_quote + 1..].find('\'') {
                let filename = &line[start_quote + 1..start_quote + 1 + end_quote];
                tracks.push(filename.to_string());
                continue;
            }
        }

        // Fallback: tokenize whitespace
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 5 {
            tracks.push(parts[4].to_string());
        }
    }

    tracks
}

/// Resolves a filename against a base directory, supporting case-insensitive file system lookups.
pub fn resolve_path_case_insensitive<P: AsRef<Path>>(base_dir: P, filename: &str) -> Option<PathBuf> {
    let base = base_dir.as_ref();
    let exact = base.join(filename);
    if exact.exists() {
        return Some(exact);
    }

    let normalized = filename.replace('\\', "/");
    let rel = Path::new(&normalized);
    let mut current = base.to_path_buf();

    for comp in rel.components() {
        let comp_str = comp.as_os_str().to_string_lossy();
        if comp_str == "." {
            continue;
        }
        if comp_str == ".." {
            current.pop();
            continue;
        }

        if let Ok(entries) = std::fs::read_dir(&current) {
            let mut matched = false;
            for entry in entries.flatten() {
                if entry.file_name().to_string_lossy().eq_ignore_ascii_case(&comp_str) {
                    current = entry.path();
                    matched = true;
                    break;
                }
            }
            if !matched {
                return None;
            }
        } else {
            return None;
        }
    }

    if current.exists() {
        Some(current)
    } else {
        None
    }
}

/// Parses CUE content and resolves track paths against `base_dir`.
/// Returns `Err(ScannerError::MissingTrack)` if any referenced track does not exist.
pub fn parse_cue_content(content: &str, base_dir: &Path) -> Result<Vec<PathBuf>, ScannerError> {
    let refs = parse_cue_references(content);
    let mut paths = Vec::with_capacity(refs.len());

    for filename in refs {
        match resolve_path_case_insensitive(base_dir, &filename) {
            Some(resolved) => paths.push(resolved),
            None => return Err(ScannerError::MissingTrack(base_dir.join(filename))),
        }
    }

    Ok(paths)
}
