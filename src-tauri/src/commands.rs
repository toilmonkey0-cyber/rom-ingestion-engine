// src-tauri/src/commands.rs
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::Emitter;

pub use crate::chdman::downloader::{
    detect_chdman, download_and_install_chdman, ChdmanSource, ChdmanStatus, DownloadProgressEvent,
};
use crate::chdman::runner::ChdmanRunner;
use crate::classifier::jev::JevClient;
use crate::classifier::redump::RedumpDatabase;
use crate::models::{
    ArtworkProgressEvent, ClassificationSource, DiscFingerprint, ExecutionSummary, FrontendPreset,
    GameClassification, GameStatusEvent, IngestionPlan, JobProgressEvent, MediaOptions,
    MigrationProgressEvent, PlannedGame, Platform, SkippedSource, TaskStatus, TrashOutcome,
};
use crate::organizer::presets::CustomPresetConfig;
use crate::plan_builder::build_ingestion_plan_with_options;
use crate::scanner::is_under_root;

/// Abstraction for emitting progress and status events to the frontend or test listener.
pub trait EventSink: Send + Sync {
    fn emit_job_progress(&self, _event: &JobProgressEvent) {}
    fn emit_game_status(&self, _event: &GameStatusEvent) {}
    fn emit_download_progress(&self, _event: &DownloadProgressEvent) {}
    fn emit_artwork_progress(&self, _event: &ArtworkProgressEvent) {}
    fn emit_migration_progress(&self, _event: &MigrationProgressEvent) {}
}

impl EventSink for () {}

impl EventSink for tauri::AppHandle {
    fn emit_job_progress(&self, event: &JobProgressEvent) {
        let _ = self.emit("job-progress", event);
    }

    fn emit_game_status(&self, event: &GameStatusEvent) {
        let _ = self.emit("game-status", event);
    }

    fn emit_download_progress(&self, event: &DownloadProgressEvent) {
        let _ = self.emit("chdman-download-progress", event);
    }

    fn emit_artwork_progress(&self, event: &ArtworkProgressEvent) {
        let _ = self.emit("artwork-progress", event);
    }

    fn emit_migration_progress(&self, event: &MigrationProgressEvent) {
        let _ = self.emit("migration-progress", event);
    }
}

/// Mock event sink for testing and headless verification.
#[derive(Debug, Clone, Default)]
pub struct MockEventSink {
    pub progress_events: Arc<Mutex<Vec<JobProgressEvent>>>,
    pub status_events: Arc<Mutex<Vec<GameStatusEvent>>>,
    pub download_events: Arc<Mutex<Vec<DownloadProgressEvent>>>,
    pub artwork_events: Arc<Mutex<Vec<ArtworkProgressEvent>>>,
    pub migration_events: Arc<Mutex<Vec<MigrationProgressEvent>>>,
}

impl MockEventSink {
    pub fn new() -> Self {
        Self::default()
    }
}

impl EventSink for MockEventSink {
    fn emit_job_progress(&self, event: &JobProgressEvent) {
        if let Ok(mut lock) = self.progress_events.lock() {
            lock.push(event.clone());
        }
    }

    fn emit_game_status(&self, event: &GameStatusEvent) {
        if let Ok(mut lock) = self.status_events.lock() {
            lock.push(event.clone());
        }
    }

    fn emit_download_progress(&self, event: &DownloadProgressEvent) {
        if let Ok(mut lock) = self.download_events.lock() {
            lock.push(event.clone());
        }
    }

    fn emit_artwork_progress(&self, event: &ArtworkProgressEvent) {
        if let Ok(mut lock) = self.artwork_events.lock() {
            lock.push(event.clone());
        }
    }

    fn emit_migration_progress(&self, event: &MigrationProgressEvent) {
        if let Ok(mut lock) = self.migration_events.lock() {
            lock.push(event.clone());
        }
    }
}

static CUSTOM_CHDMAN_PATH: Mutex<Option<String>> = Mutex::new(None);

pub fn get_custom_chdman_path() -> Option<String> {
    CUSTOM_CHDMAN_PATH.lock().ok().and_then(|guard| guard.clone())
}

pub fn set_stored_custom_chdman_path(path: Option<String>) {
    if let Ok(mut guard) = CUSTOM_CHDMAN_PATH.lock() {
        *guard = path;
    }
}

/// Fallback classifier used when Redump hash cache misses and no Jev API key is provided
/// or when remote Jev evaluation fails.
/// Extracts `.zip`/`.7z` archives found under `input_dir` into the managed
/// staging tree (`<AppData>/rom-ingestion-engine/extracted/<stem>/`) and
/// returns the staging roots to scan alongside the input. Already-extracted
/// archives (not newer than their staging) are reused, so re-scans are fast.
pub fn stage_archives_for_input(input_dir: &Path) -> Result<Vec<PathBuf>, String> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else {
                    out.push(p);
                }
            }
        }
    }
    let mut all_files = Vec::new();
    walk(input_dir, &mut all_files);

    let staging_root = crate::chdman::downloader::get_managed_tools_dir()
        .map_err(|e| e.to_string())?
        .parent()
        .map(|p| p.join("extracted"))
        .ok_or_else(|| "no staging location".to_string())?;

    let mut roots = Vec::new();
    for f in all_files {
        let ext = f
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();
        if ext != "zip" && ext != "7z" {
            continue;
        }
        let stem = f
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("archive-{}", roots.len()));
        // Only stage archives that actually contain disc images: a Downloads
        // folder is full of unrelated zips (firmware, projects) and staging
        // them all wastes gigabytes and scan time.
        if !archive_contains_disc_images(&f, &ext) {
            continue;
        }
        let dest = staging_root.join(crate::organizer::presets::sanitize_component(&stem, "archive"));
        let archive_mtime = std::fs::metadata(&f).and_then(|m| m.modified()).ok();
        let dest_mtime = std::fs::metadata(&dest).and_then(|m| m.modified()).ok();
        let fresh = match (archive_mtime, dest_mtime) {
            (Some(a), Some(d)) => d >= a,
            _ => false,
        };
        if fresh && dest.is_dir() {
            roots.push(dest);
            continue;
        }
        let _ = std::fs::remove_dir_all(&dest);
        std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
        match ext.as_str() {
            "zip" => extract_zip(&f, &dest)?,
            "7z" => extract_7z(&f, &dest)?,
            _ => unreachable!(),
        }
        roots.push(dest);
    }
    Ok(roots)
}

/// Cheap content sniff: lists archive entry names and reports whether any
/// look like disc-image files (.cue/.gdi/.iso/.img/.bin).
fn archive_contains_disc_images(archive: &Path, ext: &str) -> bool {
    fn names_look_like_discs(mut names: impl Iterator<Item = String>) -> bool {
        names.any(|n| {
            let lower = n.to_ascii_lowercase();
            [".cue", ".gdi", ".iso", ".img"].iter().any(|e| lower.ends_with(e))
                || (lower.ends_with(".bin")
                    && lower.contains("track"))
        })
    }
    match ext {
        "zip" => {
            let Ok(file) = std::fs::File::open(archive) else { return false };
            let Ok(mut za) = zip::ZipArchive::new(file) else { return false };
            names_look_like_discs((0..za.len()).filter_map(|i| {
                za.by_index(i).ok().map(|e| e.name().to_string())
            }))
        }
        "7z" => {
            let Ok(mut reader) = sevenz_rust::SevenZReader::open(
                archive,
                sevenz_rust::Password::empty(),
            ) else {
                return false;
            };
            let names: Vec<String> = reader
                .archive()
                .files
                .iter()
                .filter(|e| !e.is_directory())
                .map(|e| e.name().to_string())
                .collect();
            names_look_like_discs(names.into_iter())
        }
        _ => false,
    }
}

/// Guards archive entry names against absolute paths and `..` traversal.
fn safe_entry_dest(root: &Path, name: &str) -> Option<PathBuf> {
    let rel = std::path::Path::new(name);
    for comp in rel.components() {
        match comp {
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => return None,
            _ => {}
        }
    }
    let dest = root.join(rel);
    if dest.strip_prefix(root).is_ok() {
        Some(dest)
    } else {
        None
    }
}

fn extract_zip(archive: &Path, dest: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive).map_err(|e| e.to_string())?;
    let mut za = zip::ZipArchive::new(file).map_err(|e| format!("bad zip: {}", e))?;
    for i in 0..za.len() {
        let mut entry = za.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        let Some(target) = safe_entry_dest(dest, name.as_str()) else {
            continue; // hostile entry name: skipped, not fatal
        };
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = std::fs::File::create(&target).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn extract_7z(archive: &Path, dest: &Path) -> Result<(), String> {
    sevenz_rust::decompress_file(archive, dest).map_err(|e| format!("bad 7z: {}", e))?;
    // decompress_file writes entry names verbatim: verify nothing escaped.
    let root = std::fs::canonicalize(dest).map_err(|e| e.to_string())?;
    fn check(dir: &Path, root: &Path) -> Result<(), String> {
        for e in std::fs::read_dir(dir).map_err(|e| e.to_string())?.flatten() {
            let p = e.path();
            let canon = std::fs::canonicalize(&p).map_err(|e| e.to_string())?;
            if !canon.starts_with(root) {
                return Err(format!("archive tried to escape staging: {}", p.display()));
            }
            if p.is_dir() {
                check(&p, root)?;
            }
        }
        Ok(())
    }
    check(&root, &root)
}

fn classify_fallback(disc: &DiscFingerprint) -> GameClassification {
    let filename = disc
        .primary_file
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let stem = disc
        .primary_file
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or(filename);
    let canonical_title = crate::classifier::redump::clean_canonical_title(stem);
    let region = crate::classifier::redump::extract_region(filename)
        .unwrap_or_else(|| "Unknown".to_string());
    let (disc_number, total_discs) = crate::classifier::redump::extract_disc_info(filename);
    let is_multidisc = disc_number.map(|d| d > 1).unwrap_or(false)
        || total_discs.map(|t| t > 1).unwrap_or(false);

    GameClassification {
        canonical_title,
        platform: disc.detected_platform,
        region,
        is_multidisc,
        disc_number,
        total_discs,
        confidence: 0.70,
        source: ClassificationSource::Fallback,
    }
}

/// Resolves the remote Libretro artwork CDN URL for a game, returning `Some(url)`
/// if an online cover art match is found or `None` if missing.
#[tauri::command]
pub async fn resolve_game_artwork(
    platform: Platform,
    title: String,
    region: String,
) -> Result<Option<String>, String> {
    let client = reqwest::Client::builder()
        .user_agent("rom-ingest/0.1.0")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    Ok(crate::organizer::media::resolve_artwork_url(&client, platform, &title, &region).await)
}

/// Scans a source directory, classifies discovered disc images via Redump and TypeSafe Jev,
/// and returns a structured dry-run `IngestionPlan`.
///
/// `redump_dat_paths` optionally points at Redump XML DAT files; every entry
/// whose track-1 SHA-1 matches a DAT hash is classified as `RedumpCache`,
/// i.e. byte-verified against the reference dump.
#[tauri::command]
pub async fn scan_and_plan(
    input_dir: String,
    output_dir: String,
    preset: FrontendPreset,
    api_key: Option<String>,
    media_options: Option<MediaOptions>,
    dat_path: Option<String>,
    region_priority: Option<Vec<String>>,
    jev_base_url: Option<String>,
    custom_config: Option<CustomPresetConfig>,
    redump_dat_paths: Option<Vec<String>>,
) -> Result<IngestionPlan, String> {
    let in_path = PathBuf::from(&input_dir);
    let out_path = PathBuf::from(&output_dir);

    if !in_path.exists() {
        return Err(format!("Input directory does not exist: {}", input_dir));
    }
    if !in_path.is_dir() {
        return Err(format!("Input path is not a directory: {}", input_dir));
    }
    if out_path.exists() && !out_path.is_dir() {
        return Err(format!(
            "Output path exists and is not a directory: {}",
            output_dir
        ));
    }
    let canon_in = std::fs::canonicalize(&in_path).unwrap_or_else(|_| in_path.clone());
    let canon_out = std::fs::canonicalize(&out_path).unwrap_or_else(|_| out_path.clone());
    if canon_in == canon_out {
        return Err(
            "Output directory must be different from the input directory (sources are trashed "
                .to_string()
                + "after successful ingestion, which would destroy the results)",
        );
    }
    std::fs::create_dir_all(&out_path)
        .map_err(|e| format!("Cannot create output directory '{}': {}", output_dir, e))?;

    // Archive ingestion: .zip/.7z under the input are extracted to managed
    // staging and scanned as additional roots.
    let mut roots = vec![in_path.clone()];
    match stage_archives_for_input(&in_path) {
        Ok(staged) => roots.extend(staged),
        Err(e) => return Err(format!("Archive extraction failed: {}", e)),
    }

    let mut fingerprints = Vec::new();
    let mut skipped_sources: Vec<SkippedSource> = Vec::new();
    for root in &roots {
        let scan = crate::scanner::scan_directory(root)
            .map_err(|e| format!("Scan error: {}", e))?;
        for sk in scan.skipped {
            skipped_sources.push(SkippedSource {
                path: sk.descriptor,
                reason: sk.reason,
            });
        }
        fingerprints.extend(scan.fingerprints);
    }
    fingerprints.sort_by(|a, b| a.primary_file.cmp(&b.primary_file));
    fingerprints.dedup_by(|a, b| a.primary_file == b.primary_file);

    let mut redump_db = RedumpDatabase::with_builtin_data();
    if let Some(paths) = redump_dat_paths.as_ref() {
        for dat_path in paths {
            let file = std::fs::File::open(dat_path)
                .map_err(|e| format!("Cannot open Redump DAT '{}': {}", dat_path, e))?;
            let loaded = redump_db
                .load_dat_xml(file)
                .map_err(|e| format!("Failed to parse Redump DAT '{}': {}", dat_path, e))?;
            if loaded == 0 {
                return Err(format!(
                    "Redump DAT '{}' contained no valid entries (is it a Redump .dat file?)",
                    dat_path
                ));
            }
        }
    }
    // A user-supplied DAT may be a Redump XML dump or the shipped CSV/TSV
    // export; sniff the prefix and parse accordingly.
    if let Some(path) = dat_path.as_ref().filter(|p| !p.trim().is_empty()) {
        let file =
            std::fs::File::open(path).map_err(|e| format!("Failed to open DAT {}: {}", path, e))?;
        let mut reader = std::io::BufReader::new(file);
        let is_xml = {
            use std::io::BufRead;
            match reader.fill_buf() {
                Ok(chunk) => String::from_utf8_lossy(chunk).trim_start().starts_with("<?xml"),
                Err(_) => false,
            }
        };
        if is_xml {
            let loaded = redump_db
                .load_dat_xml(&mut reader)
                .map_err(|e| format!("Failed to parse DAT {}: {}", path, e))?;
            if loaded == 0 {
                return Err(format!(
                    "DAT '{}' contained no valid entries (is it a Redump DAT file?)",
                    path
                ));
            }
        } else {
            redump_db
                .load_csv_or_tsv(&mut reader)
                .map_err(|e| format!("Failed to read DAT {}: {}", path, e))?;
        }
    }
    let jev_client = api_key
        .as_ref()
        .filter(|k| !k.trim().is_empty())
        .map(|k| match jev_base_url.as_ref().filter(|u| !u.trim().is_empty()) {
            Some(base) => JevClient::with_base_url(k.trim().to_string(), base.clone()),
            None => JevClient::new(k.trim().to_string()),
        });

    let mut classified_items = Vec::new();
    let mut jev_errors: Vec<(PathBuf, String)> = Vec::new();

    for disc in fingerprints {
        let hash_match = disc
            .calculated_sha1
            .as_ref()
            .and_then(|sha1| redump_db.lookup_sha1(sha1));
        let serial_match = if hash_match.is_none() && !redump_db.is_empty() {
            crate::classifier::serial::read_serial_from_tracks(&disc.binary_tracks)
                .and_then(|serial| redump_db.lookup_serial(&serial))
        } else {
            None
        };

        let classification = if let Some(c) = hash_match.or(serial_match) {
            c
        } else if let Some(ref jev) = jev_client {
            let filename = disc
                .primary_file
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            let folder = disc
                .primary_file
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("");

            match jev.evaluate_game_filename(filename, folder).await {
                Ok(mut c) => {
                    if c.platform == Platform::Unknown {
                        if let Some(p) = redump_db.infer_platform_by_title(&c.canonical_title) {
                            c.platform = p;
                            c.confidence = c.confidence.max(0.75);
                        }
                    }
                    c
                }
                Err(err) => {
                    jev_errors.push((disc.primary_file.clone(), err.to_string()));
                    classify_fallback(&disc)
                }
            }
        } else {
            let mut c = classify_fallback(&disc);
            if c.platform == Platform::Unknown {
                if let Some(p) = redump_db.infer_platform_by_title(&c.canonical_title) {
                    c.platform = p;
                    c.confidence = 0.75;
                }
            }
            c
        };

        classified_items.push((disc, classification));
    }

    let media_opts = media_options.unwrap_or_default();
    let mut plan = build_ingestion_plan_with_options(
        in_path,
        out_path.clone(),
        preset,
        custom_config.as_ref(),
        classified_items,
        skipped_sources,
        &media_opts,
    );
    for game in &mut plan.games {
        for disc in &game.discs {
            if let Some((_, err)) = jev_errors
                .iter()
                .find(|(path, _)| path == &disc.source_descriptor)
            {
                game.status_note = Some(format!("Jev classification failed: {err}"));
                game.source = ClassificationSource::Fallback;
            }
        }
    }
    if let Some(priority) = region_priority {
        crate::library::apply_region_priority(&mut plan.games, &priority);
    }
    let ledger = crate::library::load_ledger(&out_path);
    if !ledger.is_empty() {
        for game in &mut plan.games {
            for disc in &mut game.discs {
                let track_sha = disc
                    .binary_tracks
                    .first()
                    .and_then(|track| crate::scanner::calculate_full_sha1(track).ok());
                if let Some(sha) = track_sha {
                    if let Some(hit) = crate::library::ledger_hit(&ledger, &sha) {
                        if PathBuf::from(&hit.chd_path).is_file() {
                            disc.status = TaskStatus::Skipped;
                        }
                    }
                }
            }
            if game.discs.iter().all(|disc| disc.status == TaskStatus::Skipped) {
                game.enabled = false;
            }
        }
    }
    Ok(plan)
}

fn media_kind_name(kind: crate::models::MediaType) -> &'static str {
    match kind {
        crate::models::MediaType::BoxArt => "box art",
        crate::models::MediaType::Screenshots => "screenshot",
        crate::models::MediaType::TitleScreens => "title screen",
    }
}

fn jailed_path_string(input_dir: &Path, path: &Path) -> Option<String> {
    if crate::paths::existing_path_within(input_dir, path) {
        Some(path.to_string_lossy().to_string())
    } else {
        None
    }
}

fn referenced_track_paths(descriptor: &Path, stored: &[PathBuf]) -> Vec<PathBuf> {
    if !stored.is_empty() {
        return stored.to_vec();
    }
    let ext = descriptor
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    let Ok(content) = std::fs::read_to_string(descriptor) else {
        return Vec::new();
    };
    let refs = match ext.as_deref() {
        Some("gdi") => crate::scanner::parse_gdi_references(&content),
        Some("cue") => crate::scanner::parse_cue_references(&content),
        _ => return Vec::new(),
    };
    let parent = descriptor.parent().unwrap_or(Path::new(""));
    refs.into_iter()
        .map(|reference| parent.join(reference))
        .filter(|path| path.exists())
        .collect()
}

fn resolve_execution_paths(
    output_dir: &Path,
    preset: FrontendPreset,
    platform: Platform,
    title: &str,
    region: &str,
    is_multidisc: bool,
    disc_number: u8,
    disc_count: u8,
    source: &Path,
    stored_command: &str,
) -> Result<crate::organizer::presets::TargetPaths, String> {
    let command = crate::paths::chdman_command_for_input(source)?;
    if !stored_command.is_empty() && stored_command != command {
        return Err(format!(
            "chdman command {stored_command} does not match {command} for {}",
            source.display()
        ));
    }
    let paths = crate::organizer::presets::resolve_target_paths(
        output_dir,
        preset,
        platform,
        title,
        region,
        is_multidisc,
        Some(disc_number),
        Some(disc_count),
    );
    if !crate::paths::is_lexically_within(output_dir, &paths.chd_path) {
        return Err(format!(
            "target escapes output folder: {}",
            paths.chd_path.display()
        ));
    }
    Ok(paths)
}

/// Internal result of executing a single disc conversion.
struct DiscConversionResult {
    game_id: String,
    disc_number: u8,
    success: bool,
    error: Option<String>,
    output_bytes: u64,
    source_files: Vec<String>,
    source_sha1: String,
    source_bytes: u64,
    serial: Option<String>,
    command: String,
    /// Resolved CHD path. Read when a later step needs the file that was written.
    #[allow(dead_code)]
    target_chd_path: PathBuf,
    /// Playlist entry captured from the plan; the M3U writer re-derives it.
    #[allow(dead_code)]
    relative_m3u_entry: Option<String>,
}

/// Internal execution engine supporting both Tauri AppHandle and MockEventSink.
pub async fn execute_plan_internal<E: EventSink + Clone + Send + Sync + 'static>(
    emitter: &E,
    plan: IngestionPlan,
    chdman_override: Option<ChdmanRunner>,
    concurrency_override: Option<usize>,
) -> Result<ExecutionSummary, String> {
    let max_concurrency = concurrency_override.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .clamp(1, 8)
    });

    let http_client = reqwest::Client::builder()
        .user_agent("rom-ingest/0.1.0")
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrency));
    let chdman = Arc::new(chdman_override.unwrap_or_else(|| {
        let configured_bin = get_custom_chdman_path()
            .or_else(|| std::env::var("CHDMAN_PATH").ok())
            .or_else(|| detect_chdman(None).path)
            .map(PathBuf::from);
        ChdmanRunner::new(configured_bin)
    }));

    let total_games = plan.games.len();
    let mut total_discs = 0;
    for g in &plan.games {
        total_discs += g.discs.len();
    }

    let mut successful_games = 0;
    let mut failed_games = 0;
    let mut failed_game_ids = Vec::new();
    let mut partial_game_ids = Vec::new();
    let mut processed_discs = 0;
    let mut all_source_files_to_trash = Vec::new();
    let mut total_output_bytes = 0u64;
    let input_dir = plan.input_dir.clone();
    let output_dir = plan.output_dir.clone();
    let preset = plan.preset;
    let chdman_version = chdman.version_string().await;
    let mut disc_join_set = tokio::task::JoinSet::new();
    let mut shells: Vec<PlannedGame> = Vec::new();

    for game in plan.games {
        if !game.enabled {
            emitter.emit_game_status(&GameStatusEvent {
                game_id: game.id.clone(),
                status: TaskStatus::Skipped,
                error: None,
            });
            continue;
        }

        emitter.emit_game_status(&GameStatusEvent {
            game_id: game.id.clone(),
            status: TaskStatus::Compressing,
            error: None,
        });

        let game_shell = game.clone();
        let disc_count = game.discs.len() as u8;

        for disc in game.discs {
            let sem = semaphore.clone();
            let runner = chdman.clone();
            let em = emitter.clone();
            let game_id = game.id.clone();
            let disc_number = disc.disc_number;
            let src = disc.source_descriptor.clone();
            let stored_tracks = disc.binary_tracks.clone();
            let stored_command = disc.chdman_command.clone();
            let title = game.canonical_title.clone();
            let region = game.region.clone();
            let platform = game.platform;
            let is_multidisc = game.is_multidisc;
            let out_dir = output_dir.clone();
            let in_dir = input_dir.clone();
            let relative_m3u_entry = disc.relative_m3u_entry.clone();

            disc_join_set.spawn(async move {
                let paths = match resolve_execution_paths(
                    &out_dir,
                    preset,
                    platform,
                    &title,
                    &region,
                    is_multidisc,
                    disc_number,
                    disc_count.max(1),
                    &src,
                    &stored_command,
                ) {
                    Ok(paths) => paths,
                    Err(error) => {
                        return DiscConversionResult {
                            game_id: game_id.clone(),
                            disc_number,
                            success: false,
                            error: Some(error),
                            output_bytes: 0,
                            source_files: Vec::new(),
                            source_sha1: String::new(),
                            source_bytes: 0,
                            serial: None,
                            command: String::new(),
                            relative_m3u_entry: None,
                            target_chd_path: PathBuf::new(),
                        };
                    }
                };
                let target = paths.chd_path;

                let _permit = match sem.acquire().await {
                    Ok(p) => p,
                    Err(e) => {
                        return DiscConversionResult {
                            game_id: game_id.clone(),
                            disc_number,
                            success: false,
                            error: Some(e.to_string()),
                            output_bytes: 0,
                            source_files: Vec::new(),
                            source_sha1: String::new(),
                            source_bytes: 0,
                            serial: None,
                            command: String::new(),
                            target_chd_path: target,
                            relative_m3u_entry: None,
                        };
                    }
                };

                let em_for_prog = em.clone();
                let g_id = game_id.clone();
                let on_prog = move |pct: f32| {
                    em_for_prog.emit_job_progress(&JobProgressEvent {
                        game_id: g_id.clone(),
                        disc_number,
                        progress: pct,
                        message: format!("Compressing disc {} ({:.1}%)", disc_number, pct),
                    });
                };

                // Convert, then gate on `chdman verify`: only discs that pass
                // are eligible for source cleanup, and a failed verification
                // removes the suspect CHD so it is never treated as good.
                let res = match runner.convert(&src, &target, on_prog).await {
                    Ok(()) => match runner.verify_output(&target).await {
                        Ok(()) => Ok(()),
                        Err(err) => {
                            let _ = tokio::fs::remove_file(&target).await;
                            Err(err)
                        }
                    },
                    Err(err) => Err(err),
                };
                drop(_permit);

                match res {
                    Ok(()) => {
                        em.emit_job_progress(&JobProgressEvent {
                            game_id: game_id.clone(),
                            disc_number,
                            progress: 100.0,
                            message: format!("Disc {} complete (verified)", disc_number),
                        });

                        let out_bytes = std::fs::metadata(&target).map(|m| m.len()).unwrap_or(0);
                        let mut sources = Vec::new();
                        if let Some(descriptor) = jailed_path_string(&in_dir, &src) {
                            sources.push(descriptor);
                        }
                        for track in referenced_track_paths(&src, &stored_tracks) {
                            if let Some(jailed) = jailed_path_string(&in_dir, &track) {
                                sources.push(jailed);
                            }
                        }

                        DiscConversionResult {
                            game_id: game_id.clone(),
                            disc_number,
                            success: true,
                            error: None,
                            output_bytes: out_bytes,
                            source_sha1: stored_tracks
                                .first()
                                .and_then(|track| crate::scanner::calculate_full_sha1(track).ok())
                                .unwrap_or_default(),
                            source_bytes: stored_tracks
                                .iter()
                                .map(|track| {
                                    std::fs::metadata(track).map(|meta| meta.len()).unwrap_or(0)
                                })
                                .sum(),
                            serial: crate::classifier::serial::read_serial_from_tracks(&stored_tracks),
                            command: crate::paths::chdman_command_for_input(&src)
                                .unwrap_or(stored_command.as_str())
                                .to_string(),
                            source_files: sources,
                            target_chd_path: target,
                            relative_m3u_entry,
                        }
                    }
                    Err(err) => DiscConversionResult {
                        game_id: game_id.clone(),
                        disc_number,
                        success: false,
                        error: Some(err.to_string()),
                        output_bytes: 0,
                        source_files: Vec::new(),
                        source_sha1: String::new(),
                        source_bytes: 0,
                        serial: None,
                        command: String::new(),
                        target_chd_path: target,
                        relative_m3u_entry: None,
                    },
                }
            });
        }
        shells.push(game_shell);
    }

    let mut grouped: HashMap<String, Vec<DiscConversionResult>> = HashMap::new();
    while let Some(joined) = disc_join_set.join_next().await {
        if let Ok(disc) = joined {
            grouped.entry(disc.game_id.clone()).or_default().push(disc);
        }
    }
    let mut media_jobs = Vec::new();
    for game in shells {
        let mut game_disc_results = grouped.remove(&game.id).unwrap_or_default();
        game_disc_results.sort_by_key(|result| result.disc_number);
        if game_disc_results.is_empty() {
            continue;
        }
        let has_failure = game_disc_results.iter().any(|result| !result.success);
        if has_failure {
            failed_games += 1;
            failed_game_ids.push(game.id.clone());
            if game.is_multidisc {
                partial_game_ids.push(game.id.clone());
            }
            let first_error = game_disc_results
                .iter()
                .find(|result| !result.success)
                .and_then(|result| result.error.clone())
                .unwrap_or_else(|| "Unknown disc conversion error".to_string());
            emitter.emit_game_status(&GameStatusEvent {
                game_id: game.id.clone(),
                status: TaskStatus::Failed,
                error: Some(first_error),
            });
            continue;
        }
        if game.is_multidisc {
            let mut relative_entries = Vec::new();
            let mut m3u_path = None;
            for disc in &game.discs {
                if let Ok(paths) = resolve_execution_paths(
                    &output_dir,
                    preset,
                    game.platform,
                    &game.canonical_title,
                    &game.region,
                    true,
                    disc.disc_number,
                    game.discs.len() as u8,
                    &disc.source_descriptor,
                    &disc.chdman_command,
                ) {
                    if m3u_path.is_none() {
                        m3u_path = paths.m3u_path;
                    }
                    if let Some(entry) = paths.relative_m3u_entry {
                        relative_entries.push(entry);
                    }
                }
            }
            if let Some(m3u_path) = m3u_path {
                if let Err(error) = crate::organizer::m3u::write_m3u_file(&m3u_path, &relative_entries) {
                    failed_games += 1;
                    failed_game_ids.push(game.id.clone());
                    partial_game_ids.push(game.id.clone());
                    emitter.emit_game_status(&GameStatusEvent {
                        game_id: game.id.clone(),
                        status: TaskStatus::Failed,
                        error: Some(format!("Failed to write M3U playlist: {error}")),
                    });
                    continue;
                }
            }
        }
        successful_games += 1;
        for result in &game_disc_results {
            processed_discs += 1;
            total_output_bytes += result.output_bytes;
            all_source_files_to_trash.extend(result.source_files.clone());
            if !result.source_sha1.is_empty() {
                let _ = crate::library::append_ledger(
                    &output_dir,
                    &crate::library::LedgerRecord {
                        source_path: result.source_files.first().cloned().unwrap_or_default(),
                        source_bytes: result.source_bytes,
                        source_sha1: result.source_sha1.clone(),
                        serial: result.serial.clone(),
                        chd_path: result.target_chd_path.to_string_lossy().to_string(),
                        chdman_version: chdman_version.clone(),
                        command: result.command.clone(),
                        result: "ok".to_string(),
                    },
                );
            }
        }
        media_jobs.push(game.clone());
        emitter.emit_game_status(&GameStatusEvent {
            game_id: game.id.clone(),
            status: TaskStatus::Verified,
            error: None,
        });
    }
    for game in media_jobs {
        if game.target_media_paths.is_empty() {
            continue;
        }
        for media_dest in &game.target_media_paths {
            let kind = crate::organizer::media::media_type_for_path(media_dest);
            let candidates = crate::organizer::media::generate_candidate_urls(
                game.platform,
                &game.canonical_title,
                &game.region,
                kind,
            );
            let resolved_url = if kind == crate::models::MediaType::BoxArt {
                if let Some(url) = game.artwork_url.clone() {
                    Some(url)
                } else {
                    crate::organizer::media::resolve_artwork_url_from_candidates(&http_client, &candidates).await
                }
            } else {
                crate::organizer::media::resolve_artwork_url_from_candidates(&http_client, &candidates).await
            };
            let Some(url) = resolved_url else {
                eprintln!(
                    "Warning: no {} image for '{}'",
                    media_kind_name(kind),
                    game.canonical_title
                );
                continue;
            };
            if let Err(error) =
                crate::organizer::media::download_media_file(&http_client, &url, media_dest).await
            {
                eprintln!(
                    "Warning: Failed to download artwork for '{}': {}",
                    game.canonical_title, error
                );
            }
        }
    }

    let mut shared_counts: HashMap<PathBuf, usize> = HashMap::new();
    for path in &all_source_files_to_trash {
        let key = PathBuf::from(path)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(path));
        *shared_counts.entry(key).or_default() += 1;
    }
    all_source_files_to_trash.retain(|path| {
        let key = PathBuf::from(path)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(path));
        shared_counts.get(&key).copied().unwrap_or(0) == 1
    });
    all_source_files_to_trash.sort();
    all_source_files_to_trash.dedup();

    Ok(ExecutionSummary {
        total_games,
        successful_games,
        failed_games,
        total_discs,
        processed_discs,
        source_files_to_trash: all_source_files_to_trash,
        total_source_bytes: plan.total_source_bytes,
        total_output_bytes,
        failed_game_ids,
        partial_game_ids,
    })
}

/// Executes the planned conversions asynchronously with bounded concurrency,
/// emitting progress events and returning an `ExecutionSummary`.
#[tauri::command]
pub async fn execute_plan(
    app_handle: tauri::AppHandle,
    plan: IngestionPlan,
) -> Result<ExecutionSummary, String> {
    execute_plan_internal(&app_handle, plan, None, None).await
}

/// Safely moves verified source dumps and track files to the OS Recycle Bin / Trash.
///
/// Paths are validated before deletion: each must exist, have a disc-image
/// extension (.cue/.bin/.gdi/.iso/.img/.raw), and — when `base_dir` is
/// supplied — be located underneath it. Invalid paths are rejected outright
/// rather than silently skipped, so the caller cannot mistake a partial
/// cleanup for a complete one.
///
/// Returns a `TrashOutcome` with the count of trashed files and their total bytes.
/// True when the volume backing `path` has a Recycle Bin the OS will use.
/// FAT32/exFAT removable drives (SD cards) commonly have recycling disabled:
/// `trash::delete` on those volumes PERMANENTLY deletes, despite the app's
/// recoverability promise. This check keeps that promise honest.
pub fn volume_has_recycle_bin(path: &Path) -> bool {
    let Some(root) = path.ancestors().last() else {
        return true; // relative path: assume the working volume is fine
    };
    // A volume root looks like "X:\". $RECYCLE.BIN exists on volumes with
    // recycling enabled.
    root.join("$RECYCLE.BIN").is_dir()
}

#[tauri::command]
pub fn trash_source_files(
    source_files: Vec<String>,
    base_dir: Option<String>,
    allow_permanent: Option<bool>,
) -> Result<TrashOutcome, String> {
    const ALLOWED_EXTENSIONS: [&str; 6] = ["cue", "bin", "gdi", "iso", "img", "raw"];

    let base = base_dir
        .as_deref()
        .map(PathBuf::from)
        .map(|b| std::fs::canonicalize(&b).unwrap_or(b));

    // Validate everything up front so a rejected list leaves nothing trashed.
    let mut unique_paths = HashSet::new();
    let mut non_recyclable_roots: Vec<String> = Vec::new();
    for s in &source_files {
        let p = PathBuf::from(s);
        if !p.exists() {
            return Err(format!(
                "Refusing to trash: source file no longer exists: {}",
                s
            ));
        }
        if !volume_has_recycle_bin(&p) {
            let root = p
                .ancestors()
                .last()
                .map(|r| r.to_string_lossy().to_string())
                .unwrap_or_else(|| "this drive".to_string());
            if !non_recyclable_roots.contains(&root) {
                non_recyclable_roots.push(root);
            }
        }
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase());
        match ext.as_deref() {
            Some(e) if ALLOWED_EXTENSIONS.contains(&e) => {}
            _ => {
                return Err(format!(
                    "Refusing to trash '{}': not a disc-image file (.cue/.bin/.gdi/.iso/.img/.raw)",
                    s
                ))
            }
        }
        if let Some(ref base) = base {
            if !is_under_root(base, &p) {
                return Err(format!(
                    "Refusing to trash '{}': it is outside the scanned library '{}'",
                    s,
                    base.display()
                ));
            }
        }
        unique_paths.insert(p);
    }

    if !non_recyclable_roots.is_empty() && allow_permanent != Some(true) {
        return Err(format!(
            "NO RECYCLE BIN on {} — this drive is configured for permanent deletion, so              removing files there CANNOT be undone. Confirm explicitly to delete permanently.",
            non_recyclable_roots.join(", ")
        ));
    }

    let mut count = 0;
    let mut bytes = 0u64;
    for p in &unique_paths {
        let len = std::fs::metadata(p).map(|meta| meta.len()).unwrap_or(0);
        trash::delete(p).map_err(|e| {
            format!(
                "Failed to move '{}' to trash: {}",
                p.to_string_lossy(),
                e
            )
        })?;
        count += 1;
        bytes += len;
    }

    Ok(TrashOutcome { count, bytes })
}

#[tauri::command]
pub fn apply_dat_release(
    mut game: PlannedGame,
    title: String,
    region: String,
    platform: Platform,
    disc_number: Option<u8>,
    output_dir: String,
    preset: FrontendPreset,
) -> PlannedGame {
    crate::library::apply_dat_choice(
        &mut game,
        &title,
        &region,
        platform,
        disc_number,
        Path::new(&output_dir),
        preset,
    );
    game
}

#[tauri::command]
pub fn rename_planned_game(
    mut game: PlannedGame,
    title: String,
    output_dir: String,
    preset: FrontendPreset,
    media_options: Option<MediaOptions>,
) -> PlannedGame {
    crate::library::apply_title_edit(
        &mut game,
        &title,
        Path::new(&output_dir),
        preset,
        &media_options.unwrap_or_default(),
    );
    game
}

#[tauri::command]
pub fn accept_cue_rewrite(cue_path: String) -> Result<String, String> {
    let path = PathBuf::from(&cue_path);
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let parent = path.parent().unwrap_or(Path::new("."));
    let rewritten = crate::library::propose_relative_cue(&text, parent)
        .ok_or_else(|| "Cue does not need a relative rewrite".to_string())?;
    std::fs::write(&path, &rewritten).map_err(|e| e.to_string())?;
    Ok(rewritten)
}

#[tauri::command]
pub fn deploy_verified_library(source_dir: String, dest_dir: String) -> Result<Vec<String>, String> {
    crate::library::deploy_library(Path::new(&source_dir), Path::new(&dest_dir), None)
}

/// Returns the current detection status and availability of chdman.
#[tauri::command]
pub async fn check_chdman_status(custom_path: Option<String>) -> Result<ChdmanStatus, String> {
    let effective_path = custom_path
        .filter(|p| !p.trim().is_empty())
        .or_else(get_custom_chdman_path);
    Ok(detect_chdman(effective_path.as_deref()))
}

/// Initiates streaming download of platform-specific chdman binary, emits progress events,
/// verifies cryptographic SHA-256 integrity, and unpacks to the managed directory.
#[tauri::command]
pub async fn download_chdman(app_handle: tauri::AppHandle) -> Result<ChdmanStatus, String> {
    download_and_install_chdman(&app_handle)
        .await
        .map_err(|e| e.to_string())
}

/// Headless download helper for testing and headless verification.
pub async fn download_chdman_internal<E: EventSink>(event_sink: &E) -> Result<ChdmanStatus, String> {
    download_and_install_chdman(event_sink)
        .await
        .map_err(|e| e.to_string())
}

/// Validates user-selected binary and persists path for subsequent operations.
#[tauri::command]
pub async fn set_custom_chdman_path(path: String) -> Result<ChdmanStatus, String> {
    let p = PathBuf::from(&path);
    if !p.is_file() {
        return Err(format!("Specified chdman path is not a file: {}", path));
    }
    let status = detect_chdman(Some(&path));
    if !status.ready {
        return Err(format!("File at '{}' is not a valid executable", path));
    }
    set_stored_custom_chdman_path(Some(path));
    Ok(status)
}

/// Finish Line: writes `gamelist.xml` metadata (ES-DE / Batocera) and
/// downloads box art from the libretro thumbnail service for the converted
/// library, emitting per-game `artwork-progress` events.
#[tauri::command]
pub async fn finish_library(
    app_handle: tauri::AppHandle,
    plan: IngestionPlan,
    download_artwork: bool,
) -> Result<crate::models::FinishLibrarySummary, String> {
    crate::metadata::finish_library_internal(&app_handle, &plan, download_artwork, None).await
}

/// Plans a library re-organization between frontend presets (dry run).
#[tauri::command]
pub fn plan_migration(
    root: String,
    source_preset: FrontendPreset,
    target_preset: FrontendPreset,
    custom_config: Option<CustomPresetConfig>,
) -> Result<crate::migrator::MigrationPlan, String> {
    crate::migrator::plan_migration(
        Path::new(&root),
        source_preset,
        target_preset,
        custom_config.as_ref(),
    )
}

/// Executes a planned preset migration: moves files, rewrites playlists,
/// regenerates gamelist metadata.
#[tauri::command]
pub fn execute_migration(
    app_handle: tauri::AppHandle,
    plan: crate::migrator::MigrationPlan,
) -> Result<crate::migrator::MigrationSummary, String> {
    crate::migrator::execute_migration(&app_handle, &plan)
}

use serde::{Deserialize, Serialize};

/// Persisted user preferences (everything except the Jev API key, which
/// stays session-only by design). Stored as JSON next to the managed tools
/// dir so it survives webview data resets.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    pub input_dir: Option<String>,
    pub output_dir: Option<String>,
    pub preset: Option<FrontendPreset>,
    pub custom_config: Option<CustomPresetConfig>,
    pub redump_dats: Option<Vec<String>>,
    pub watch_enabled: Option<bool>,
}

fn settings_path() -> Result<PathBuf, String> {
    let dir = crate::chdman::downloader::get_managed_tools_dir()
        .map_err(|e| e.to_string())?
        .parent()
        .map(|p| p.join("settings.json"))
        .ok_or_else(|| "no settings location".to_string())?;
    Ok(dir)
}

#[tauri::command]
pub fn get_app_settings() -> Result<AppSettings, String> {
    let path = settings_path()?;
    if !path.is_file() {
        return Ok(AppSettings::default());
    }
    serde_json::from_str(&std::fs::read_to_string(&path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Corrupt settings file: {}", e))
}

#[tauri::command]
pub fn set_app_settings(settings: AppSettings) -> Result<(), String> {
    let path = settings_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(())
}

/// Free/total bytes of the volume containing `path` — the "will it fit?"
/// input for space budgeting.
#[tauri::command]
pub fn get_volume_info(path: String) -> Result<serde_json::Value, String> {
    let p = PathBuf::from(&path);
    let probe = if p.is_dir() { p } else { p.parent().map(|x| x.to_path_buf()).unwrap_or(p) };
    if !probe.is_dir() {
        return Err(format!("Not a directory: {}", probe.display()));
    }
    let free = fs2::available_space(&probe).map_err(|e| e.to_string())?;
    let total = fs2::total_space(&probe).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "free_bytes": free, "total_bytes": total }))
}

/// Redump.org per-system DAT slugs serving verified ZIP downloads.
const REDUMP_DAT_SLUGS: [&str; 5] = ["psx", "ss", "dc", "mcd", "pce"];

/// Downloads Redump verification DATs (one per requested system) into
/// `dest_dir` (or the app's managed `dats/` folder when omitted) and returns
/// the extracted `.dat` paths, ready to feed into `scan_and_plan`.
#[tauri::command]
pub async fn download_redump_dats(
    dest_dir: Option<String>,
    slugs: Vec<String>,
) -> Result<Vec<String>, String> {
    let dest = match dest_dir.filter(|d| !d.trim().is_empty()) {
        Some(d) => PathBuf::from(d),
        None => crate::chdman::downloader::get_managed_tools_dir()
            .map_err(|e| e.to_string())?
            .parent()
            .map(|p| p.join("dats"))
            .unwrap_or_else(|| PathBuf::from("dats")),
    };
    std::fs::create_dir_all(&dest).map_err(|e| format!("Cannot create '{}': {}", dest.display(), e))?;

    let client = reqwest::Client::builder()
        .user_agent("rom-ingest-dat-downloader/0.1.0")
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?;

    let mut written = Vec::new();
    for slug in &slugs {
        if !REDUMP_DAT_SLUGS.contains(&slug.as_str()) {
            return Err(format!("Unknown Redump system slug: '{}'", slug));
        }
        let url = format!("http://redump.org/datfile/{}/", slug);
        let bytes = client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Request for '{}' failed: {}", slug, e))?
            .error_for_status()
            .map_err(|e| format!("redump.org returned an error for '{}': {}", slug, e))?
            .bytes()
            .await
            .map_err(|e| format!("Download of '{}' failed: {}", slug, e))?;

        if bytes.len() < 4 || &bytes[..2] != b"PK" {
            return Err(format!(
                "'{}' did not return a ZIP archive (got {} bytes) — redump.org may be regenerating the DAT; retry shortly",
                slug,
                bytes.len()
            ));
        }

        let cursor = std::io::Cursor::new(&bytes[..]);
        let mut archive = zip::ZipArchive::new(cursor).map_err(|e| format!("Bad ZIP for '{}': {}", slug, e))?;
        let mut extracted = false;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
            let name = entry.name().to_string();
            if name.to_ascii_lowercase().ends_with(".dat") {
                let target = dest.join(format!("redump_{}.dat", slug));
                let mut out = std::fs::File::create(&target)
                    .map_err(|e| format!("Cannot write '{}': {}", target.display(), e))?;
                std::io::copy(&mut entry, &mut out)
                    .map_err(|e| format!("Extracting '{}' failed: {}", name, e))?;
                written.push(target.to_string_lossy().to_string());
                extracted = true;
                break;
            }
        }
        if !extracted {
            return Err(format!("ZIP for '{}' contained no .dat file", slug));
        }
    }

    Ok(written)
}

/// Reads a small image file (box art) as a base64 data URL for the UI.
/// Only `.png`/`.jpg` files under 5 MB are served.
#[tauri::command]
pub fn read_image_file(path: String) -> Result<Option<String>, String> {
    let p = PathBuf::from(&path);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        _ => return Ok(None),
    };
    let meta = std::fs::metadata(&p).map_err(|e| format!("Cannot stat '{}': {}", path, e))?;
    if !meta.is_file() {
        return Ok(None);
    }
    if meta.len() > 5 * 1024 * 1024 {
        return Err(format!("Image too large to preview ({} bytes)", meta.len()));
    }
    let bytes = std::fs::read(&p).map_err(|e| format!("Cannot read '{}': {}", path, e))?;
    use base64::Engine as _;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(Some(format!("data:{};base64,{}", mime, encoded)))
}

/// Applies a dry-run rename: updates the canonical title and re-resolves
/// that game's output file names (CHD, M3U, artwork lookups).
#[tauri::command]
pub fn set_game_title(
    mut plan: IngestionPlan,
    game_id: String,
    title: String,
) -> Result<IngestionPlan, String> {
    crate::plan_builder::retarget_game_title(&mut plan, &game_id, &title);
    Ok(plan)
}

/// Applies a per-game platform override (dry-run UI) and returns the plan
/// with re-resolved target paths for that game.
#[tauri::command]
pub fn set_game_platform(
    mut plan: IngestionPlan,
    game_id: String,
    platform: crate::models::Platform,
) -> Result<IngestionPlan, String> {
    crate::plan_builder::retarget_game_platform(&mut plan, &game_id, platform);
    Ok(plan)
}

/// Starts or stops the "Incoming" watch folder: while enabled, new dumps
/// dropped into the input folder are auto-ingested (scan → convert → verify)
/// after a short quiet period. Sources are never modified.
#[tauri::command]
pub async fn configure_watch_folder(
    app_handle: tauri::AppHandle,
    input_dir: String,
    output_dir: String,
    preset: FrontendPreset,
    custom_config: Option<CustomPresetConfig>,
    enabled: bool,
) -> Result<String, String> {
    if !enabled {
        crate::watch::stop_watch();
        return Ok("stopped".to_string());
    }

    let in_path = PathBuf::from(&input_dir);
    let out_path = PathBuf::from(&output_dir);
    if !in_path.is_dir() {
        return Err(format!("Watch folder is not a directory: {}", input_dir));
    }
    if out_path.exists() && !out_path.is_dir() {
        return Err(format!("Output path exists and is not a directory: {}", output_dir));
    }
    let canon_in = std::fs::canonicalize(&in_path).unwrap_or_else(|_| in_path.clone());
    let canon_out = std::fs::canonicalize(&out_path).unwrap_or_else(|_| out_path.clone());
    if canon_in == canon_out {
        return Err("Output directory must differ from the watched folder".to_string());
    }
    std::fs::create_dir_all(&out_path)
        .map_err(|e| format!("Cannot create output directory '{}': {}", output_dir, e))?;

    crate::watch::start_watch(app_handle, in_path, out_path, preset, custom_config)?;
    Ok("watching".to_string())
}

