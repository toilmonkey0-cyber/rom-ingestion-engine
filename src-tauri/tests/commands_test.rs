// src-tauri/tests/commands_test.rs
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use tempfile::tempdir;

use rom_ingest_core::chdman::runner::ChdmanRunner;
use rom_ingest_core::commands::*;
use rom_ingest_core::models::*;
use rom_ingest_core::organizer::m3u::parse_m3u_content;

fn get_mock_chdman_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mock_chdman"))
}

#[tokio::test]
async fn test_scan_and_plan_invalid_directory() {
    let result = scan_and_plan(
        "non_existent_folder_xyz_12345".to_string(),
        "out".to_string(),
        FrontendPreset::EsDe,
        None,
        None,
        None,
    )
    .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("does not exist"));
}

#[tokio::test]
async fn test_scan_and_plan_success_with_fallback() {
    let dir = tempdir().unwrap();
    let psx_dir = dir.path().join("psx");
    std::fs::create_dir_all(&psx_dir).unwrap();

    let cue_path = psx_dir.join("Crash Bandicoot (USA).cue");
    let bin_path = psx_dir.join("Crash Bandicoot (USA).bin");

    let mut f_bin = File::create(&bin_path).unwrap();
    f_bin.write_all(&[0u8; 1024]).unwrap();

    let cue_content = "FILE \"Crash Bandicoot (USA).bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n";
    let mut f_cue = File::create(&cue_path).unwrap();
    f_cue.write_all(cue_content.as_bytes()).unwrap();

    let out_dir = dir.path().join("out");

    let plan = scan_and_plan(
        psx_dir.to_string_lossy().to_string(),
        out_dir.to_string_lossy().to_string(),
        FrontendPreset::EsDe,
        None,
        None,
        None,
    )
    .await
    .unwrap();

    assert_eq!(plan.games.len(), 1);
    assert_eq!(plan.games[0].canonical_title, "Crash Bandicoot");
    assert_eq!(plan.games[0].platform, Platform::Psx);
    assert_eq!(plan.games[0].discs.len(), 1);
    assert_eq!(plan.preset, FrontendPreset::EsDe);
    assert_eq!(plan.total_source_bytes, 1024);
}

#[tokio::test]
async fn test_execute_plan_single_disc_success() {
    let dir = tempdir().unwrap();
    let in_dir = dir.path().join("in");
    let out_dir = dir.path().join("out");
    std::fs::create_dir_all(&in_dir).unwrap();
    std::fs::create_dir_all(&out_dir).unwrap();

    let cue_path = in_dir.join("Game.cue");
    let bin_path = in_dir.join("Game.bin");
    File::create(&cue_path)
        .unwrap()
        .write_all(b"FILE \"Game.bin\" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n")
        .unwrap();
    File::create(&bin_path)
        .unwrap()
        .write_all(&[0u8; 2048])
        .unwrap();

    let target_chd = out_dir.join("roms").join("psx").join("Game (USA).chd");

    let disc = PlannedDisc {
        disc_number: 1,
        source_descriptor: cue_path.clone(),
        target_chd_path: target_chd.clone(),
        relative_m3u_entry: None,
        status: TaskStatus::Pending,
    };

    let game = PlannedGame {
        id: "psx-game".to_string(),
        canonical_title: "Game".to_string(),
        platform: Platform::Psx,
        region: "USA".to_string(),
        is_multidisc: false,
        discs: vec![disc],
        target_m3u_path: None,
        confidence: 0.95,
        source: ClassificationSource::RedumpCache,
        enabled: true,
        needs_review: false,
    };

    let plan = IngestionPlan {
        input_dir: in_dir.clone(),
        output_dir: out_dir.clone(),
        preset: FrontendPreset::EsDe,
        skipped_sources: Vec::new(),
        games: vec![game],
        total_source_bytes: 2048,
        estimated_output_bytes: 1200,
    };

    let emitter = MockEventSink::new();
    let chdman = ChdmanRunner::new(Some(get_mock_chdman_path()));

    let summary = execute_plan_internal(&emitter, plan, Some(chdman), Some(2))
        .await
        .unwrap();

    assert_eq!(summary.total_games, 1);
    assert_eq!(summary.successful_games, 1);
    assert_eq!(summary.failed_games, 0);
    assert_eq!(summary.total_discs, 1);
    assert_eq!(summary.processed_discs, 1);
    assert!(target_chd.exists());

    // Check source files collected for trash include cue and bin
    assert!(summary
        .source_files_to_trash
        .contains(&cue_path.to_string_lossy().to_string()));
    assert!(summary
        .source_files_to_trash
        .contains(&bin_path.to_string_lossy().to_string()));

    // Check events emitted
    let status_events = emitter.status_events.lock().unwrap().clone();
    assert!(status_events
        .iter()
        .any(|e| e.status == TaskStatus::Compressing));
    assert!(status_events
        .iter()
        .any(|e| e.status == TaskStatus::Verified));

    let prog_events = emitter.progress_events.lock().unwrap().clone();
    assert!(prog_events.iter().any(|e| e.progress == 100.0));
}

#[tokio::test]
async fn test_execute_plan_multidisc_creates_m3u_and_discs() {
    let dir = tempdir().unwrap();
    let in_dir = dir.path().join("in");
    let out_dir = dir.path().join("out");
    std::fs::create_dir_all(&in_dir).unwrap();
    std::fs::create_dir_all(&out_dir).unwrap();

    let cue1 = in_dir.join("FF7_Disc1.cue");
    let cue2 = in_dir.join("FF7_Disc2.cue");
    File::create(&cue1).unwrap();
    File::create(&cue2).unwrap();

    let chd1 = out_dir
        .join("roms")
        .join("psx")
        .join(".discs")
        .join("Final Fantasy VII (USA) (Disc 1).chd");
    let chd2 = out_dir
        .join("roms")
        .join("psx")
        .join(".discs")
        .join("Final Fantasy VII (USA) (Disc 2).chd");
    let m3u = out_dir
        .join("roms")
        .join("psx")
        .join("Final Fantasy VII (USA).m3u");

    let game = PlannedGame {
        id: "psx-final-fantasy-vii".to_string(),
        canonical_title: "Final Fantasy VII".to_string(),
        platform: Platform::Psx,
        region: "USA".to_string(),
        is_multidisc: true,
        discs: vec![
            PlannedDisc {
                disc_number: 1,
                source_descriptor: cue1.clone(),
                target_chd_path: chd1.clone(),
        relative_m3u_entry: None,
                status: TaskStatus::Pending,
            },
            PlannedDisc {
                disc_number: 2,
                source_descriptor: cue2.clone(),
                target_chd_path: chd2.clone(),
        relative_m3u_entry: None,
                status: TaskStatus::Pending,
            },
        ],
        target_m3u_path: Some(m3u.clone()),
        confidence: 0.99,
        source: ClassificationSource::RedumpCache,
        enabled: true,
        needs_review: false,
    };

    let plan = IngestionPlan {
        input_dir: in_dir,
        output_dir: out_dir,
        preset: FrontendPreset::EsDe,
        skipped_sources: Vec::new(),
        games: vec![game],
        total_source_bytes: 1_400_000,
        estimated_output_bytes: 840_000,
    };

    let emitter = MockEventSink::new();
    let chdman = ChdmanRunner::new(Some(get_mock_chdman_path()));

    let summary = execute_plan_internal(&emitter, plan, Some(chdman), Some(2))
        .await
        .unwrap();

    assert_eq!(summary.successful_games, 1);
    assert_eq!(summary.processed_discs, 2);
    assert!(chd1.exists());
    assert!(chd2.exists());
    assert!(m3u.exists());

    // Verify M3U playlist contents
    let m3u_content = std::fs::read_to_string(&m3u).unwrap();
    let lines = parse_m3u_content(&m3u_content);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], ".discs/Final Fantasy VII (USA) (Disc 1).chd");
    assert_eq!(lines[1], ".discs/Final Fantasy VII (USA) (Disc 2).chd");
}

#[tokio::test]
async fn test_execute_plan_skipped_when_disabled() {
    let dir = tempdir().unwrap();
    let cue = dir.path().join("Skipped.cue");
    File::create(&cue).unwrap();

    let game = PlannedGame {
        id: "skipped-game".to_string(),
        canonical_title: "Skipped Game".to_string(),
        platform: Platform::Psx,
        region: "USA".to_string(),
        is_multidisc: false,
        discs: vec![PlannedDisc {
            disc_number: 1,
            source_descriptor: cue,
            target_chd_path: dir.path().join("out.chd"),
        relative_m3u_entry: None,
            status: TaskStatus::Pending,
        }],
        target_m3u_path: None,
        confidence: 0.5,
        source: ClassificationSource::Fallback,
        enabled: false,
        needs_review: true,
    };

    let plan = IngestionPlan {
        input_dir: dir.path().to_path_buf(),
        output_dir: dir.path().to_path_buf(),
        preset: FrontendPreset::EsDe,
        skipped_sources: Vec::new(),
        games: vec![game],
        total_source_bytes: 0,
        estimated_output_bytes: 0,
    };

    let emitter = MockEventSink::new();
    let chdman = ChdmanRunner::new(Some(get_mock_chdman_path()));

    let summary = execute_plan_internal(&emitter, plan, Some(chdman), Some(1))
        .await
        .unwrap();

    assert_eq!(summary.successful_games, 0);
    assert_eq!(summary.failed_games, 0);
    assert_eq!(summary.processed_discs, 0);

    let status_events = emitter.status_events.lock().unwrap().clone();
    assert_eq!(status_events.len(), 1);
    assert_eq!(status_events[0].status, TaskStatus::Skipped);
}

#[tokio::test]
async fn test_execute_plan_failure_handling() {
    let dir = tempdir().unwrap();
    // Input filename with "failure" causes mock_chdman to exit code 1
    let cue = dir.path().join("bad_failure_game.cue");
    File::create(&cue).unwrap();

    let game = PlannedGame {
        id: "failure-game".to_string(),
        canonical_title: "Failure Game".to_string(),
        platform: Platform::Psx,
        region: "USA".to_string(),
        is_multidisc: false,
        discs: vec![PlannedDisc {
            disc_number: 1,
            source_descriptor: cue,
            target_chd_path: dir.path().join("out.chd"),
        relative_m3u_entry: None,
            status: TaskStatus::Pending,
        }],
        target_m3u_path: None,
        confidence: 0.9,
        source: ClassificationSource::RedumpCache,
        enabled: true,
        needs_review: false,
    };

    let plan = IngestionPlan {
        input_dir: dir.path().to_path_buf(),
        output_dir: dir.path().to_path_buf(),
        preset: FrontendPreset::EsDe,
        skipped_sources: Vec::new(),
        games: vec![game],
        total_source_bytes: 0,
        estimated_output_bytes: 0,
    };

    let emitter = MockEventSink::new();
    let chdman = ChdmanRunner::new(Some(get_mock_chdman_path()));

    let summary = execute_plan_internal(&emitter, plan, Some(chdman), Some(1))
        .await
        .unwrap();

    assert_eq!(summary.successful_games, 0);
    assert_eq!(summary.failed_games, 1);
    assert_eq!(summary.processed_discs, 0);

    let status_events = emitter.status_events.lock().unwrap().clone();
    assert!(status_events.iter().any(|e| e.status == TaskStatus::Failed));
}

#[test]
fn test_trash_source_files() {
    let dir = tempdir().unwrap();
    let file1 = dir.path().join("trash_test1.bin");
    let file2 = dir.path().join("trash_test2.bin");
    File::create(&file1).unwrap().write_all(b"123").unwrap();
    File::create(&file2).unwrap().write_all(b"456").unwrap();

    assert!(file1.exists());
    assert!(file2.exists());

    // Duplicates are deduplicated and still trashed.
    let files_to_trash = vec![
        file1.to_string_lossy().to_string(),
        file2.to_string_lossy().to_string(),
        file1.to_string_lossy().to_string(), // duplicate
    ];

    let count = trash_source_files(files_to_trash, None, None).unwrap();
    assert_eq!(count, 2);
    assert!(!file1.exists());
    assert!(!file2.exists());
}

#[test]
fn test_trash_source_files_rejects_invalid_input_atomically() {
    let dir = tempdir().unwrap();
    let good = dir.path().join("good.bin");
    let bad_ext = dir.path().join("danger.exe");
    File::create(&good).unwrap().write_all(b"123").unwrap();
    File::create(&bad_ext).unwrap().write_all(b"456").unwrap();

    // Non-image extensions are rejected, and nothing is trashed as a result.
    let err = trash_source_files(
        vec![
            good.to_string_lossy().to_string(),
            bad_ext.to_string_lossy().to_string(),
        ],
        None,
        None,
    )
    .unwrap_err();
    assert!(err.contains("not a disc-image file"), "got: {}", err);
    assert!(good.exists(), "validation must happen before any deletion");
    assert!(bad_ext.exists());

    // Non-existent paths are rejected too.
    let err2 = trash_source_files(vec!["non_existent_file_9999.bin".to_string()], None, None).unwrap_err();
    assert!(err2.contains("no longer exists"), "got: {}", err2);

    // Containment: a valid image outside the given base dir is refused.
    let err3 = trash_source_files(
        vec![good.to_string_lossy().to_string()],
        Some(dir.path().join("nested").to_string_lossy().to_string()),
        None,
    )
    .unwrap_err();
    assert!(err3.contains("outside the scanned library"), "got: {}", err3);
}

#[tokio::test]
async fn test_check_chdman_status_command() {
    let mock_path = get_mock_chdman_path().to_string_lossy().to_string();
    let status = check_chdman_status(Some(mock_path.clone())).await.unwrap();
    assert!(status.ready);
    assert_eq!(status.source, ChdmanSource::CustomPath);
    assert_eq!(status.version.as_deref(), Some("0.268"));

    let fallback_status = check_chdman_status(Some("C:/non_existent/path/chdman.exe".to_string()))
        .await
        .unwrap();
    assert_ne!(fallback_status.source, ChdmanSource::CustomPath);
}

#[tokio::test]
async fn test_set_custom_chdman_path_valid_and_invalid() {
    let mock_path = get_mock_chdman_path().to_string_lossy().to_string();
    let status = set_custom_chdman_path(mock_path.clone()).await.unwrap();
    assert!(status.ready);
    assert_eq!(status.source, ChdmanSource::CustomPath);
    assert_eq!(status.version.as_deref(), Some("0.268"));

    // After setting, check_chdman_status(None) should use stored custom path
    let current_status = check_chdman_status(None).await.unwrap();
    assert!(current_status.ready);
    assert_eq!(current_status.source, ChdmanSource::CustomPath);

    // Invalid path should error
    let err_result = set_custom_chdman_path("C:/fake_path_does_not_exist/chdman.exe".to_string()).await;
    assert!(err_result.is_err());
}

#[tokio::test]
async fn test_execute_plan_chdman_verify_gate_blocks_success() {
    let dir = tempdir().unwrap();
    let in_dir = dir.path().join("vin");
    let out_dir = dir.path().join("vout");
    std::fs::create_dir_all(&in_dir).unwrap();
    std::fs::create_dir_all(&out_dir).unwrap();

    // The fallback title keeps the "verify_fail" marker in the target CHD
    // filename, which the mock chdman uses to fail `verify`.
    let cue = in_dir.join("Klonoa verify_fail (USA).cue");
    let bin = in_dir.join("Klonoa verify_fail (USA).bin");
    File::create(&bin).unwrap().write_all(b"klonoa data").unwrap();
    File::create(&cue)
        .unwrap()
        .write_all(b"FILE \"Klonoa verify_fail (USA).bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n")
        .unwrap();

    let plan = scan_and_plan(
        in_dir.to_string_lossy().to_string(),
        out_dir.to_string_lossy().to_string(),
        FrontendPreset::EsDe,
        None,
        None,
        None,
    )
    .await
    .expect("scan_and_plan");
    assert_eq!(plan.games.len(), 1);

    let target_chd = plan.games[0].discs[0].target_chd_path.clone();

    let emitter = MockEventSink::new();
    let runner = ChdmanRunner::new(Some(get_mock_chdman_path()));
    let summary = execute_plan_internal(&emitter, plan, Some(runner), Some(2))
        .await
        .expect("execute_plan_internal completes");

    // Conversion succeeded but verification failed: the game must be counted
    // as failed, no source files may be eligible for trash, and the suspect
    // CHD must have been removed.
    assert_eq!(summary.successful_games, 0);
    assert_eq!(summary.failed_games, 1);
    assert!(summary.source_files_to_trash.is_empty());
    assert!(!target_chd.exists(), "failed-verification CHD must be deleted");

    let statuses = emitter.status_events.lock().unwrap().clone();
    assert!(statuses.iter().any(|e| e.status == TaskStatus::Failed
        && e.error.as_deref().unwrap_or("").contains("CHD verification failed")));
}

#[tokio::test]
async fn test_runner_verify_success_and_failure() {
    let runner = ChdmanRunner::new(Some(get_mock_chdman_path()));

    // Valid CHD (magic bytes) verifies cleanly.
    let dir = tempdir().unwrap();
    let good = dir.path().join("good.chd");
    File::create(&good).unwrap().write_all(b"MComprHD\x00\x00restofthefilepadding").unwrap();
    runner.verify(&good).await.expect("valid chd verifies");

    // Not a CHD at all.
    let bad = dir.path().join("bad.chd");
    File::create(&bad).unwrap().write_all(b"junkjunkjunk").unwrap();
    assert!(runner.verify(&bad).await.is_err());

    // Missing file.
    assert!(runner.verify(dir.path().join("missing.chd")).await.is_err());
}

#[tokio::test]
async fn test_scan_and_plan_with_redump_dat_verification() {
    let dir = tempdir().unwrap();
    let in_dir = dir.path().join("din");
    let out_dir = dir.path().join("dout");
    std::fs::create_dir_all(&in_dir).unwrap();
    std::fs::create_dir_all(&out_dir).unwrap();

    let bin = in_dir.join("RayEarth (USA).bin");
    let content = b"rayearth track one payload";
    File::create(&bin).unwrap().write_all(content).unwrap();
    let cue = in_dir.join("RayEarth (USA).cue");
    File::create(&cue)
        .unwrap()
        .write_all(b"FILE \"RayEarth (USA).bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n")
        .unwrap();

    // SHA-1 of the full track content (computed with the scanner's own
    // full-file hasher), embedded in a Redump-style DAT.
    let sha1 = rom_ingest_core::scanner::calculate_track1_sha1(&bin).expect("hash track");
    let dat = format!(
        r#"<datafile><header><name>Redump.org - Sony - Playstation</name></header>
        <game name="RayEarth (USA)"><rom name="track.bin" size="{}" sha1="{}"/></game></datafile>"#,
        content.len(),
        sha1
    );
    let dat_path = dir.path().join("PSX.dat");
    File::create(&dat_path).unwrap().write_all(dat.as_bytes()).unwrap();

    let plan = scan_and_plan(
        in_dir.to_string_lossy().to_string(),
        out_dir.to_string_lossy().to_string(),
        FrontendPreset::EsDe,
        None,
        None,
        Some(vec![dat_path.to_string_lossy().to_string()]),
    )
    .await
    .expect("scan with DAT");

    assert_eq!(plan.games.len(), 1);
    let game = &plan.games[0];
    assert_eq!(game.canonical_title, "RayEarth");
    assert_eq!(game.source, ClassificationSource::RedumpCache);
    assert_eq!(game.confidence, 1.0);
    assert!(!game.needs_review);

    // A bogus DAT path must fail loudly instead of silently skipping.
    let err = scan_and_plan(
        in_dir.to_string_lossy().to_string(),
        out_dir.to_string_lossy().to_string(),
        FrontendPreset::EsDe,
        None,
        None,
        Some(vec![dir.path().join("nope.dat").to_string_lossy().to_string()]),
    )
    .await
    .unwrap_err();
    assert!(err.contains("Cannot open Redump DAT"), "got: {}", err);
}
