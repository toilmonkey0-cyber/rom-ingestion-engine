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
    ClassificationSource, DiscFingerprint, ExecutionSummary, FrontendPreset, GameClassification,
    GameStatusEvent, IngestionPlan, JobProgressEvent, MediaOptions, PlannedGame, Platform, TaskStatus,
    TrashOutcome,
};
use crate::plan_builder::build_ingestion_plan_with_options;

/// Abstraction for emitting progress and status events to the frontend or test listener.
pub trait EventSink: Send + Sync {
    fn emit_job_progress(&self, _event: &JobProgressEvent) {}
    fn emit_game_status(&self, _event: &GameStatusEvent) {}
    fn emit_download_progress(&self, _event: &DownloadProgressEvent) {}
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
}

/// Mock event sink for testing and headless verification.
#[derive(Debug, Clone, Default)]
pub struct MockEventSink {
    pub progress_events: Arc<Mutex<Vec<JobProgressEvent>>>,
    pub status_events: Arc<Mutex<Vec<GameStatusEvent>>>,
    pub download_events: Arc<Mutex<Vec<DownloadProgressEvent>>>,
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
) -> Result<IngestionPlan, String> {
    let in_path = PathBuf::from(&input_dir);
    let out_path = PathBuf::from(&output_dir);

    if !in_path.exists() {
        return Err(format!("Input directory does not exist: {}", input_dir));
    }
    if !in_path.is_dir() {
        return Err(format!("Input path is not a directory: {}", input_dir));
    }

    let fingerprints =
        crate::scanner::scan_directory(&in_path).map_err(|e| format!("Scan error: {}", e))?;

    let mut redump_db = RedumpDatabase::new();
    if let Some(path) = dat_path.as_ref().filter(|p| !p.trim().is_empty()) {
        let file = std::fs::File::open(path).map_err(|e| format!("Failed to open DAT {}: {}", path, e))?;
        redump_db
            .load_csv_or_tsv(file)
            .map_err(|e| format!("Failed to read DAT {}: {}", path, e))?;
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
                Ok(c) => c,
                Err(err) => {
                    jev_errors.push((disc.primary_file.clone(), err.to_string()));
                    classify_fallback(&disc)
                }
            }
        } else {
            classify_fallback(&disc)
        };

        classified_items.push((disc, classification));
    }

    let media_opts = media_options.unwrap_or_default();
    let mut plan = build_ingestion_plan_with_options(
        in_path,
        out_path.clone(),
        preset,
        classified_items,
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

                let res = match runner.convert(&src, &target, on_prog).await {
                    Ok(()) => runner.verify_output(&target).await,
                    Err(err) => Err(err),
                };
                drop(_permit);

                match res {
                    Ok(()) => {
                        em.emit_job_progress(&JobProgressEvent {
                            game_id: game_id.clone(),
                            disc_number,
                            progress: 100.0,
                            message: format!("Disc {} complete", disc_number),
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
/// Returns the number of files successfully moved to the trash.
#[tauri::command]
pub fn trash_source_files(source_files: Vec<String>, input_dir: String) -> Result<TrashOutcome, String> {
    let mut count = 0;
    let mut bytes = 0u64;
    let mut unique_paths = HashSet::new();
    let root = PathBuf::from(input_dir);

    for s in source_files {
        let p = PathBuf::from(&s);
        if p.exists() && crate::paths::existing_path_within(&root, &p) && unique_paths.insert(p.clone())
        {
            let len = std::fs::metadata(&p).map(|meta| meta.len()).unwrap_or(0);
            trash::delete(&p).map_err(|e| format!("Failed to move '{}' to trash: {}", s, e))?;
            count += 1;
            bytes += len;
        }
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

