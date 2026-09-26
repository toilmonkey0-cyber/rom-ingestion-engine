// src-tauri/src/plan_builder.rs
use std::collections::HashSet;
use std::path::PathBuf;

use crate::models::{
    ClassificationSource, DiscFingerprint, FrontendPreset, GameClassification, IngestionPlan,
    MediaOptions, Platform, PlannedDisc, PlannedGame, TaskStatus,
};
use crate::organizer::media::resolve_media_paths_for_game;
use crate::organizer::presets::resolve_target_paths;

/// Generates a clean, deterministic game ID for a planned game.
fn generate_game_id(platform: Platform, title: &str, index: usize) -> String {
    let plat_str = match platform {
        Platform::Psx => "psx",
        Platform::Saturn => "saturn",
        Platform::Dreamcast => "dreamcast",
        Platform::SegaCd => "segacd",
        Platform::PceCd => "pcecd",
        Platform::Unknown => "unknown",
    };
    let slug: String = title
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let cleaned_slug = slug
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    if cleaned_slug.is_empty() {
        format!("{}-game-{}", plat_str, index + 1)
    } else {
        format!("{}-{}", plat_str, cleaned_slug)
    }
}

/// Internal temporary representation of a disc during planning.
#[derive(Clone)]
struct IntermediateDisc {
    disc_number: u8,
    total_discs: Option<u8>,
    source_descriptor: PathBuf,
    binary_tracks: Vec<PathBuf>,
}

/// Builds an `IngestionPlan` from classified disc fingerprints with default media options (boxart enabled).
pub fn build_ingestion_plan(
    input_dir: PathBuf,
    output_dir: PathBuf,
    preset: FrontendPreset,
    items: Vec<(DiscFingerprint, GameClassification)>,
) -> IngestionPlan {
    build_ingestion_plan_with_options(
        input_dir,
        output_dir,
        preset,
        items,
        &MediaOptions::default(),
    )
}

/// Builds an `IngestionPlan` from classified disc fingerprints with specific `MediaOptions`.
///
/// Multi-disc items of the same canonical title and platform are merged into
/// single `PlannedGame` instances, sorted by disc number, with target `.chd` paths
/// and optional `.m3u` playlists determined according to the target frontend preset.
/// Target media paths (boxart, screenshots, titles) are populated according to `media_options`.
pub fn build_ingestion_plan_with_options(
    input_dir: PathBuf,
    output_dir: PathBuf,
    preset: FrontendPreset,
    items: Vec<(DiscFingerprint, GameClassification)>,
    media_options: &MediaOptions,
) -> IngestionPlan {
    let mut planned_games = Vec::new();
    let mut used_ids = HashSet::new();
    let mut total_source_bytes: u64 = 0;
    let mut ok_items = Vec::new();

    for (fingerprint, classification) in items {
        if fingerprint.scan_error.is_some() {
            total_source_bytes += fingerprint.total_bytes;
            let base_id = generate_game_id(classification.platform, &classification.canonical_title, planned_games.len());
            let mut game_id = base_id.clone();
            let mut counter = 2;
            while !used_ids.insert(game_id.clone()) {
                game_id = format!("{}-{}", base_id, counter);
                counter += 1;
            }
            let command = crate::paths::chdman_command_for_input(&fingerprint.primary_file)
                .unwrap_or("createcd")
                .to_string();
            let paths = resolve_target_paths(
                &output_dir,
                preset,
                classification.platform,
                &classification.canonical_title,
                &classification.region,
                false,
                Some(1),
                Some(1),
            );
            planned_games.push(PlannedGame {
                id: game_id,
                canonical_title: classification.canonical_title.clone(),
                platform: classification.platform,
                region: classification.region.clone(),
                is_multidisc: false,
                discs: vec![PlannedDisc {
                    disc_number: 1,
                    source_descriptor: fingerprint.primary_file.clone(),
                    target_chd_path: paths.chd_path,
                    status: TaskStatus::Failed,
                    binary_tracks: fingerprint.binary_tracks.clone(),
                    chdman_command: command,
                }],
                target_m3u_path: None,
                confidence: classification.confidence,
                source: classification.source,
                enabled: false,
                needs_review: true,
                status_note: None,
        role: String::new(),
        artwork_url: None,
                target_media_paths: Vec::new(),
            });
        } else {
            ok_items.push((fingerprint, classification));
        }
    }

    // Group playable discs by release identity. Broken descriptors stay out of this map.
    let mut groups: Vec<((Platform, String, String, String), Vec<(DiscFingerprint, GameClassification)>)> =
        Vec::new();

    for (fingerprint, classification) in ok_items {
        let edition = crate::classifier::redump::extract_edition_tag(
            &fingerprint.primary_file.to_string_lossy(),
        );
        let key = (
            classification.platform,
            classification.canonical_title.trim().to_lowercase(),
            classification.region.trim().to_lowercase(),
            edition,
        );
        if let Some((_, group)) = groups.iter_mut().find(|(k, _)| *k == key) {
            group.push((fingerprint, classification));
        } else {
            groups.push((key, vec![(fingerprint, classification)]));
        }
    }

    // 2. Build planned games
    for (group_idx, ((platform, _, _, _), group)) in groups.into_iter().enumerate() {
        let is_multidisc = group.len() > 1 || group.iter().any(|(_, c)| c.is_multidisc);

        // Pick best classification for canonical metadata
        let best_class = group
            .iter()
            .max_by(|a, b| {
                a.1.confidence
                    .partial_cmp(&b.1.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(_, c)| c)
            .expect("group is non-empty");

        let canonical_title = best_class.canonical_title.clone();
        let region = best_class.region.clone();

        // Calculate confidence and review flag
        let total_conf: f32 = group.iter().map(|(_, c)| c.confidence).sum();
        let avg_confidence = if !group.is_empty() {
            total_conf / group.len() as f32
        } else {
            1.0
        };
        let needs_review =
            avg_confidence < 0.80 || group.iter().any(|(_, c)| c.confidence < 0.80);

        // Determine aggregated classification source
        let all_same_source = group.iter().all(|(_, c)| c.source == group[0].1.source);
        let source = if all_same_source {
            group[0].1.source
        } else if group.iter().any(|(_, c)| c.source == ClassificationSource::JevAI) {
            ClassificationSource::JevAI
        } else if group
            .iter()
            .any(|(_, c)| c.source == ClassificationSource::RedumpCache)
        {
            ClassificationSource::RedumpCache
        } else {
            ClassificationSource::Fallback
        };

        // Collect and sort discs
        let mut temp_discs = Vec::new();
        for (fp, class) in &group {
            total_source_bytes += fp.total_bytes;

            let disc_num = if let Some(d) = class.disc_number {
                d
            } else if let Some((Some(d), _)) = Some(crate::classifier::redump::extract_disc_info(
                &fp.primary_file.to_string_lossy(),
            )) {
                d
            } else if !is_multidisc {
                1
            } else {
                0
            };

            temp_discs.push(IntermediateDisc {
                disc_number: disc_num,
                total_discs: class.total_discs,
                source_descriptor: fp.primary_file.clone(),
                binary_tracks: fp.binary_tracks.clone(),
            });
        }

        // Sort discs by disc_number ascending, tiebreaker on descriptor path
        temp_discs.sort_by(|a, b| {
            a.disc_number
                .cmp(&b.disc_number)
                .then_with(|| a.source_descriptor.cmp(&b.source_descriptor))
        });

        // Missing or duplicate disc numbers are a different release or a second copy.
        // Leave them as separate games. Do not invent a 1..N order.
        let has_zero = temp_discs.iter().any(|d| d.disc_number == 0);
        let mut disc_num_set = HashSet::new();
        let has_duplicates = temp_discs.iter().any(|d| !disc_num_set.insert(d.disc_number));
        let split_ambiguous = temp_discs.len() > 1 && (has_zero || has_duplicates);

        let releases: Vec<Vec<IntermediateDisc>> = if split_ambiguous {
            temp_discs
                .into_iter()
                .map(|mut disc| {
                    if disc.disc_number == 0 {
                        disc.disc_number = 1;
                    }
                    vec![disc]
                })
                .collect()
        } else {
            vec![temp_discs]
        };

        for release in releases {
            let release_is_multidisc = !split_ambiguous && is_multidisc && release.len() > 1;
            let release_needs_review = needs_review || split_ambiguous;
            let total_discs = Some(release.len() as u8);
            let mut planned_discs = Vec::new();
            let mut target_m3u_path = None;

            for disc in &release {
                let target_paths = resolve_target_paths(
                    &output_dir,
                    preset,
                    platform,
                    &canonical_title,
                    &region,
                    release_is_multidisc,
                    Some(disc.disc_number),
                    disc.total_discs.or(total_discs),
                );

                if release_is_multidisc && target_m3u_path.is_none() {
                    target_m3u_path = target_paths.m3u_path;
                }

                let chdman_command = crate::paths::chdman_command_for_input(&disc.source_descriptor)
                    .unwrap_or("createcd")
                    .to_string();
                planned_discs.push(PlannedDisc {
                    disc_number: disc.disc_number,
                    source_descriptor: disc.source_descriptor.clone(),
                    target_chd_path: target_paths.chd_path,
                    status: TaskStatus::Pending,
                    binary_tracks: disc.binary_tracks.clone(),
                    chdman_command,
                });
            }

            let base_id = generate_game_id(platform, &canonical_title, group_idx);
            let mut game_id = base_id.clone();
            let mut counter = 2;
            while !used_ids.insert(game_id.clone()) {
                game_id = format!("{}-{}", base_id, counter);
                counter += 1;
            }

            let target_media_paths = resolve_media_paths_for_game(
                &output_dir,
                preset,
                platform,
                &canonical_title,
                &region,
                media_options,
            );

            planned_games.push(PlannedGame {
                id: game_id,
                canonical_title: canonical_title.clone(),
                platform,
                region: region.clone(),
                is_multidisc: release_is_multidisc,
                discs: planned_discs,
                target_m3u_path,
                confidence: avg_confidence,
                source,
                enabled: !(release_needs_review && source == ClassificationSource::Fallback),
                needs_review: release_needs_review,
                status_note: None,
        role: String::new(),
        artwork_url: None,
                target_media_paths,
            });
        }
    }

    // Estimated output bytes: approximately 60% of source size for CD/GD-ROM compression
    let estimated_output_bytes = (total_source_bytes as f64 * 0.60).round() as u64;

    IngestionPlan {
        input_dir,
        output_dir,
        preset,
        games: planned_games,
        total_source_bytes,
        estimated_output_bytes,
    }
}
