use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub use crate::models::{MediaOptions, MediaType};
use crate::models::{FrontendPreset, Platform};
use crate::organizer::presets::{get_platform_folder, join_forward_slashes};

/// Maps a supported disc platform to its corresponding Libretro system taxonomy name.
///
/// Returns `None` for unsupported or `Unknown` platforms.
pub fn platform_to_libretro_system(platform: Platform) -> Option<&'static str> {
    match platform {
        Platform::Psx => Some("Sony - PlayStation"),
        Platform::Saturn => Some("Sega - Saturn"),
        Platform::Dreamcast => Some("Sega - Dreamcast"),
        Platform::SegaCd => Some("Sega - Mega-CD - Sega CD"),
        Platform::PceCd => Some("NEC - PC Engine CD - TurboGrafx-CD"),
        Platform::Unknown => None,
    }
}

/// Sanitizes a title string according to Libretro repository file naming conventions.
///
/// Characters invalid in Libretro file repositories (`&`, `*`, `/`, `:`, `\`, `<`, `>`, `?`, `|`)
/// are replaced with `_`.
pub fn sanitize_libretro_title(title: &str) -> String {
    title
        .chars()
        .map(|c| match c {
            '&' | '*' | '/' | ':' | '\\' | '<' | '>' | '?' | '|' => '_',
            _ => c,
        })
        .collect()
}

/// Returns the Libretro repository folder name for a given media type.
pub fn media_type_to_libretro_folder(media_type: MediaType) -> &'static str {
    match media_type {
        MediaType::BoxArt => "Named_Boxarts",
        MediaType::Screenshots => "Named_Snaps",
        MediaType::TitleScreens => "Named_Titles",
    }
}

/// Generates a prioritized list of candidate CDN URLs for a given game and media type.
///
/// Candidate Fallback Order:
/// 1. `"{CanonicalTitle} ({Region})"` (Exact match with region tag)
/// 2. `"{CanonicalTitle}"` (Title only without region tag)
/// 3. Clean title stem (stripping subtitles after `:` or ` - `)
pub fn generate_candidate_urls(
    platform: Platform,
    title: &str,
    region: &str,
    media_type: MediaType,
) -> Vec<String> {
    let system = match platform_to_libretro_system(platform) {
        Some(s) => s,
        None => return Vec::new(),
    };

    let system_repo = system.replace(' ', "_");
    let media_folder = media_type_to_libretro_folder(media_type);

    let trimmed_title = title.trim();
    let trimmed_region = region.trim();

    // Strip region tag from title if already present at the end
    let base_title = if !trimmed_region.is_empty()
        && trimmed_title.ends_with(&format!("({})", trimmed_region))
    {
        trimmed_title
            .trim_end_matches(&format!("({})", trimmed_region))
            .trim()
    } else {
        trimmed_title
    };

    let mut candidate_titles = Vec::new();

    // 1. Exact match: "{CanonicalTitle} ({Region})"
    if !trimmed_region.is_empty() {
        candidate_titles.push(format!("{} ({})", base_title, trimmed_region));
    }

    // 2. Title only: "{CanonicalTitle}"
    candidate_titles.push(base_title.to_string());

    // 3. Clean title stem (stripping subtitles after ':' or ' - ')
    let stem = if let Some((head, _)) = base_title.split_once(':') {
        Some(head.trim())
    } else if let Some((head, _)) = base_title.split_once(" - ") {
        Some(head.trim())
    } else {
        None
    };

    if let Some(s) = stem {
        if !s.is_empty() && s != base_title {
            if !trimmed_region.is_empty() {
                candidate_titles.push(format!("{} ({})", s, trimmed_region));
            }
            candidate_titles.push(s.to_string());
        }
    }

    let mut urls = Vec::new();
    let mut seen = HashSet::new();

    for candidate in candidate_titles {
        let sanitized = sanitize_libretro_title(&candidate);
        if seen.insert(sanitized.clone()) {
            urls.push(format!(
                "https://raw.githubusercontent.com/libretro-thumbnails/{}/master/{}/{}.png",
                system_repo, media_folder, sanitized
            ));
        }
    }

    urls
}

/// Resolves the destination media file path for a frontend preset, platform, file stem, and media type.
///
/// Preset Mappings:
/// - **Anbernic Stock OS:** `ROMS/<SYSTEM>/Imgs/<Stem>.png`
/// - **OnionOS / GarlicOS:** `Roms/<SYSTEM>/Imgs/<Stem>.png`
/// - **ES-DE:** `roms/<system>/media/covers/<Stem>.png` (or `media/screenshots/`, `media/titlescreens/`)
/// - **Batocera / Knulli:** `roms/<system>/images/<Stem>-thumb.png` (or `-screenshot.png`, `-titlescreen.png`)
/// - **Custom:** `roms/<system>/media/covers/<Stem>.png`
pub fn resolve_preset_media_path(
    output_dir: &Path,
    preset: FrontendPreset,
    platform: Platform,
    stem: &str,
    media_type: MediaType,
) -> PathBuf {
    let platform_folder = get_platform_folder(preset, platform);
    let clean_stem = stem.strip_suffix(".png").unwrap_or(stem);

    let (rel_media_folder, filename) = match preset {
        FrontendPreset::AnbernicStock => ("Imgs", format!("{}.png", clean_stem)),
        FrontendPreset::OnionOs => ("Imgs", format!("{}.png", clean_stem)),
        FrontendPreset::EsDe | FrontendPreset::Custom => {
            let sub = match media_type {
                MediaType::BoxArt => "media/covers",
                MediaType::Screenshots => "media/screenshots",
                MediaType::TitleScreens => "media/titlescreens",
            };
            (sub, format!("{}.png", clean_stem))
        }
        FrontendPreset::Batocera => {
            let filename = match media_type {
                MediaType::BoxArt => format!("{}-thumb.png", clean_stem),
                MediaType::Screenshots => format!("{}-screenshot.png", clean_stem),
                MediaType::TitleScreens => format!("{}-titlescreen.png", clean_stem),
            };
            ("images", filename)
        }
    };

    let base_dir = join_forward_slashes(output_dir, platform_folder);
    PathBuf::from(format!("{}/{}/{}", base_dir, rel_media_folder, filename))
}
