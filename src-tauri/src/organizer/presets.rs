use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

use crate::models::{FrontendPreset, GameClassification, Platform};

/// Resolved destination paths for a game or disc based on frontend presets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetPaths {
    /// Absolute or root-relative target path for the converted CHD file.
    pub chd_path: PathBuf,
    /// Absolute or root-relative target path for the generated M3U file (multi-disc games only).
    pub m3u_path: Option<PathBuf>,
    /// Relative path entry to be written inside the M3U playlist file (multi-disc games only).
    /// Always formatted with forward slashes (`/`).
    pub relative_m3u_entry: Option<String>,
}

/// Optional configuration override for the `Custom` frontend preset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomPresetConfig {
    pub psx: String,
    pub saturn: String,
    pub dreamcast: String,
    pub sega_cd: String,
    pub pce_cd: String,
    pub unknown: String,
    pub multidisc_subfolder: String,
}

impl Default for CustomPresetConfig {
    fn default() -> Self {
        Self {
            psx: "roms/psx".to_string(),
            saturn: "roms/saturn".to_string(),
            dreamcast: "roms/dreamcast".to_string(),
            sega_cd: "roms/segacd".to_string(),
            pce_cd: "roms/pcenginecd".to_string(),
            unknown: "roms/unknown".to_string(),
            multidisc_subfolder: ".discs".to_string(),
        }
    }
}

impl CustomPresetConfig {
    pub fn get_platform_folder(&self, platform: Platform) -> &str {
        match platform {
            Platform::Psx => &self.psx,
            Platform::Saturn => &self.saturn,
            Platform::Dreamcast => &self.dreamcast,
            Platform::SegaCd => &self.sega_cd,
            Platform::PceCd => &self.pce_cd,
            Platform::Unknown => &self.unknown,
        }
    }

    pub fn get_multidisc_subfolder(&self) -> &str {
        &self.multidisc_subfolder
    }
}

/// Returns the relative platform directory path for a given frontend preset.
///
/// Mappings:
/// - **ES-DE**: `ROMs/{psx,saturn,dreamcast,segacd,pcenginecd}` (ES-DE home-tree convention; gamelists are centralized under `ES-DE/gamelists/`, see `finish_library`)
/// - **OnionOS**: `Roms/{PS,SEGASATURN,DREAMCAST,SEGACD,PCECD}`
/// - **AnbernicStock**: `ROMS/{PS,SATURN,DC,MDCD,PCE}`
/// - **Batocera**: `roms/{psx,saturn,dreamcast,segacd,pcenginecd}`
/// - **Custom**: provided by `CustomPresetConfig` (defaults to `roms/{psx,...}` when absent)
pub fn get_platform_folder(preset: FrontendPreset, platform: Platform) -> &'static str {
    match preset {
        FrontendPreset::EsDe => match platform {
            Platform::Psx => "ROMs/psx",
            Platform::Saturn => "ROMs/saturn",
            Platform::Dreamcast => "ROMs/dreamcast",
            Platform::SegaCd => "ROMs/segacd",
            Platform::PceCd => "ROMs/pcenginecd",
            Platform::Unknown => "ROMs/unknown",
        },
        FrontendPreset::OnionOs => match platform {
            Platform::Psx => "Roms/PS",
            Platform::Saturn => "Roms/SEGASATURN",
            Platform::Dreamcast => "Roms/DREAMCAST",
            Platform::SegaCd => "Roms/SEGACD",
            Platform::PceCd => "Roms/PCECD",
            Platform::Unknown => "Roms/UNKNOWN",
        },
        FrontendPreset::AnbernicStock => match platform {
            Platform::Psx => "ROMS/PS",
            Platform::Saturn => "ROMS/SATURN",
            Platform::Dreamcast => "ROMS/DC",
            Platform::SegaCd => "ROMS/MDCD",
            Platform::PceCd => "ROMS/PCE",
            Platform::Unknown => "ROMS/UNKNOWN",
        },
        FrontendPreset::Batocera => match platform {
            Platform::Psx => "roms/psx",
            Platform::Saturn => "roms/saturn",
            Platform::Dreamcast => "roms/dreamcast",
            Platform::SegaCd => "roms/segacd",
            Platform::PceCd => "roms/pcenginecd",
            Platform::Unknown => "roms/unknown",
        },
        FrontendPreset::Custom => match platform {
            Platform::Psx => "roms/psx",
            Platform::Saturn => "roms/saturn",
            Platform::Dreamcast => "roms/dreamcast",
            Platform::SegaCd => "roms/segacd",
            Platform::PceCd => "roms/pcenginecd",
            Platform::Unknown => "roms/unknown",
        },
    }
}

/// Sanitizes a single path component (title, region, or custom folder segment)
/// derived from untrusted classification data or user input.
///
/// Strips path separators, parent references, Windows-illegal characters and
/// control characters; collapses whitespace; falls back to `fallback` when empty.
pub fn sanitize_component(input: &str, fallback: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '/' | '\\' | ':' | '?' | '*' | '"' | '<' | '>' | '|' => {}
            '.' => {
                // Keep interior dots; drop runs that would form `..`
                if !out.ends_with('.') {
                    out.push('.');
                }
            }
            c if c.is_control() => {}
            c if c.is_whitespace() => {
                if !out.ends_with(' ') {
                    out.push(' ');
                }
            }
            c => out.push(c),
        }
    }
    let trimmed = out.trim_matches(|c: char| c == '.' || c.is_whitespace());
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

/// Sanitizes a user-supplied relative folder path (custom preset): each
/// `/`-separated segment is sanitized and empty segments are dropped.
pub fn sanitize_relative_folder(folder: &str) -> String {
    let parts: Vec<String> = folder
        .replace('\\', "/")
        .split('/')
        .map(|seg| sanitize_component(seg, ""))
        .filter(|seg| !seg.is_empty())
        .collect();
    if parts.is_empty() {
        "roms".to_string()
    } else {
        parts.join("/")
    }
}

/// Platform folder resolution honoring a `CustomPresetConfig` when the preset
/// is `Custom` (and one was supplied).
pub fn get_platform_folder_with_custom(
    preset: FrontendPreset,
    platform: Platform,
    custom: Option<&CustomPresetConfig>,
) -> String {
    match (preset, custom) {
        (FrontendPreset::Custom, Some(cfg)) => {
            sanitize_relative_folder(cfg.get_platform_folder(platform))
        }
        _ => get_platform_folder(preset, platform).to_string(),
    }
}

/// Returns the hidden multi-disc subfolder name for a given frontend preset.
///
/// All standard presets currently use `.discs` to hide individual disc images
/// from the frontend game list, while the root `.m3u` playlist is displayed.
pub fn get_multidisc_subfolder(_preset: FrontendPreset) -> &'static str {
    ".discs"
}

/// Joins a base root path and a relative folder using forward slashes (`/`),
/// normalizing separators cleanly for all operating systems.
fn join_forward_slashes(root: &Path, folder: &str) -> String {
    let s = root.to_string_lossy().replace('\\', "/");
    let trimmed = s.trim_end_matches('/');
    if trimmed.is_empty() {
        if s.starts_with('/') {
            format!("/{}", folder)
        } else {
            folder.to_string()
        }
    } else {
        format!("{}/{}", trimmed, folder)
    }
}

/// Resolves target CHD and M3U file paths for a disc according to frontend preset conventions.
///
/// For multi-disc games:
/// - Places CHD in `<output_root>/<platform_folder>/<multidisc_subfolder>/<Title> (<Region>) (Disc N).chd`
/// - Sets `m3u_path` to `<output_root>/<platform_folder>/<Title> (<Region>).m3u`
/// - Sets `relative_m3u_entry` to `<multidisc_subfolder>/<Title> (<Region>) (Disc N).chd`
///
/// For single-disc games:
/// - Places CHD in `<output_root>/<platform_folder>/<Title> (<Region>).chd`
/// - `m3u_path` and `relative_m3u_entry` are `None`.
#[allow(clippy::too_many_arguments)]
pub fn resolve_target_paths(
    output_root: &Path,
    preset: FrontendPreset,
    platform: Platform,
    canonical_title: &str,
    region: &str,
    is_multidisc: bool,
    disc_number: Option<u8>,
    _total_discs: Option<u8>,
) -> TargetPaths {
    resolve_target_paths_with_custom(
        output_root,
        preset,
        None,
        platform,
        canonical_title,
        region,
        is_multidisc,
        disc_number,
        _total_discs,
    )
}

/// `resolve_target_paths` with an optional `CustomPresetConfig`, honored when
/// `preset` is `Custom`. Titles and regions are sanitized before they become
/// path components, since they may originate from the remote Jev API or
/// arbitrary filenames.
#[allow(clippy::too_many_arguments)]
pub fn resolve_target_paths_with_custom(
    output_root: &Path,
    preset: FrontendPreset,
    custom_config: Option<&CustomPresetConfig>,
    platform: Platform,
    canonical_title: &str,
    region: &str,
    is_multidisc: bool,
    disc_number: Option<u8>,
    _total_discs: Option<u8>,
) -> TargetPaths {
    let platform_folder = get_platform_folder_with_custom(preset, platform, custom_config);
    let multidisc_subfolder = match (preset, custom_config) {
        (FrontendPreset::Custom, Some(cfg)) => sanitize_component(cfg.get_multidisc_subfolder(), ".discs"),
        _ => get_multidisc_subfolder(preset).to_string(),
    };

    let trimmed_title = sanitize_component(canonical_title.trim(), "Unknown Game");
    let trimmed_region = sanitize_component(region.trim(), "");

    let base_name = if trimmed_region.is_empty()
        || trimmed_title.ends_with(&format!("({})", trimmed_region))
    {
        trimmed_title.clone()
    } else {
        format!("{} ({})", trimmed_title, trimmed_region)
    };

    let base_dir = join_forward_slashes(output_root, &platform_folder);

    if is_multidisc {
        let disc_stem = if let Some(d) = disc_number {
            format!("{} (Disc {})", base_name, d)
        } else {
            base_name.clone()
        };

        let chd_filename = format!("{}.chd", disc_stem);
        let m3u_filename = format!("{}.m3u", base_name);

        let chd_path = PathBuf::from(format!(
            "{}/{}/{}",
            base_dir, multidisc_subfolder, chd_filename
        ));
        let m3u_path = Some(PathBuf::from(format!("{}/{}", base_dir, m3u_filename)));
        let relative_m3u_entry = Some(format!("{}/{}", multidisc_subfolder, chd_filename));

        TargetPaths {
            chd_path,
            m3u_path,
            relative_m3u_entry,
        }
    } else {
        let chd_filename = format!("{}.chd", base_name);
        let chd_path = PathBuf::from(format!("{}/{}", base_dir, chd_filename));

        TargetPaths {
            chd_path,
            m3u_path: None,
            relative_m3u_entry: None,
        }
    }
}

/// Convenience helper to resolve target paths directly from a `GameClassification`.
pub fn resolve_target_paths_for_classification(
    output_root: &Path,
    preset: FrontendPreset,
    classification: &GameClassification,
) -> TargetPaths {
    resolve_target_paths(
        output_root,
        preset,
        classification.platform,
        &classification.canonical_title,
        &classification.region,
        classification.is_multidisc,
        classification.disc_number,
        classification.total_discs,
    )
}
