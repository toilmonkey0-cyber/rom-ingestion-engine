use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::models::{FrontendPreset, Platform};
use crate::organizer::m3u::{generate_m3u_content, parse_m3u_content};
use crate::organizer::presets::{
    get_multidisc_subfolder, get_platform_folder, get_platform_folder_with_custom,
    sanitize_relative_folder, CustomPresetConfig,
};

/// A single file move (or rewrite) planned by the migrator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationItemKind {
    Chd,
    Playlist,
    Artwork,
    /// Stale `gamelist.xml` left behind in the old layout.
    StaleGamelist,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationItem {
    pub kind: MigrationItemKind,
    pub source: PathBuf,
    pub target: PathBuf,
}

/// A playlist whose entries must be rewritten for the new layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaylistRewrite {
    pub target: PathBuf,
    /// Forward-slash entries relative to the playlist location.
    pub entries: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MigrationPlan {
    pub root: PathBuf,
    pub source_preset: FrontendPreset,
    pub target_preset: FrontendPreset,
    pub games: usize,
    pub items: Vec<MigrationItem>,
    pub playlist_rewrites: Vec<PlaylistRewrite>,
    /// Playlists referencing disc files that do not exist on disk. They are
    /// migrated as-is (or reported) but cannot be repaired by moving files —
    /// surfaced so the dry run tells the truth about library health.
    #[serde(default)]
    pub broken_playlists: Vec<BrokenPlaylist>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrokenPlaylist {
    pub playlist: PathBuf,
    pub missing_entries: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationSummary {
    pub files_moved: usize,
    pub playlists_rewritten: usize,
    pub gamelists_written: usize,
    /// Files skipped because the target already existed (left untouched).
    pub skipped_existing: Vec<String>,
}

/// Case-insensitive, forward-slash comparison key for a relative path.
fn rel_key(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .unwrap_or(p)
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase()
}

/// Maps a file's parent folder back to a platform using the source preset's
/// layout (including nested custom folders like `SD/roms/psx`).
fn platform_for_folder(root: &Path, folder: &Path, preset: FrontendPreset, custom: Option<&CustomPresetConfig>) -> Platform {
    let key = rel_key(root, folder);
    let candidates = [
        Platform::Psx,
        Platform::Saturn,
        Platform::Dreamcast,
        Platform::SegaCd,
        Platform::PceCd,
        Platform::Unknown,
    ];
    for platform in candidates {
        let folder_str = if preset == FrontendPreset::Custom && custom.is_some() {
            sanitize_relative_folder(&get_platform_folder_with_custom(preset, platform, custom))
        } else {
            get_platform_folder(preset, platform).replace('\\', "/")
        };
        let folder_lower = folder_str.to_ascii_lowercase();
        if key == folder_lower || key.ends_with(&format!("/{}", folder_lower)) {
            return platform;
        }
    }
    Platform::Unknown
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, out)?;
        } else if path.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

/// Plans a library re-organization from one frontend preset to another.
///
/// Migration only moves and renames files — CHDs are frontend-agnostic, so
/// nothing is re-converted. Playlists are rewritten when the multi-disc
/// subfolder changes, artwork is relocated per the target preset's
/// conventions, and `gamelist.xml` metadata is regenerated for
/// EmulationStation-derived frontends.
pub fn plan_migration(
    root: &Path,
    source_preset: FrontendPreset,
    target_preset: FrontendPreset,
    custom: Option<&CustomPresetConfig>,
) -> Result<MigrationPlan, String> {
    if source_preset == target_preset {
        return Err("Source and target presets are identical — nothing to migrate.".to_string());
    }
    if !root.is_dir() {
        return Err(format!("Library root is not a directory: {}", root.display()));
    }

    let mut all_files = Vec::new();
    collect_files(root, &mut all_files)
        .map_err(|e| format!("Failed to scan '{}': {}", root.display(), e))?;

    let source_subfolder = if source_preset == FrontendPreset::Custom && custom.is_some() {
        crate::organizer::presets::sanitize_component(
            custom.map(|c| c.get_multidisc_subfolder()).unwrap_or(".discs"),
            ".discs",
        )
    } else {
        get_multidisc_subfolder(source_preset).to_string()
    };
    let target_subfolder = if target_preset == FrontendPreset::Custom && custom.is_some() {
        crate::organizer::presets::sanitize_component(
            custom.map(|c| c.get_multidisc_subfolder()).unwrap_or(".discs"),
            ".discs",
        )
    } else {
        get_multidisc_subfolder(target_preset).to_string()
    };

    let mut items: Vec<MigrationItem> = Vec::new();
    let mut rewrites: Vec<PlaylistRewrite> = Vec::new();
    let mut broken: Vec<BrokenPlaylist> = Vec::new();
    let mut games = 0usize;
    let mut referenced_chds: HashSet<String> = HashSet::new();
    let mut migrated_stems: Vec<(PathBuf /* old platform folder */, String /* stem */, bool /* multidisc */, PathBuf /* new m3u or chd target */)> =
        Vec::new();

    let m3u_files: Vec<PathBuf> = all_files
        .iter()
        .filter(|p| p.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("m3u")).unwrap_or(false))
        .cloned()
        .collect();

    // 1. Multi-disc games (playlists + their referenced CHDs + art)
    for m3u in &m3u_files {
        let platform_folder = match m3u.parent() {
            Some(p) => p.to_path_buf(),
            None => continue,
        };
        let platform = platform_for_folder(root, &platform_folder, source_preset, custom);
        let content = match std::fs::read_to_string(m3u) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let entries = parse_m3u_content(&content);
        if entries.is_empty() {
            continue;
        }

        let target_platform_folder = get_platform_folder_with_custom(target_preset, platform, custom);
        let stem = m3u.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
        games += 1;

        // Move each referenced CHD into the new layout. Playlist entries may
        // be relative to the playlist, or root-relative with a leading slash
        // (real-world cards use both); try playlist-relative first, then root.
        let mut missing: Vec<String> = Vec::new();
        for entry in &entries {
            let rel = entry.trim_start_matches('/');
            let rel = rel.replace('/', std::path::MAIN_SEPARATOR.to_string().as_str());
            let old_chd = if platform_folder.join(&rel).is_file() {
                platform_folder.join(&rel)
            } else {
                root.join(&rel)
            };
            if !old_chd.is_file() {
                missing.push(entry.clone());
                continue;
            }
            referenced_chds.insert(rel_key(root, &old_chd));
            let chd_name = old_chd.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
            let new_chd = PathBuf::from(format!(
                "{}/{}/{}/{}",
                root.to_string_lossy().replace('\\', "/"),
                sanitize_relative_folder(&target_platform_folder),
                target_subfolder,
                chd_name
            ));
            if new_chd != old_chd {
                items.push(MigrationItem { kind: MigrationItemKind::Chd, source: old_chd.clone(), target: new_chd });
            }
        }

        if !missing.is_empty() {
            broken.push(BrokenPlaylist {
                playlist: m3u.clone(),
                missing_entries: missing,
            });
        }

        // Move the playlist itself.
        let m3u_name = m3u.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
        let new_m3u = PathBuf::from(format!(
            "{}/{}/{}",
            root.to_string_lossy().replace('\\', "/"),
            sanitize_relative_folder(&target_platform_folder),
            m3u_name
        ));
        if new_m3u != *m3u {
            items.push(MigrationItem { kind: MigrationItemKind::Playlist, source: m3u.clone(), target: new_m3u.clone() });
        }

        // Rewrite entries when the subfolder changed.
        let new_entries: Vec<String> = entries
            .iter()
            .map(|e| {
                let name = e.rsplit('/').next().unwrap_or(e);
                format!("{}/{}", target_subfolder, name)
            })
            .collect();
        if new_entries != entries {
            rewrites.push(PlaylistRewrite { target: new_m3u.clone(), entries: new_entries });
        }

        migrated_stems.push((platform_folder.clone(), stem, true, new_m3u));
    }

    // 2. Standalone CHDs (single-disc games not referenced by any playlist)
    for chd in &all_files {
        let ext = chd.extension().and_then(|e| e.to_str()).unwrap_or_default();
        if !ext.eq_ignore_ascii_case("chd") || referenced_chds.contains(&rel_key(root, chd)) {
            continue;
        }
        let parent = match chd.parent() {
            Some(p) => p.to_path_buf(),
            None => continue,
        };
        // Orphans inside the multi-disc subfolder resolve via the platform
        // folder above it; anything else uses its own folder.
        let platform_folder = if rel_key(root, &parent)
            .rsplit('/')
            .next()
            .map(|last| last.eq_ignore_ascii_case(&source_subfolder))
            .unwrap_or(false)
        {
            parent.parent().map(|p| p.to_path_buf()).unwrap_or(parent.clone())
        } else {
            parent.clone()
        };
        let platform = platform_for_folder(root, &platform_folder, source_preset, custom);
        let target_platform_folder = get_platform_folder_with_custom(target_preset, platform, custom);
        let stem = chd.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
        games += 1;

        let chd_name = chd.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
        let new_chd = PathBuf::from(format!(
            "{}/{}/{}",
            root.to_string_lossy().replace('\\', "/"),
            sanitize_relative_folder(&target_platform_folder),
            chd_name
        ));
        if new_chd != *chd {
            items.push(MigrationItem { kind: MigrationItemKind::Chd, source: chd.clone(), target: new_chd.clone() });
        }
        migrated_stems.push((platform_folder.clone(), stem, false, new_chd));
    }

    // 3. Artwork + stale gamelists
    for (old_platform_folder, stem, _multidisc, new_display) in &migrated_stems {
        let platform = platform_for_folder(root, old_platform_folder, source_preset, custom);
        let new_platform_folder = PathBuf::from(format!(
            "{}/{}",
            root.to_string_lossy().replace('\\', "/"),
            sanitize_relative_folder(&get_platform_folder_with_custom(target_preset, platform, custom))
        ));
        let old_adjacent = old_platform_folder.join(format!("{}.png", stem));
        let old_media = old_platform_folder.join("media").join("images").join(format!("{}.png", stem));
        let source_art = if old_media.is_file() {
            old_media
        } else if old_adjacent.is_file() {
            old_adjacent
        } else {
            PathBuf::new()
        };

        if !source_art.as_os_str().is_empty() {
            let new_art = match target_preset {
                FrontendPreset::EsDe | FrontendPreset::Batocera => new_platform_folder
                    .join("media")
                    .join("images")
                    .join(format!("{}.png", stem)),
                _ => new_display.parent().unwrap_or(&new_platform_folder).join(format!("{}.png", stem)),
            };
            if new_art != source_art {
                items.push(MigrationItem { kind: MigrationItemKind::Artwork, source: source_art, target: new_art });
            }
        }

        let old_gamelist = old_platform_folder.join("gamelist.xml");
        if old_gamelist.is_file() && new_platform_folder != *old_platform_folder {
            items.push(MigrationItem {
                kind: MigrationItemKind::StaleGamelist,
                source: old_gamelist,
                target: new_platform_folder.join("gamelist.xml"),
            });
        }
    }

    items.dedup_by(|a, b| a.source == b.source && a.target == b.target);

    Ok(MigrationPlan {
        root: root.to_path_buf(),
        source_preset,
        target_preset,
        games,
        items,
        playlist_rewrites: rewrites,
        broken_playlists: broken,
    })
}

/// Moves a file, falling back to copy+remove when a rename would cross
/// devices (e.g. library spanning two mounts).
fn move_file(source: &Path, target: &Path) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create '{}': {}", parent.display(), e))?;
    }
    match std::fs::rename(source, target) {
        Ok(()) => Ok(()),
        Err(_) => {
            std::fs::copy(source, target)
                .map_err(|e| format!("Cannot copy '{}' -> '{}': {}", source.display(), target.display(), e))?;
            std::fs::remove_file(source)
                .map_err(|e| format!("Copied but cannot remove source '{}': {}", source.display(), e))?;
            Ok(())
        }
    }
}

/// Executes a migration plan: moves files, rewrites playlists, and writes
/// fresh `gamelist.xml` metadata for EmulationStation-derived targets.
pub fn execute_migration<E: crate::commands::EventSink>(
    emitter: &E,
    plan: &MigrationPlan,
) -> Result<MigrationSummary, String> {
    let mut summary = MigrationSummary {
        files_moved: 0,
        playlists_rewritten: 0,
        gamelists_written: 0,
        skipped_existing: Vec::new(),
    };

    let total = plan.items.len();
    for (idx, item) in plan.items.iter().enumerate() {
        if !item.source.exists() {
            continue; // already moved in an earlier run
        }
        if item.target.exists() && item.target != item.source {
            summary.skipped_existing.push(item.target.to_string_lossy().to_string());
            continue;
        }
        match item.kind {
            MigrationItemKind::StaleGamelist => {
                // The old layout's metadata is wrong for the new layout —
                // remove it; a fresh one is written below when needed.
                std::fs::remove_file(&item.source)
                    .map_err(|e| format!("Cannot remove stale gamelist '{}': {}", item.source.display(), e))?;
            }
            _ => {
                move_file(&item.source, &item.target)?;
                summary.files_moved += 1;
            }
        }
        emitter.emit_migration_progress(&crate::models::MigrationProgressEvent {
            message: format!("Moved {}", item.source.to_string_lossy()),
            completed: idx + 1,
            total,
        });
    }

    // Playlist rewrites happen at their (possibly new) locations.
    for rewrite in &plan.playlist_rewrites {
        if rewrite.target.exists() {
            std::fs::write(&rewrite.target, generate_m3u_content(&rewrite.entries).as_bytes())
                .map_err(|e| format!("Cannot rewrite playlist '{}': {}", rewrite.target.display(), e))?;
            summary.playlists_rewritten += 1;
        }
    }

    // Fresh gamelist.xml for EmulationStation-derived targets, built from the
    // final file locations.
    if matches!(plan.target_preset, FrontendPreset::EsDe | FrontendPreset::Batocera) {
        use std::collections::BTreeMap;
        let mut by_folder: BTreeMap<PathBuf, Vec<crate::metadata::gamelist::GamelistEntry>> = BTreeMap::new();

        for item in &plan.items {
            if item.kind != MigrationItemKind::Playlist {
                continue;
            }
            let Some(parent) = item.target.parent() else { continue };
            let Some(name) = item.target.file_name().and_then(|n| n.to_str()) else { continue };
            let stem = item.target.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
            let art = parent.join("media").join("images").join(format!("{}.png", stem));
            by_folder.entry(parent.to_path_buf()).or_default().push(crate::metadata::gamelist::GamelistEntry {
                path: format!("./{}", name),
                name: stem.to_string(),
                image: if art.is_file() {
                    Some(format!("./media/images/{}.png", stem))
                } else {
                    None
                },
                desc: None,
            });
        }
        // Standalone CHDs that moved
        let discs_folder = get_multidisc_subfolder(plan.target_preset);
        for item in &plan.items {
            if item.kind != MigrationItemKind::Chd {
                continue;
            }
            let in_discs = item
                .target
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .map(|n| n.eq_ignore_ascii_case(discs_folder))
                .unwrap_or(false);
            if in_discs {
                continue; // belongs to a playlist entry
            }
            let Some(parent) = item.target.parent() else { continue };
            let Some(name) = item.target.file_name().and_then(|n| n.to_str()) else { continue };
            let stem = item.target.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
            let art = parent.join("media").join("images").join(format!("{}.png", stem));
            by_folder.entry(parent.to_path_buf()).or_default().push(crate::metadata::gamelist::GamelistEntry {
                path: format!("./{}", name),
                name: stem.to_string(),
                image: if art.is_file() {
                    Some(format!("./media/images/{}.png", stem))
                } else {
                    None
                },
                desc: None,
            });
        }

        for (folder, entries) in by_folder {
            if entries.is_empty() {
                continue;
            }
            let path = folder.join("gamelist.xml");
            crate::metadata::gamelist::write_gamelist(&path, &entries)
                .map_err(|e| format!("Cannot write gamelist '{}': {}", path.display(), e))?;
            summary.gamelists_written += 1;
        }
    }

    Ok(summary)
}
