// src-tauri/src/commands.rs
use std::collections::HashSet;
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
    GameClassification, GameStatusEvent, IngestionPlan, JobProgressEvent, MigrationProgressEvent,
    SkippedSource, TaskStatus,
};
use crate::organizer::presets::CustomPresetConfig;
use crate::plan_builder::build_ingestion_plan;
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

    let scan = crate::scanner::scan_directory(&in_path).map_err(|e| format!("Scan error: {}", e))?;
    let skipped_sources: Vec<SkippedSource> = scan
        .skipped
        .into_iter()
        .map(|s| SkippedSource {
            path: s.descriptor,
            reason: s.reason,
        })
        .collect();
    let fingerprints = scan.fingerprints;

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
    let jev_client = api_key
        .as_ref()
        .filter(|k| !k.trim().is_empty())
        .map(|k| JevClient::new(k.trim().to_string()));

    let mut classified_items = Vec::new();

    for disc in fingerprints {
        // 1. Try local Redump SHA-1 lookup
        let redump_match = if let Some(ref sha1) = disc.calculated_sha1 {
            redump_db.lookup_sha1(sha1)
        } else {
            None
        };

        // 2. If no Redump match, try TypeSafe Jev if API key provided
        let classification = if let Some(c) = redump_match {
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
                Err(_) => classify_fallback(&disc),
            }
        } else {
            classify_fallback(&disc)
        };

        classified_items.push((disc, classification));
    }

    let plan = build_ingestion_plan(
        in_path,
        out_path,
        preset,
        custom_config.as_ref(),
        classified_items,
        skipped_sources,
    );
    Ok(plan)
}

/// Internal result of executing a single disc conversion.
struct DiscConversionResult {
    disc_number: u8,
    success: bool,
    error: Option<String>,
    output_bytes: u64,
    source_files: Vec<String>,
    target_chd_path: PathBuf,
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
    let mut processed_discs = 0;
    let mut all_source_files_to_trash = Vec::new();
    let mut total_output_bytes = 0u64;

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

        let mut disc_join_set = tokio::task::JoinSet::new();
        let input_root = plan.input_dir.clone();

        for disc in game.discs {
            let sem = semaphore.clone();
            let runner = chdman.clone();
            let em = emitter.clone();
            let game_id = game.id.clone();
            let disc_number = disc.disc_number;
            let src = disc.source_descriptor.clone();
            let target = disc.target_chd_path.clone();
            let relative_m3u_entry = disc.relative_m3u_entry.clone();
            let disc_input_root = input_root.clone();

            disc_join_set.spawn(async move {
                let _permit = match sem.acquire().await {
                    Ok(p) => p,
                    Err(e) => {
                        return DiscConversionResult {
                            disc_number,
                            success: false,
                            error: Some(e.to_string()),
                            output_bytes: 0,
                            source_files: Vec::new(),
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

                let res = runner.convert(&src, &target, on_prog).await;

                match res {
                    Ok(()) => {
                        // Post-conversion gate: run `chdman verify` on the
                        // finished output. Only discs that pass are eligible
                        // for source cleanup; a failed verification removes
                        // the suspect CHD so it is never treated as good.
                        em.emit_job_progress(&JobProgressEvent {
                            game_id: game_id.clone(),
                            disc_number,
                            progress: 100.0,
                            message: format!("Verifying disc {}...", disc_number),
                        });

                        if let Err(err) = runner.verify(&target).await {
                            let _ = tokio::fs::remove_file(&target).await;
                            return DiscConversionResult {
                                disc_number,
                                success: false,
                                error: Some(format!("CHD verification failed: {}", err)),
                                output_bytes: 0,
                                source_files: Vec::new(),
                                target_chd_path: target,
                                relative_m3u_entry: None,
                            };
                        }

                        em.emit_job_progress(&JobProgressEvent {
                            game_id: game_id.clone(),
                            disc_number,
                            progress: 100.0,
                            message: format!("Disc {} complete (verified)", disc_number),
                        });

                        let out_bytes = std::fs::metadata(&target).map(|m| m.len()).unwrap_or(0);
                        let mut sources = vec![src.to_string_lossy().to_string()];

                        // Discover referenced tracks if cue/gdi. Only tracks that
                        // resolve (case-insensitively) inside the scanned input
                        // root are eligible for trash; anything else is left alone.
                        let ext = src
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.to_ascii_lowercase());
                        if let Some(ref ext_str) = ext {
                            if ext_str == "cue" || ext_str == "gdi" {
                                if let Ok(content) = std::fs::read_to_string(&src) {
                                    let refs = if ext_str == "gdi" {
                                        crate::scanner::parse_gdi_references(&content)
                                    } else {
                                        crate::scanner::parse_cue_references(&content)
                                    };
                                    let parent = src.parent().unwrap_or(Path::new(""));
                                    for r in refs {
                                        if let Some(resolved) =
                                            crate::scanner::cue_parser::resolve_path_case_insensitive(
                                                parent, &r,
                                            )
                                        {
                                            if resolved.is_file()
                                                && is_under_root(&disc_input_root, &resolved)
                                            {
                                                sources.push(
                                                    resolved.to_string_lossy().to_string(),
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        DiscConversionResult {
                            disc_number,
                            success: true,
                            error: None,
                            output_bytes: out_bytes,
                            source_files: sources,
                            target_chd_path: target,
                            relative_m3u_entry,
                        }
                    }
                    Err(err) => DiscConversionResult {
                        disc_number,
                        success: false,
                        error: Some(err.to_string()),
                        output_bytes: 0,
                        source_files: Vec::new(),
                        target_chd_path: target,
                        relative_m3u_entry: None,
                    },
                }
            });
        }

        let mut game_disc_results = Vec::new();
        while let Some(res) = disc_join_set.join_next().await {
            match res {
                Ok(disc_res) => game_disc_results.push(disc_res),
                Err(join_err) => {
                    game_disc_results.push(DiscConversionResult {
                        disc_number: 0,
                        success: false,
                        error: Some(join_err.to_string()),
                        output_bytes: 0,
                        source_files: Vec::new(),
                        target_chd_path: PathBuf::new(),
                        relative_m3u_entry: None,
                    });
                }
            }
        }

        // Sort disc results by disc_number
        game_disc_results.sort_by_key(|r| r.disc_number);

        let has_failure = game_disc_results.iter().any(|r| !r.success);
        if has_failure {
            failed_games += 1;
            let first_error = game_disc_results
                .iter()
                .find(|r| !r.success)
                .and_then(|r| r.error.clone())
                .unwrap_or_else(|| "Unknown disc conversion error".to_string());

            emitter.emit_game_status(&GameStatusEvent {
                game_id: game.id.clone(),
                status: TaskStatus::Failed,
                error: Some(first_error),
            });
        } else {
            // Write M3U if multi-disc
            if game.is_multidisc {
                if let Some(ref m3u_path) = game.target_m3u_path {
                    let multidisc_subfolder =
                        crate::organizer::presets::get_multidisc_subfolder(plan.preset);
                    let mut relative_entries = Vec::new();
                    for d in &game_disc_results {
                        if let Some(entry) = &d.relative_m3u_entry {
                            relative_entries.push(entry.clone());
                        } else if let Some(name) =
                            d.target_chd_path.file_name().and_then(|f| f.to_str())
                        {
                            relative_entries.push(format!("{}/{}", multidisc_subfolder, name));
                        }
                    }

                    if let Err(e) =
                        crate::organizer::m3u::write_m3u_file(m3u_path, &relative_entries)
                    {
                        failed_games += 1;
                        emitter.emit_game_status(&GameStatusEvent {
                            game_id: game.id.clone(),
                            status: TaskStatus::Failed,
                            error: Some(format!("Failed to write M3U playlist: {}", e)),
                        });
                        continue;
                    }
                }
            }

            successful_games += 1;
            for d in game_disc_results {
                processed_discs += 1;
                total_output_bytes += d.output_bytes;
                all_source_files_to_trash.extend(d.source_files);
            }

            emitter.emit_game_status(&GameStatusEvent {
                game_id: game.id.clone(),
                status: TaskStatus::Verified,
                error: None,
            });
        }
    }

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
/// Returns the number of files successfully moved to the trash.
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
) -> Result<usize, String> {
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
    for p in &unique_paths {
        trash::delete(p).map_err(|e| {
            format!(
                "Failed to move '{}' to trash: {}",
                p.to_string_lossy(),
                e
            )
        })?;
        count += 1;
    }

    Ok(count)
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

