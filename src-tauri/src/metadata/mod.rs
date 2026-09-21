pub mod artwork;
pub mod gamelist;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::commands::EventSink;
use crate::models::{ArtworkProgressEvent, ArtworkStatus, FinishLibrarySummary, IngestionPlan};

use artwork::thumbnail_candidates;

/// The file a frontend loads for a planned game: the M3U playlist for
/// multi-disc titles, the CHD itself otherwise.
fn display_file(game: &crate::models::PlannedGame) -> Option<PathBuf> {
    if !game.enabled {
        return None;
    }
    game.target_m3u_path
        .clone()
        .or_else(|| game.discs.first().map(|d| d.target_chd_path.clone()))
}

/// Destination for a game's box art according to the preset:
/// - ES-DE / Batocera: `<platform folder>/media/images/<stem>.png`,
///   referenced from the generated `gamelist.xml`.
/// - Others: `<stem>.png` next to the playlist/CHD for frontends that
///   auto-load adjacent art (MinUI-style conventions).
fn artwork_dest(preset: &crate::models::FrontendPreset, display: &Path) -> PathBuf {
    use crate::models::FrontendPreset::*;
    let parent = display.parent().unwrap_or_else(|| Path::new(""));
    let stem = display
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("artwork");
    match preset {
        EsDe | Batocera => parent.join("media").join("images").join(format!("{}.png", stem)),
        _ => parent.join(format!("{}.png", stem)),
    }
}

/// Writes `gamelist.xml` files and downloads box art for a converted library.
///
/// `thumbnail_base_url` overrides the libretro thumbnail host (test hook).
/// Art files that already exist are skipped; per-game progress is reported
/// through `artwork-progress` events.
pub async fn finish_library_internal<E: EventSink + Clone + Send + Sync + 'static>(
    emitter: &E,
    plan: &IngestionPlan,
    download_artwork: bool,
    thumbnail_base_url: Option<&str>,
) -> Result<FinishLibrarySummary, String> {
    use crate::models::FrontendPreset::*;

    let base = thumbnail_base_url.unwrap_or(artwork::LIBRETRO_THUMBNAILS_BASE);
    let mut summary = FinishLibrarySummary {
        gamelists_written: 0,
        artwork_downloaded: 0,
        artwork_skipped: 0,
        artwork_failed: 0,
    };

    // 1. Collect enabled games with a display file, deduplicated by art
    //    destination (two games cannot share one image).
    let mut targets: Vec<(PathBuf, PathBuf, String, String, crate::models::Platform)> =
        Vec::new(); // (display, art_dest, title, region, platform)
    let mut seen_dests = std::collections::HashSet::new();
    for game in &plan.games {
        if let Some(display) = display_file(game) {
            let dest = artwork_dest(&plan.preset, &display);
            if seen_dests.insert(dest.clone()) {
                targets.push((
                    display,
                    dest,
                    game.canonical_title.clone(),
                    game.region.clone(),
                    game.platform,
                ));
            }
        }
    }

    let total = targets.len();
    let client = reqwest::Client::builder()
        .user_agent("rom-ingest-artwork/0.1.0")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    // 2. Artwork phase
    for (idx, (_display, dest, title, region, platform)) in targets.iter().enumerate() {
        let completed = idx;
        if !download_artwork {
            continue;
        }
        if dest.exists() {
            summary.artwork_skipped += 1;
            emitter.emit_artwork_progress(&ArtworkProgressEvent {
                game_id: String::new(),
                title: title.clone(),
                status: ArtworkStatus::Skipped,
                completed,
                total,
            });
            continue;
        }

        emitter.emit_artwork_progress(&ArtworkProgressEvent {
            game_id: String::new(),
            title: title.clone(),
            status: ArtworkStatus::Downloading,
            completed,
            total,
        });

        let urls = thumbnail_candidates(*platform, title, region, base);
        let mut fetched: Option<Vec<u8>> = None;
        for url in &urls {
            match client.get(url).send().await {
                Ok(resp) if resp.status().is_success() => match resp.bytes().await {
                    Ok(b) if b.len() > 128 => {
                        fetched = Some(b.to_vec());
                        break;
                    }
                    _ => continue,
                },
                _ => continue,
            }
        }

        match fetched {
            Some(bytes) => {
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| format!("Cannot create media folder '{}': {}", parent.display(), e))?;
                }
                std::fs::write(dest, &bytes)
                    .map_err(|e| format!("Cannot write artwork '{}': {}", dest.display(), e))?;
                summary.artwork_downloaded += 1;
                emitter.emit_artwork_progress(&ArtworkProgressEvent {
                    game_id: String::new(),
                    title: title.clone(),
                    status: ArtworkStatus::Done,
                    completed: completed + 1,
                    total,
                });
            }
            None => {
                summary.artwork_failed += 1;
                emitter.emit_artwork_progress(&ArtworkProgressEvent {
                    game_id: String::new(),
                    title: title.clone(),
                    status: ArtworkStatus::Failed,
                    completed: completed + 1,
                    total,
                });
            }
        }
    }

    // 3. Gamelist phase (ES-DE / Batocera are EmulationStation-derived and
    //    both read gamelist.xml from the system folder).
    if matches!(plan.preset, EsDe | Batocera) {
        let mut by_folder: BTreeMap<PathBuf, Vec<gamelist::GamelistEntry>> = BTreeMap::new();
        for (display, dest, title, _region, _platform) in &targets {
            let Some(parent) = display.parent() else {
                continue;
            };
            let Some(file_name) = display.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let image = if dest.exists() {
                let image_name = dest
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default()
                    .to_string();
                Some(format!("./media/images/{}", image_name))
            } else {
                None
            };
            by_folder.entry(parent.to_path_buf()).or_default().push(gamelist::GamelistEntry {
                path: format!("./{}", file_name),
                name: title.clone(),
                image,
            });
        }

        for (folder, entries) in by_folder {
            let gamelist_path = folder.join("gamelist.xml");
            gamelist::write_gamelist(&gamelist_path, &entries)
                .map_err(|e| format!("Cannot write gamelist '{}': {}", gamelist_path.display(), e))?;
            summary.gamelists_written += 1;
        }
    }

    Ok(summary)
}
