use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::models::{FrontendPreset, MediaOptions, PlannedGame, Platform};
use crate::organizer::media::resolve_media_paths_for_game;
use crate::organizer::presets::resolve_target_paths;
use crate::paths::is_lexically_within;

pub const LEDGER_FILE: &str = "ingestion-ledger.jsonl";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedgerRecord {
    pub source_path: String,
    pub source_bytes: u64,
    pub source_sha1: String,
    pub serial: Option<String>,
    pub chd_path: String,
    pub chdman_version: String,
    pub command: String,
    pub result: String,
}

pub fn ledger_path(output_dir: &Path) -> PathBuf {
    output_dir.join(LEDGER_FILE)
}

pub fn load_ledger(output_dir: &Path) -> Vec<LedgerRecord> {
    let path = ledger_path(output_dir);
    let Ok(file) = File::open(path) else {
        return Vec::new();
    };
    BufReader::new(file)
        .lines()
        .filter_map(|line| line.ok())
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str(&line).ok())
        .collect()
}

pub fn append_ledger(output_dir: &Path, record: &LedgerRecord) -> Result<(), String> {
    if let Some(parent) = output_dir.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(output_dir).map_err(|e| e.to_string())?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(ledger_path(output_dir))
        .map_err(|e| e.to_string())?;
    let line = serde_json::to_string(record).map_err(|e| e.to_string())?;
    writeln!(file, "{line}").map_err(|e| e.to_string())
}

/// A prior successful record for the same source bytes means this dump is already in the library.
pub fn ledger_hit<'a>(records: &'a [LedgerRecord], source_sha1: &str) -> Option<&'a LedgerRecord> {
    let sha = source_sha1.trim().to_ascii_lowercase();
    if sha.is_empty() {
        return None;
    }
    records.iter().rev().find(|record| {
        record.source_sha1.eq_ignore_ascii_case(&sha) && record.result == "ok"
    })
}

pub fn apply_dat_choice(
    game: &mut PlannedGame,
    title: &str,
    region: &str,
    platform: Platform,
    disc_number: Option<u8>,
    output_dir: &Path,
    preset: FrontendPreset,
) {
    game.canonical_title = title.to_string();
    game.region = region.to_string();
    game.platform = platform;
    game.source = crate::models::ClassificationSource::RedumpCache;
    game.confidence = 1.0;
    game.needs_review = false;
    game.enabled = true;
    game.status_note = None;
    if let Some(number) = disc_number {
        if let Some(disc) = game.discs.get_mut(0) {
            disc.disc_number = number;
        }
    }
    rewrite_output_paths(game, output_dir, preset, &MediaOptions::default());
}

/// A dry-run title edit keeps the CHD, M3U, and PNG stems that execution will write.
pub fn apply_title_edit(
    game: &mut PlannedGame,
    title: &str,
    output_dir: &Path,
    preset: FrontendPreset,
    media_options: &MediaOptions,
) {
    game.canonical_title = title.to_string();
    rewrite_output_paths(game, output_dir, preset, media_options);
}

fn rewrite_output_paths(
    game: &mut PlannedGame,
    output_dir: &Path,
    preset: FrontendPreset,
    media_options: &MediaOptions,
) {
    let title = game.canonical_title.clone();
    let region = game.region.clone();
    let platform = game.platform;
    let count = game.discs.len().max(1) as u8;
    let multi = game.discs.len() > 1;
    game.is_multidisc = multi;
    let mut m3u = None;
    for disc in &mut game.discs {
        let paths = resolve_target_paths(
            output_dir,
            preset,
            platform,
            &title,
            &region,
            multi,
            Some(disc.disc_number),
            Some(count),
        );
        if is_lexically_within(output_dir, &paths.chd_path) {
            disc.target_chd_path = paths.chd_path;
            if m3u.is_none() {
                m3u = paths.m3u_path;
            }
        }
    }
    game.target_m3u_path = if multi { m3u } else { None };
    game.target_media_paths = resolve_media_paths_for_game(
        output_dir,
        preset,
        platform,
        &title,
        &region,
        media_options,
    );
}

/// Region priority picks one keeper. Other regions of the same title stay visible and disabled.
pub fn apply_region_priority(games: &mut [PlannedGame], priority: &[String]) {
    if priority.is_empty() {
        return;
    }
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (index, game) in games.iter().enumerate() {
        let key_title = game.canonical_title.trim().to_ascii_lowercase();
        let edition = game
            .discs
            .first()
            .map(|disc| {
                crate::classifier::redump::extract_edition_tag(
                    &disc.source_descriptor.to_string_lossy(),
                )
            })
            .unwrap_or_default();
        if let Some(group) = groups.iter_mut().find(|group| {
            let other = &games[group[0]];
            let other_edition = other
                .discs
                .first()
                .map(|disc| {
                    crate::classifier::redump::extract_edition_tag(
                        &disc.source_descriptor.to_string_lossy(),
                    )
                })
                .unwrap_or_default();
            other.platform == game.platform
                && other.canonical_title.trim().eq_ignore_ascii_case(&key_title)
                && other_edition == edition
        }) {
            group.push(index);
        } else {
            groups.push(vec![index]);
        }
    }
    for group in groups {
        if group.len() < 2 {
            games[group[0]].role = "keeper".to_string();
            continue;
        }
        let keeper = priority.iter().find_map(|wanted| {
            group.iter().copied().find(|idx| {
                games[*idx].region.eq_ignore_ascii_case(wanted)
            })
        });
        let Some(keeper) = keeper else {
            continue;
        };
        for idx in group {
            if idx == keeper {
                games[idx].role = "keeper".to_string();
                games[idx].enabled = true;
            } else {
                games[idx].role = "alternate".to_string();
                games[idx].enabled = false;
            }
        }
    }
}

pub fn propose_relative_cue(cue_text: &str, cue_dir: &Path) -> Option<String> {
    let mut changed = false;
    let mut lines = Vec::new();
    for line in cue_text.lines() {
        let trimmed = line.trim_start();
        if !trimmed.to_ascii_uppercase().starts_with("FILE ") {
            lines.push(line.to_string());
            continue;
        }
        let Some(name) = file_token(trimmed) else {
            lines.push(line.to_string());
            continue;
        };
        let path = Path::new(&name);
        let base = path.file_name()?.to_string_lossy().to_string();
        let sibling = cue_dir.join(&base);
        let absolute = path.is_absolute();
        let named_wrong = !sibling.exists() && path.exists();
        if sibling.exists() && (absolute || name != base) {
            let rewritten = rewrite_file_line(line, &name, &base);
            lines.push(rewritten);
            changed = true;
        } else if named_wrong {
            lines.push(line.to_string());
        } else {
            lines.push(line.to_string());
        }
    }
    if changed {
        Some(lines.join("\n"))
    } else {
        None
    }
}

fn file_token(line: &str) -> Option<String> {
    let rest = line.split_once("FILE").map(|(_, rest)| rest.trim())?;
    if let Some(quoted) = rest.strip_prefix('"') {
        quoted.split('"').next().map(|s| s.to_string())
    } else if let Some(quoted) = rest.strip_prefix('\'') {
        quoted.split('\'').next().map(|s| s.to_string())
    } else {
        rest.split_whitespace().next().map(|s| s.to_string())
    }
}

fn rewrite_file_line(line: &str, old: &str, new_name: &str) -> String {
    if line.contains(&format!("\"{old}\"")) {
        line.replace(&format!("\"{old}\""), &format!("\"{new_name}\""))
    } else {
        line.replace(old, &format!("\"{new_name}\""))
    }
}

pub fn directory_bytes(root: &Path) -> u64 {
    let mut total = 0u64;
    let Ok(entries) = fs::read_dir(root) else {
        return 0;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            total += directory_bytes(&path);
        } else if let Ok(meta) = entry.metadata() {
            total += meta.len();
        }
    }
    total
}

/// Copies `source` onto `dest` only when `available_bytes` covers the tree.
/// `available_bytes: None` asks the operating system. A short budget writes nothing.
pub fn deploy_library(source: &Path, dest: &Path, available_bytes: Option<u64>) -> Result<Vec<String>, String> {
    if !source.is_dir() {
        return Err(format!("library folder is missing: {}", source.display()));
    }
    let needed = directory_bytes(source);
    let available = match available_bytes {
        Some(value) => value,
        None => free_bytes(dest.ancestors().find(|path| path.exists()).unwrap_or(dest))?,
    };
    if available < needed {
        return Err(format!(
            "not enough free space: need {needed} bytes, {available} available"
        ));
    }
    if dest.exists() {
        return Err(format!("destination already exists: {}", dest.display()));
    }
    copy_tree(source, dest)?;
    let mut names = Vec::new();
    collect_relative_names(dest, dest, &mut names);
    names.sort();
    Ok(names)
}

fn copy_tree(source: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if from.is_dir() {
            copy_tree(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn collect_relative_names(root: &Path, dir: &Path, names: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_relative_names(root, &path, names);
        } else if let Ok(rel) = path.strip_prefix(root) {
            names.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

fn free_bytes(path: &Path) -> Result<u64, String> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        extern "system" {
            fn GetDiskFreeSpaceExW(
                lp_directory_name: *const u16,
                lp_free_bytes_available: *mut u64,
                lp_total_number_of_bytes: *mut u64,
                lp_total_number_of_free_bytes: *mut u64,
            ) -> i32;
        }
        let wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let mut free = 0u64;
        let ok = unsafe {
            GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, std::ptr::null_mut(), std::ptr::null_mut())
        };
        if ok == 0 {
            return Err("could not read free space".to_string());
        }
        return Ok(free);
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Ok(u64::MAX)
    }
}

/// Names recorded in the ledger, relative to the output directory, that exist after a copy.
pub fn ledger_output_names(output_dir: &Path, records: &[LedgerRecord]) -> Vec<String> {
    records
        .iter()
        .filter(|record| record.result == "ok")
        .filter_map(|record| {
            Path::new(&record.chd_path)
                .strip_prefix(output_dir)
                .ok()
                .map(|rel| rel.to_string_lossy().replace('\\', "/"))
        })
        .collect()
}
