pub mod cue_parser;

use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use sha1::{Digest, Sha1};

use crate::models::{DiscFingerprint, Platform};

pub use cue_parser::{parse_cue_content, parse_cue_references, parse_gdi_references, CueSheet};

#[derive(Debug, thiserror::Error)]
pub enum ScannerError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Referenced track not found: {0}")]
    MissingTrack(PathBuf),

    #[error("Parse error: {0}")]
    ParseError(String),
}

/// Calculates the SHA-1 checksum on up to the first 16MB of track 1.
pub fn calculate_track1_sha1<P: AsRef<Path>>(track1_path: P) -> std::io::Result<String> {
    let file = File::open(track1_path)?;
    let mut handle = file.take(16 * 1024 * 1024);
    let mut hasher = Sha1::new();
    let mut buffer = [0u8; 64 * 1024];

    loop {
        let n = handle.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

/// Derives platform hints from directory names in the file's path or descriptor type.
pub fn detect_platform_from_path(path: &Path) -> Platform {
    let mut current = path.parent();
    while let Some(parent) = current {
        if let Some(folder_name) = parent.file_name().and_then(|n| n.to_str()) {
            let folder_clean = folder_name.trim();
            let folder_lower = folder_clean.to_ascii_lowercase();

            // Exact match on folder token / acronym
            match folder_lower.as_str() {
                "psx" | "ps1" | "psone" | "ps-one" | "playstation" | "ps" => return Platform::Psx,
                "saturn" | "ss" => return Platform::Saturn,
                "dreamcast" | "dc" => return Platform::Dreamcast,
                "segacd" | "sega-cd" | "sega_cd" | "megacd" | "mega-cd" | "mega_cd" | "scd" => return Platform::SegaCd,
                "pcecd" | "pce-cd" | "pce_cd" | "pcenginecd" | "turbografx-cd" | "tg-cd" | "tgcd" | "pce" => return Platform::PceCd,
                _ => {}
            }

            // Alphanumeric normalization (e.g., "Sega CD" -> "segacd", "Sega Saturn" -> "segasaturn")
            let alphanumeric: String = folder_lower.chars().filter(|c| c.is_alphanumeric()).collect();
            if alphanumeric == "psx" || alphanumeric == "ps1" || alphanumeric == "psone" || alphanumeric == "playstation" || alphanumeric == "sonyplaystation" {
                return Platform::Psx;
            }
            if alphanumeric == "saturn" || alphanumeric == "segasaturn" {
                return Platform::Saturn;
            }
            if alphanumeric == "dreamcast" || alphanumeric == "segadreamcast" {
                return Platform::Dreamcast;
            }
            if alphanumeric == "segacd" || alphanumeric == "megacd" {
                return Platform::SegaCd;
            }
            if alphanumeric == "pcecd" || alphanumeric == "pcenginecd" || alphanumeric == "turbografxcd" || alphanumeric == "turbografx16cd" || alphanumeric == "tgcd" {
                return Platform::PceCd;
            }

            // Keyword containment
            if folder_lower.contains("dreamcast") {
                return Platform::Dreamcast;
            }
            if folder_lower.contains("saturn") {
                return Platform::Saturn;
            }
            if folder_lower.contains("playstation") {
                return Platform::Psx;
            }
            if folder_lower.contains("segacd") || folder_lower.contains("sega cd") || folder_lower.contains("mega cd") || folder_lower.contains("megacd") {
                return Platform::SegaCd;
            }
            if folder_lower.contains("pcecd") || folder_lower.contains("pc engine cd") || folder_lower.contains("turbografx") {
                return Platform::PceCd;
            }
        }
        current = parent.parent();
    }

    // Extension fallback: .gdi files are uniquely Sega Dreamcast GD-ROMs
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        if ext.eq_ignore_ascii_case("gdi") {
            return Platform::Dreamcast;
        }
    }

    Platform::Unknown
}

fn collect_files<P: AsRef<Path>>(dir: P, files: &mut Vec<PathBuf>) -> Result<(), ScannerError> {
    let read_dir = std::fs::read_dir(dir.as_ref())?;
    for entry in read_dir {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

/// Recursively scans a root directory for disc images (.cue, .gdi, standalone .iso/.img),
/// pairs multi-track referenced binary files, computes track 1 SHA-1, and infers platform hints.
pub fn scan_directory<P: AsRef<Path>>(root: P) -> Result<Vec<DiscFingerprint>, ScannerError> {
    let root = root.as_ref();
    if !root.exists() {
        return Err(ScannerError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Directory not found: {}", root.display()),
        )));
    }
    if !root.is_dir() {
        return Err(ScannerError::Io(std::io::Error::new(
            std::io::ErrorKind::NotADirectory,
            format!("Path is not a directory: {}", root.display()),
        )));
    }

    let mut all_files = Vec::new();
    collect_files(root, &mut all_files)?;

    let mut descriptors = Vec::new();
    let mut standalone_candidates = Vec::new();

    for file in all_files {
        let ext = file.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase());
        match ext.as_deref() {
            Some("cue") | Some("gdi") => descriptors.push(file),
            Some("iso") | Some("img") => standalone_candidates.push(file),
            _ => {}
        }
    }

    let mut paired_tracks: HashSet<PathBuf> = HashSet::new();
    let mut fingerprints = Vec::new();

    for desc_path in descriptors {
        let parent = desc_path.parent().unwrap_or(Path::new(""));
        let bytes = std::fs::read(&desc_path)?;
        let content = String::from_utf8_lossy(&bytes);
        let ext = desc_path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase());

        let raw_refs = if ext.as_deref() == Some("gdi") {
            cue_parser::parse_gdi_references(&content)
        } else {
            cue_parser::parse_cue_references(&content)
        };

        let mut binary_tracks = Vec::new();
        for filename in raw_refs {
            match cue_parser::resolve_path_case_insensitive(parent, &filename) {
                Some(resolved) => {
                    paired_tracks.insert(resolved.clone());
                    binary_tracks.push(resolved);
                }
                None => return Err(ScannerError::MissingTrack(parent.join(filename))),
            }
        }

        let mut total_bytes = 0u64;
        for track in &binary_tracks {
            if let Ok(meta) = std::fs::metadata(track) {
                total_bytes += meta.len();
            }
        }

        let calculated_sha1 = if let Some(track1) = binary_tracks.first() {
            calculate_track1_sha1(track1).ok()
        } else {
            None
        };

        let detected_platform = detect_platform_from_path(&desc_path);

        fingerprints.push(DiscFingerprint {
            primary_file: desc_path,
            binary_tracks,
            detected_platform,
            calculated_sha1,
            total_bytes,
        });
    }

    // Process standalone ISO / IMG files not referenced by any descriptor
    for iso_path in standalone_candidates {
        let is_paired = paired_tracks.iter().any(|p| {
            p == &iso_path || p.to_string_lossy().eq_ignore_ascii_case(&iso_path.to_string_lossy())
        });
        if is_paired {
            continue;
        }

        let total_bytes = std::fs::metadata(&iso_path).map(|m| m.len()).unwrap_or(0);
        let calculated_sha1 = calculate_track1_sha1(&iso_path).ok();
        let detected_platform = detect_platform_from_path(&iso_path);

        fingerprints.push(DiscFingerprint {
            primary_file: iso_path.clone(),
            binary_tracks: vec![iso_path],
            detected_platform,
            calculated_sha1,
            total_bytes,
        });
    }

    fingerprints.sort_by(|a, b| a.primary_file.cmp(&b.primary_file));
    Ok(fingerprints)
}
