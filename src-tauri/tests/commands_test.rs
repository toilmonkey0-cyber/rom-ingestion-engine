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
        None,
        None,
        None, // needle_base_url
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
        None,
        None,
        None, // needle_base_url
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
        status: TaskStatus::Pending,
            binary_tracks: Vec::new(),
            chdman_command: String::new(),
            relative_m3u_entry: None,
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
        status_note: None,
        role: String::new(),
        artwork_url: None,
        target_media_paths: Vec::new(),
    };

    let plan = IngestionPlan {
        input_dir: in_dir.clone(),
        output_dir: out_dir.clone(),
        preset: FrontendPreset::EsDe,
        games: vec![game],
        skipped_sources: Vec::new(),
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
                status: TaskStatus::Pending,
            binary_tracks: Vec::new(),
            chdman_command: String::new(),
            relative_m3u_entry: None,
            },
            PlannedDisc {
                disc_number: 2,
                source_descriptor: cue2.clone(),
                target_chd_path: chd2.clone(),
                status: TaskStatus::Pending,
            binary_tracks: Vec::new(),
            chdman_command: String::new(),
            relative_m3u_entry: None,
            },
        ],
        target_m3u_path: Some(m3u.clone()),
        confidence: 0.99,
        source: ClassificationSource::RedumpCache,
        enabled: true,
        needs_review: false,
        status_note: None,
        role: String::new(),
        artwork_url: None,
        target_media_paths: Vec::new(),
    };

    let plan = IngestionPlan {
        input_dir: in_dir,
        output_dir: out_dir,
        preset: FrontendPreset::EsDe,
        games: vec![game],
        skipped_sources: Vec::new(),
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
            status: TaskStatus::Pending,
            binary_tracks: Vec::new(),
            chdman_command: String::new(),
            relative_m3u_entry: None,
        }],
        target_m3u_path: None,
        confidence: 0.5,
        source: ClassificationSource::Fallback,
        enabled: false,
        needs_review: true,
        status_note: None,
        role: String::new(),
        artwork_url: None,
        target_media_paths: Vec::new(),
    };

    let plan = IngestionPlan {
        input_dir: dir.path().to_path_buf(),
        output_dir: dir.path().to_path_buf(),
        preset: FrontendPreset::EsDe,
        games: vec![game],
        skipped_sources: Vec::new(),
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
            status: TaskStatus::Pending,
            binary_tracks: Vec::new(),
            chdman_command: String::new(),
            relative_m3u_entry: None,
        }],
        target_m3u_path: None,
        confidence: 0.9,
        source: ClassificationSource::RedumpCache,
        enabled: true,
        needs_review: false,
        status_note: None,
        role: String::new(),
        artwork_url: None,
        target_media_paths: Vec::new(),
    };

    let plan = IngestionPlan {
        input_dir: dir.path().to_path_buf(),
        output_dir: dir.path().to_path_buf(),
        preset: FrontendPreset::EsDe,
        games: vec![game],
        skipped_sources: Vec::new(),
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

    let files_to_trash = vec![
        file1.to_string_lossy().to_string(),
        file2.to_string_lossy().to_string(),
        file1.to_string_lossy().to_string(), // duplicate
    ];

    let outcome = trash_source_files(files_to_trash, None, None).unwrap();
    assert_eq!(outcome.count, 2);
    assert_eq!(outcome.bytes, 6);
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

    // Containment: a valid image outside the given base dir is refused,
    // and nothing inside the same call is trashed.
    let err3 = trash_source_files(
        vec![good.to_string_lossy().to_string()],
        Some(dir.path().join("nested").to_string_lossy().to_string()),
        None,
    )
    .unwrap_err();
    assert!(err3.contains("outside the scanned library"), "got: {}", err3);
    assert!(good.exists());
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
async fn test_scan_and_plan_with_custom_media_options() {
    let dir = tempdir().unwrap();
    let psx_dir = dir.path().join("psx");
    std::fs::create_dir_all(&psx_dir).unwrap();

    let cue_path = psx_dir.join("Crash Bandicoot (USA).cue");
    let bin_path = psx_dir.join("Crash Bandicoot (USA).bin");
    File::create(&bin_path).unwrap().write_all(&[0u8; 1024]).unwrap();
    File::create(&cue_path)
        .unwrap()
        .write_all(b"FILE \"Crash Bandicoot (USA).bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n")
        .unwrap();

    let out_dir = dir.path().join("out");

    let media_opts = MediaOptions {
        download_boxart: true,
        download_screenshots: true,
        download_titles: false,
    };

    let plan = scan_and_plan(
        psx_dir.to_string_lossy().to_string(),
        out_dir.to_string_lossy().to_string(),
        FrontendPreset::EsDe,
        None,
        Some(media_opts),
        None,
        None,
        None,
        None, // needle_base_url
        None,
        None,
    )
    .await
    .unwrap();

    assert_eq!(plan.games.len(), 1);
    assert_eq!(plan.games[0].target_media_paths.len(), 2);
    assert_eq!(
        plan.games[0].target_media_paths[0],
        out_dir.join("ROMs").join("psx").join("media").join("covers").join("Crash Bandicoot (USA).png")
    );
    assert_eq!(
        plan.games[0].target_media_paths[1],
        out_dir.join("ROMs").join("psx").join("media").join("screenshots").join("Crash Bandicoot (USA).png")
    );
}

#[tokio::test]
async fn test_resolve_game_artwork_command_unknown_platform() {
    let res = resolve_game_artwork(
        Platform::Unknown,
        "Unknown Game".to_string(),
        "USA".to_string(),
    )
    .await;
    assert_eq!(res, Ok(None));
}

#[tokio::test]
async fn test_execute_plan_with_artwork_download_and_failure_resilience() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let server = tokio::spawn(async move {
        loop {
            let (mut socket, _) = match listener.accept().await {
                Ok(conn) => conn,
                Err(_) => break,
            };

            let mut buf = [0u8; 2048];
            let n = socket.read(&mut buf).await.unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);

            if req.starts_with("GET /good_art.png") {
                let img_data = [0x89, b'P', b'N', b'G', 99, 98, 97];
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: image/png\r\nConnection: close\r\n\r\n",
                    img_data.len()
                );
                let _ = socket.write_all(resp.as_bytes()).await;
                let _ = socket.write_all(&img_data).await;
            } else if req.starts_with("GET /bad_art.png") {
                let resp = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                let _ = socket.write_all(resp.as_bytes()).await;
            }
            let _ = socket.flush().await;
        }
    });

    let dir = tempdir().unwrap();
    let in_dir = dir.path().join("in");
    let out_dir = dir.path().join("out");
    std::fs::create_dir_all(&in_dir).unwrap();
    std::fs::create_dir_all(&out_dir).unwrap();

    let cue1 = in_dir.join("Game1.cue");
    let bin1 = in_dir.join("Game1.bin");
    File::create(&cue1)
        .unwrap()
        .write_all(b"FILE \"Game1.bin\" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n")
        .unwrap();
    File::create(&bin1).unwrap().write_all(&[0u8; 2048]).unwrap();

    let cue2 = in_dir.join("Game2.cue");
    let bin2 = in_dir.join("Game2.bin");
    File::create(&cue2)
        .unwrap()
        .write_all(b"FILE \"Game2.bin\" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n")
        .unwrap();
    File::create(&bin2).unwrap().write_all(&[0u8; 2048]).unwrap();

    let target_chd1 = out_dir.join("roms").join("psx").join("Game1 (USA).chd");
    let target_chd2 = out_dir.join("roms").join("psx").join("Game2 (USA).chd");

    let media_path1 = out_dir.join("roms").join("psx").join("media").join("covers").join("Game1 (USA).png");
    let media_path2 = out_dir.join("roms").join("psx").join("media").join("covers").join("Game2 (USA).png");

    let game1 = PlannedGame {
        id: "game-1".to_string(),
        canonical_title: "Game1".to_string(),
        platform: Platform::Psx,
        region: "USA".to_string(),
        is_multidisc: false,
        discs: vec![PlannedDisc {
            disc_number: 1,
            source_descriptor: cue1,
            target_chd_path: target_chd1.clone(),
            status: TaskStatus::Pending,
            binary_tracks: Vec::new(),
            chdman_command: String::new(),
            relative_m3u_entry: None,
        }],
        target_m3u_path: None,
        confidence: 0.95,
        source: ClassificationSource::RedumpCache,
        enabled: true,
        needs_review: false,
        status_note: None,
        role: String::new(),
        artwork_url: Some(format!("http://127.0.0.1:{}/good_art.png", port)),
        target_media_paths: vec![media_path1.clone()],
    };

    let game2 = PlannedGame {
        id: "game-2".to_string(),
        canonical_title: "Game2".to_string(),
        platform: Platform::Psx,
        region: "USA".to_string(),
        is_multidisc: false,
        discs: vec![PlannedDisc {
            disc_number: 1,
            source_descriptor: cue2,
            target_chd_path: target_chd2.clone(),
            status: TaskStatus::Pending,
            binary_tracks: Vec::new(),
            chdman_command: String::new(),
            relative_m3u_entry: None,
        }],
        target_m3u_path: None,
        confidence: 0.95,
        source: ClassificationSource::RedumpCache,
        enabled: true,
        needs_review: false,
        status_note: None,
        role: String::new(),
        artwork_url: Some(format!("http://127.0.0.1:{}/bad_art.png", port)),
        target_media_paths: vec![media_path2.clone()],
    };

    let plan = IngestionPlan {
        input_dir: in_dir,
        output_dir: out_dir,
        preset: FrontendPreset::EsDe,
        games: vec![game1, game2],
        skipped_sources: Vec::new(),
        total_source_bytes: 4096,
        estimated_output_bytes: 2400,
    };
    let emitter = MockEventSink::new();
    let chdman = ChdmanRunner::new(Some(get_mock_chdman_path()));

    let summary = execute_plan_internal(&emitter, plan, Some(chdman), Some(2))
        .await
        .unwrap();

    // Both games must succeed, conversion should not fail due to bad artwork!
    assert_eq!(summary.total_games, 2);
    assert_eq!(summary.successful_games, 2);
    assert_eq!(summary.failed_games, 0);

    // Game 1 media file must exist with exact downloaded bytes
    assert!(media_path1.exists(), "Game 1 artwork should be downloaded");
    let downloaded_bytes = std::fs::read(&media_path1).unwrap();
    assert_eq!(downloaded_bytes, vec![0x89, b'P', b'N', b'G', 99, 98, 97]);

    // Game 2 media file should not exist, but conversion is verified
    assert!(!media_path2.exists(), "Game 2 artwork should not exist due to 404");
    assert!(target_chd2.exists(), "Game 2 CHD must still be created");

    let status_events = emitter.status_events.lock().unwrap().clone();
    let g2_events: Vec<_> = status_events.iter().filter(|e| e.game_id == "game-2").collect();
    assert!(g2_events.iter().any(|e| e.status == TaskStatus::Verified));
    assert!(!g2_events.iter().any(|e| e.status == TaskStatus::Failed));

    server.abort();
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
        None,
        None,
        None, // needle_base_url
        None,
        None,
    )
    .await
    .expect("scan_and_plan");
    assert_eq!(plan.games.len(), 1);

    // The merged phase gate disables fallback games pending review; the user
    // accepts the game so execution reaches the verification gate.
    let mut plan = plan;
    assert!(!plan.games[0].enabled, "fallback game must start disabled for review");
    plan.games[0].enabled = true;

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
        None,
        None,
        None,
        None, // needle_base_url
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
        None,
        None,
        None,
        None, // needle_base_url
        None,
        Some(vec![dir.path().join("nope.dat").to_string_lossy().to_string()]),
    )
    .await
    .unwrap_err();
    assert!(err.contains("Cannot open Redump DAT"), "got: {}", err);
}

#[tokio::test]
async fn test_scan_ingests_zip_archives() {
    let dir = tempdir().unwrap();
    let in_dir = dir.path().join("downloads");
    let out_dir = dir.path().join("out");
    std::fs::create_dir_all(&in_dir).unwrap();
    std::fs::create_dir_all(&out_dir).unwrap();

    // Build a zip containing a cue+bin pair, exactly like a Vimm's download.
    let zip_path = in_dir.join("Zipped Game (USA).zip");
    {
        let file = File::create(&zip_path).unwrap();
        let mut z = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default();
        z.start_file("Zipped Game (USA)/Zipped Game (USA).cue", opts).unwrap();
        z.write_all(b"FILE \"Zipped Game (USA).bin\" BINARY\n  TRACK 01 MODE2/2352\n").unwrap();
        z.start_file("Zipped Game (USA)/Zipped Game (USA).bin", opts).unwrap();
        z.write_all(b"zip game track data").unwrap();
        z.finish().unwrap();
    }

    let plan = scan_and_plan(
        in_dir.to_string_lossy().to_string(),
        out_dir.to_string_lossy().to_string(),
        FrontendPreset::EsDe,
        None,
        None,
        None,
        None,
        None,
        None, // needle_base_url
        None,
        None,
    )
    .await
    .expect("scan with archive");

    assert_eq!(plan.games.len(), 1, "archive contents become a planned game");
    assert_eq!(plan.games[0].canonical_title, "Zipped Game");
}

#[test]
fn test_archive_entry_traversal_is_blocked() {
    use rom_ingest_core::commands::stage_archives_for_input;
    let dir = tempdir().unwrap();
    let in_dir = dir.path().join("ins");
    std::fs::create_dir_all(&in_dir).unwrap();

    let zip_path = in_dir.join("evil.zip");
    {
        let file = File::create(&zip_path).unwrap();
        let mut z = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default();
        // Hostile entries: traversal + absolute — must be skipped, not written.
        z.start_file("../escaped.txt", opts).unwrap();
        z.write_all(b"pwn").unwrap();
        z.start_file("ok/game.cue", opts).unwrap();
        z.write_all(b"FILE \"x.bin\" BINARY\n").unwrap();
        z.finish().unwrap();
    }

    let roots = stage_archives_for_input(&in_dir).expect("staging succeeds");
    assert_eq!(roots.len(), 1);
    assert!(!dir.path().join("escaped.txt").exists(), "no escape");
    let staged_root = &roots[0];
    assert!(staged_root.join("ok").join("game.cue").is_file());
    let mut files = Vec::new();
    fn walk(p: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(p).unwrap().flatten() {
            let ep = e.path();
            if ep.is_dir() { walk(&ep, out) } else { out.push(ep) }
        }
    }
    walk(staged_root, &mut files);
    assert!(files.iter().all(|f| f.starts_with(staged_root)));
}

#[tokio::test]
async fn test_platform_inferred_from_dat_titles() {
    let dir = tempdir().unwrap();
    let in_dir = dir.path().join("iin");
    let out_dir = dir.path().join("oot");
    std::fs::create_dir_all(in_dir.join("psx")).unwrap();
    std::fs::create_dir_all(&out_dir).unwrap();

    // Cue name matches a DAT title; hash deliberately does NOT (so fallback
    // classification runs, and title inference must rescue the platform).
    let cue = in_dir.join("psx").join("RayEarth (USA).cue");
    let bin = in_dir.join("psx").join("RayEarth (USA).bin");
    File::create(&bin).unwrap().write_all(b"totally different bytes").unwrap();
    File::create(&cue)
        .unwrap()
        .write_all(b"FILE \"RayEarth (USA).bin\" BINARY\n  TRACK 01 MODE2/2352\n").unwrap();

    let dat = r#"<datafile><header><name>Sony - PlayStation</name></header>
        <game name="Some Other Game (Europe)"><rom name="t.bin" size="1" sha1="cccccccccccccccccccccccccccccccccccccccc"/></game>
        <game name="RayEarth (USA)"><rom name="t.bin" size="1" sha1="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"/></game>
        <game name="RayEarth (Europe)"><rom name="t.bin" size="1" sha1="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"/></game>
        </datafile>"#;
    let dat_path = dir.path().join("PSX.dat");
    File::create(&dat_path).unwrap().write_all(dat.as_bytes()).unwrap();

    let plan = scan_and_plan(
        in_dir.to_string_lossy().to_string(),
        out_dir.to_string_lossy().to_string(),
        FrontendPreset::EsDe,
        None,
        None,
        None,
        None,
        None,
        None, // needle_base_url
        None,
        Some(vec![dat_path.to_string_lossy().to_string()]),
    )
    .await
    .expect("scan");

    assert_eq!(plan.games.len(), 1);
    let g = &plan.games[0];
    // Fallback source (hash missed) but platform inferred from the title.
    assert_eq!(g.source, ClassificationSource::Fallback);
    assert_eq!(g.platform, Platform::Psx, "platform inferred from DAT title");
    let target = g.discs[0].target_chd_path.to_string_lossy().to_lowercase();
    assert!(target.contains("roms/psx/"), "goes to psx folder: {}", target);
}
