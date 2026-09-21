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

/// Resolves the first available artwork CDN URL for a given platform, title, and region.
pub async fn resolve_artwork_url(
    client: &reqwest::Client,
    platform: Platform,
    title: &str,
    region: &str,
) -> Option<String> {
    let candidates = generate_candidate_urls(platform, title, region, MediaType::BoxArt);
    resolve_artwork_url_from_candidates(client, &candidates).await
}

/// Queries each candidate CDN URL using HTTP HEAD requests in order,
/// returning the first URL that returns HTTP 200 OK.
pub async fn resolve_artwork_url_from_candidates(
    client: &reqwest::Client,
    candidates: &[String],
) -> Option<String> {
    for url in candidates {
        if let Ok(res) = client.head(url).send().await {
            if res.status() == reqwest::StatusCode::OK {
                return Some(url.clone());
            }
        }
    }
    None
}

/// Helper RAII guard that cleans up partial download files on early exit or error.
struct PartFileGuard {
    path: PathBuf,
    completed: bool,
}

impl Drop for PartFileGuard {
    fn drop(&mut self) {
        if !self.completed && self.path.exists() {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Asynchronously downloads a media file to `dest_path` using an atomic write pattern.
///
/// Writes payload chunks into `<dest_path>.part` and atomically renames to `dest_path`
/// once the transfer succeeds and stream is flushed. On any error or network disruption,
/// any temporary `.part` file is deleted.
pub async fn download_media_file(
    client: &reqwest::Client,
    url: &str,
    dest_path: &Path,
) -> Result<(), String> {
    if let Some(parent) = dest_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| {
            format!(
                "Failed to create parent directory for {}: {}",
                dest_path.display(),
                e
            )
        })?;
    }

    let part_path = PathBuf::from(format!("{}.part", dest_path.to_string_lossy()));

    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Request failed for {}: {}", url, e))?;

    let status = response.status();
    if status != reqwest::StatusCode::OK {
        return Err(format!("Download failed for {}: HTTP {}", url, status));
    }

    let mut guard = PartFileGuard {
        path: part_path.clone(),
        completed: false,
    };

    {
        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::File::create(&part_path).await.map_err(|e| {
            format!(
                "Failed to create temporary file {}: {}",
                part_path.display(),
                e
            )
        })?;

        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| format!("Failed to read stream chunk from {}: {}", url, e))?
        {
            file.write_all(&chunk)
                .await
                .map_err(|e| format!("Failed to write chunk to {}: {}", part_path.display(), e))?;
        }

        file.flush().await.map_err(|e| {
            format!("Failed to flush stream to {}: {}", part_path.display(), e)
        })?;
    }

    if dest_path.exists() {
        tokio::fs::remove_file(dest_path).await.map_err(|e| {
            format!(
                "Failed to remove existing file {}: {}",
                dest_path.display(),
                e
            )
        })?;
    }

    tokio::fs::rename(&part_path, dest_path).await.map_err(|e| {
        format!(
            "Failed to rename {} to {}: {}",
            part_path.display(),
            dest_path.display(),
            e
        )
    })?;

    guard.completed = true;
    Ok(())
}

/// Computes the target media paths for a game according to the frontend preset,
/// platform, canonical title, region, and user MediaOptions.
pub fn resolve_media_paths_for_game(
    output_dir: &Path,
    preset: FrontendPreset,
    platform: Platform,
    canonical_title: &str,
    region: &str,
    media_options: &MediaOptions,
) -> Vec<PathBuf> {
    let trimmed_title = canonical_title.trim();
    let trimmed_region = region.trim();

    let stem = if trimmed_region.is_empty()
        || trimmed_title.ends_with(&format!("({})", trimmed_region))
    {
        trimmed_title.to_string()
    } else {
        format!("{} ({})", trimmed_title, trimmed_region)
    };

    let mut paths = Vec::new();

    if media_options.download_boxart {
        paths.push(resolve_preset_media_path(
            output_dir,
            preset,
            platform,
            &stem,
            MediaType::BoxArt,
        ));
    }

    if media_options.download_screenshots {
        paths.push(resolve_preset_media_path(
            output_dir,
            preset,
            platform,
            &stem,
            MediaType::Screenshots,
        ));
    }

    if media_options.download_titles {
        paths.push(resolve_preset_media_path(
            output_dir,
            preset,
            platform,
            &stem,
            MediaType::TitleScreens,
        ));
    }

    paths
}

