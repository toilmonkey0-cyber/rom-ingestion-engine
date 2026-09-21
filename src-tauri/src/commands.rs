// src-tauri/src/commands.rs
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::Emitter;

use crate::chdman::runner::ChdmanRunner;
use crate::classifier::jev::JevClient;
use crate::classifier::redump::RedumpDatabase;
use crate::models::{
    ClassificationSource, DiscFingerprint, ExecutionSummary, FrontendPreset, GameClassification,
    GameStatusEvent, IngestionPlan, JobProgressEvent, TaskStatus,
};
use crate::plan_builder::build_ingestion_plan;

/// Abstraction for emitting progress and status events to the frontend or test listener.
pub trait EventSink: Send + Sync {
    fn emit_job_progress(&self, event: &JobProgressEvent);
    fn emit_game_status(&self, event: &GameStatusEvent);
}

impl EventSink for tauri::AppHandle {
    fn emit_job_progress(&self, event: &JobProgressEvent) {
        let _ = self.emit("job-progress", event);
    }

    fn emit_game_status(&self, event: &GameStatusEvent) {
        let _ = self.emit("game-status", event);
    }
}

/// Mock event sink for testing and headless verification.
#[derive(Debug, Clone, Default)]
pub struct MockEventSink {
    pub progress_events: Arc<Mutex<Vec<JobProgressEvent>>>,
    pub status_events: Arc<Mutex<Vec<GameStatusEvent>>>,
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
#[tauri::command]
pub async fn scan_and_plan(
    input_dir: String,
    output_dir: String,
    preset: FrontendPreset,
    api_key: Option<String>,
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

    let redump_db = RedumpDatabase::with_builtin_data();
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

    let plan = build_ingestion_plan(in_path, out_path, preset, classified_items);
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
        let env_bin = std::env::var("CHDMAN_PATH").ok().map(PathBuf::from);
        ChdmanRunner::new(env_bin)
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

        for disc in game.discs {
            let sem = semaphore.clone();
            let runner = chdman.clone();
            let em = emitter.clone();
            let game_id = game.id.clone();
            let disc_number = disc.disc_number;
            let src = disc.source_descriptor.clone();
            let target = disc.target_chd_path.clone();

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
                        let mut sources = vec![src.to_string_lossy().to_string()];

                        // Discover referenced tracks if cue/gdi
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
                                        let track_path = parent.join(r);
                                        if track_path.exists() {
                                            sources.push(track_path.to_string_lossy().to_string());
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
                        }
                    }
                    Err(err) => DiscConversionResult {
                        disc_number,
                        success: false,
                        error: Some(err.to_string()),
                        output_bytes: 0,
                        source_files: Vec::new(),
                        target_chd_path: target,
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
                        if let Some(name) = d.target_chd_path.file_name().and_then(|f| f.to_str()) {
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
/// Returns the number of files successfully moved to the trash.
#[tauri::command]
pub fn trash_source_files(source_files: Vec<String>) -> Result<usize, String> {
    let mut count = 0;
    let mut unique_paths = HashSet::new();

    for s in source_files {
        let p = PathBuf::from(&s);
        if p.exists() && unique_paths.insert(p.clone()) {
            trash::delete(&p).map_err(|e| format!("Failed to move '{}' to trash: {}", s, e))?;
            count += 1;
        }
    }

    Ok(count)
}
