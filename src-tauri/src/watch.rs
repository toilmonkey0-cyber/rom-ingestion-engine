use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::commands::EventSink;
use crate::chdman::runner::ChdmanRunner;
use crate::models::{ExecutionSummary, FrontendPreset, GameClassification, TaskStatus};
use crate::organizer::presets::CustomPresetConfig;
use crate::plan_builder::build_ingestion_plan;

/// Quiet period after the last filesystem event before an ingest pass runs.
const DEBOUNCE: Duration = Duration::from_secs(5);

static INGEST_BUSY: AtomicBool = AtomicBool::new(false);

struct WatchHandle {
    stop: tokio::sync::watch::Sender<bool>,
    _join: std::thread::JoinHandle<()>,
}

static WATCH_STATE: Mutex<Option<WatchHandle>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchStatusEvent {
    /// watching | ingesting | done | error | stopped
    pub stage: String,
    pub message: String,
}

/// One full ingestion pass over the watched folder: scan, classify with the
/// builtin Redump data plus filename heuristics (no Jev — watch mode runs
/// unattended), plan, convert, verify. Sources are never touched.
pub async fn run_watch_ingestion_once<E: EventSink + Clone + Send + Sync + 'static>(
    emitter: &E,
    input_dir: &std::path::Path,
    output_dir: &std::path::Path,
    preset: FrontendPreset,
    custom_config: Option<&CustomPresetConfig>,
    chdman_override: Option<ChdmanRunner>,
) -> Result<ExecutionSummary, String> {
    let scan = crate::scanner::scan_directory(input_dir)
        .map_err(|e| format!("Watch scan error: {}", e))?;
    if scan.fingerprints.is_empty() {
        return Ok(ExecutionSummary {
            total_games: 0,
            successful_games: 0,
            failed_games: 0,
            total_discs: 0,
            processed_discs: 0,
            source_files_to_trash: Vec::new(),
            total_source_bytes: 0,
            total_output_bytes: 0,
            failed_game_ids: Vec::new(),
            partial_game_ids: Vec::new(),
        });
    }

    let redump_db = crate::classifier::redump::RedumpDatabase::with_builtin_data();
    let mut classified = Vec::with_capacity(scan.fingerprints.len());
    for disc in scan.fingerprints {
        let classification = disc
            .calculated_sha1
            .as_ref()
            .and_then(|sha1| redump_db.lookup_sha1(sha1))
            .unwrap_or_else(|| fallback_classification(&disc));
        classified.push((disc, classification));
    }

    let plan = build_ingestion_plan(
        input_dir.to_path_buf(),
        output_dir.to_path_buf(),
        preset,
        custom_config,
        classified,
        Vec::new(),
    );
    // Watch mode runs unattended on filename-heuristic (Fallback) classifications,
    // which the phase gate marks disabled for manual review in the wizard.
    // Enable playable ones here: the verify gate plus never-touching-sources makes
    // unattended conversion safe, and skipping them makes the pass a no-op.
    // Discs with scan errors (missing tracks) stay disabled and are dropped
    // from the pass entirely — there is nothing valid to convert, and the
    // pass summary must report zero games rather than a skipped shell.
    let mut plan = plan;
    plan.games.retain(|game| {
        game.discs.iter().all(|d| d.status != TaskStatus::Failed)
    });
    for game in &mut plan.games {
        if game.source == crate::models::ClassificationSource::Fallback {
            game.enabled = true;
        }
    }
    crate::commands::execute_plan_internal(emitter, plan, chdman_override, None).await
}

fn fallback_classification(disc: &crate::models::DiscFingerprint) -> GameClassification {
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
    GameClassification {
        canonical_title,
        platform: disc.detected_platform,
        region,
        is_multidisc: disc_number.map(|d| d > 1).unwrap_or(false)
            || total_discs.map(|t| t > 1).unwrap_or(false),
        disc_number,
        total_discs,
        confidence: 0.70,
        source: crate::models::ClassificationSource::Fallback,
    }
}

/// Stops the active watcher, if any.
pub fn stop_watch() {
    if let Ok(mut guard) = WATCH_STATE.lock() {
        if let Some(handle) = guard.take() {
            // The watcher thread notices within one poll interval (500ms).
            let _ = handle.stop.send(true);
        }
    }
}

/// Is a watcher currently active?
pub fn is_watching() -> bool {
    WATCH_STATE.lock().map(|g| g.is_some()).unwrap_or(false)
}

/// Starts watching `input_dir`; after a 5s quiet period following changes,
/// runs one ingestion pass into `output_dir` under `preset`. Status is
/// reported through `watch-status` events on the app handle.
pub fn start_watch(
    app: tauri::AppHandle,
    input_dir: PathBuf,
    output_dir: PathBuf,
    preset: FrontendPreset,
    custom_config: Option<CustomPresetConfig>,
) -> Result<(), String> {
    use notify::Watcher;

    stop_watch();

    let (tx, rx) = std::sync::mpsc::channel::<notify::Result<notify::Event>>();
    let mut watcher = notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    })
    .map_err(|e| format!("Cannot create filesystem watcher: {}", e))?;

    watcher
        .watch(&input_dir, notify::RecursiveMode::Recursive)
        .map_err(|e| format!("Cannot watch '{}': {}", input_dir.display(), e))?;

    let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
    let app_for_task = app.clone();

    // Capture the async runtime handle while we are ON the runtime (async
    // command context); the watcher thread uses it to drive ingestion passes.
    // A plain thread is used deliberately so the watcher never depends on
    // being spawned from runtime context.
    let runtime_handle = tokio::runtime::Handle::current();

    let join = std::thread::spawn(move || {
        let _watcher = watcher; // keep alive for the thread lifetime
        let mut pending = false;
        let mut last_event = Instant::now();
        let input = input_dir.clone();
        let output = output_dir.clone();

        loop {
            if *stop_rx.borrow() {
                break;
            }
            match rx.recv_timeout(Duration::from_millis(500)) {
                Ok(Ok(event)) => {
                    // Only disc-image activity triggers a pass.
                    let relevant = event.paths.iter().any(|p| {
                        p.extension()
                            .and_then(|e| e.to_str())
                            .map(|e| {
                                let e = e.to_ascii_lowercase();
                                ["cue", "gdi", "iso", "img", "bin"].contains(&e.as_str())
                            })
                            .unwrap_or(false)
                    });
                    if relevant {
                        pending = true;
                        last_event = Instant::now();
                    }
                }
                Ok(Err(_)) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }

            if pending && last_event.elapsed() >= DEBOUNCE {
                pending = false;
                if INGEST_BUSY.swap(true, Ordering::SeqCst) {
                    continue; // a pass is already running (manual or watch)
                }

                let _ = app_for_task.emit(
                    "watch-status",
                    WatchStatusEvent {
                        stage: "ingesting".into(),
                        message: "New dumps detected — ingesting…".into(),
                    },
                );

                let emitter = app_for_task.clone();
                let result = runtime_handle.block_on(run_watch_ingestion_once(
                    &emitter,
                    &input,
                    &output,
                    preset,
                    custom_config.as_ref(),
                    None,
                ));

                let (stage, message) = match &result {
                    Ok(summary) if summary.failed_games > 0 => (
                        "error".to_string(),
                        format!(
                            "Watch pass finished with failures: {}/{} games failed — check the logs",
                            summary.failed_games, summary.total_games
                        ),
                    ),
                    Ok(summary) => (
                        "done".to_string(),
                        format!("Watch pass complete: {} game(s) ingested", summary.successful_games),
                    ),
                    Err(e) => ("error".to_string(), format!("Watch pass failed: {}", e)),
                };
                let _ = app_for_task.emit(
                    "watch-status",
                    WatchStatusEvent { stage, message },
                );

                INGEST_BUSY.store(false, Ordering::SeqCst);
            }
        }
    });

    if let Ok(mut guard) = WATCH_STATE.lock() {
        *guard = Some(WatchHandle { stop: stop_tx, _join: join });
    }
    Ok(())
}
