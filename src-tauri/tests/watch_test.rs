use std::fs::{self, File};
use std::io::Write;
use tempfile::tempdir;

use rom_ingest_core::chdman::runner::ChdmanRunner;
use rom_ingest_core::commands::MockEventSink;
use rom_ingest_core::models::{FrontendPreset, TaskStatus};
use rom_ingest_core::watch::run_watch_ingestion_once;

fn get_mock_chdman_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_mock_chdman"))
}

#[tokio::test]
async fn test_watch_ingestion_once_converts_new_dumps() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("incoming");
    let output = dir.path().join("library");
    fs::create_dir_all(&input).unwrap();
    fs::create_dir_all(&output).unwrap();

    let cue = input.join("psx").join("Crash Bandicoot (USA).cue");
    let bin = input.join("psx").join("Crash Bandicoot (USA).bin");
    fs::create_dir_all(input.join("psx")).unwrap();
    File::create(&bin).unwrap().write_all(b"crash data").unwrap();
    File::create(&cue)
        .unwrap()
        .write_all(b"FILE \"Crash Bandicoot (USA).bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n")
        .unwrap();

    let emitter = MockEventSink::new();
    let runner = ChdmanRunner::new(Some(get_mock_chdman_path()));
    let summary = run_watch_ingestion_once(
        &emitter,
        &input,
        &output,
        FrontendPreset::AnbernicStock,
        None,
        Some(runner),
    )
    .await
    .expect("watch ingestion pass");

    assert_eq!(summary.total_games, 1);
    assert_eq!(summary.successful_games, 1);
    let target = output.join("ROMS").join("PS").join("Crash Bandicoot (USA).chd");
    assert!(target.exists(), "converted CHD must land in the preset layout");

    // Sources are never touched by watch-mode passes: the summary may list
    // verified sources as cleanup-eligible, but nothing deletes them here.
    assert!(cue.exists());
    assert!(bin.exists());

    let statuses = emitter.status_events.lock().unwrap().clone();
    assert!(statuses.iter().any(|e| e.status == TaskStatus::Verified));
}

#[tokio::test]
async fn test_watch_ingestion_once_empty_folder_is_noop() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("incoming");
    let output = dir.path().join("library");
    fs::create_dir_all(&input).unwrap();
    fs::create_dir_all(&output).unwrap();

    let emitter = MockEventSink::new();
    let summary = run_watch_ingestion_once(
        &emitter,
        &input,
        &output,
        FrontendPreset::EsDe,
        None,
        Some(ChdmanRunner::new(Some(get_mock_chdman_path()))),
    )
    .await
    .expect("empty pass");

    assert_eq!(summary.total_games, 0);
    assert_eq!(summary.successful_games, 0);
}

#[tokio::test]
async fn test_watch_ingestion_once_skips_broken_sheet() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("incoming");
    let output = dir.path().join("library");
    fs::create_dir_all(&input).unwrap();
    fs::create_dir_all(&output).unwrap();

    // A cue referencing a missing track must be skipped, not crash the pass.
    File::create(input.join("Broken.cue"))
        .unwrap()
        .write_all(b"FILE \"Missing.bin\" BINARY\n  TRACK 01 MODE2/2352\n")
        .unwrap();

    let emitter = MockEventSink::new();
    let summary = run_watch_ingestion_once(
        &emitter,
        &input,
        &output,
        FrontendPreset::EsDe,
        None,
        Some(ChdmanRunner::new(Some(get_mock_chdman_path()))),
    )
    .await
    .expect("pass tolerates broken sheet");

    assert_eq!(summary.total_games, 0);
}
