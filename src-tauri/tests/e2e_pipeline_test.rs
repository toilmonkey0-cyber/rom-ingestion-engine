// src-tauri/tests/e2e_pipeline_test.rs
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use tempfile::tempdir;

use rom_ingest_core::chdman::downloader::{
    compute_file_sha256, download_and_install_from_url, ArchiveFormat, ChdmanSource,
    PlatformManifest,
};
use rom_ingest_core::chdman::runner::{verify_chd_header, ChdmanRunner};
use rom_ingest_core::commands::*;
use rom_ingest_core::models::*;
use rom_ingest_core::organizer::m3u::parse_m3u_content;

fn get_mock_chdman_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mock_chdman"))
}

#[tokio::test]
async fn test_e2e_pipeline_anbernic_stock_multidisc_flow() {
    // 1. Create temporary input and output directories
    let dir = tempdir().unwrap();
    let input_dir = dir.path().join("messy_roms_input");
    let output_dir = dir.path().join("anbernic_sd_card");
    std::fs::create_dir_all(&input_dir).unwrap();
    std::fs::create_dir_all(&output_dir).unwrap();

    // Setup messy multi-disc disc dumps with multi-track bin/cue:
    // Disc 1 has a .cue and two binary tracks (Track 1 data + Track 2 audio)
    // Disc 2 has a .cue and one binary track
    let cue1_path = input_dir.join("Final Fantasy VII (USA) (Disc 1).cue");
    let bin1_t1_path = input_dir.join("Final Fantasy VII (USA) (Disc 1) (Track 1).bin");
    let bin1_t2_path = input_dir.join("Final Fantasy VII (USA) (Disc 1) (Track 2).bin");

    // Known SHA-1 bytes matching RedumpDatabase builtins
    // sha1("Final Fantasy VII (USA) (Disc 1) Test Track Data") = 933ec98e7c7ff0a8399a454b8cffd222471ff9e7
    File::create(&bin1_t1_path)
        .unwrap()
        .write_all(b"Final Fantasy VII (USA) (Disc 1) Test Track Data")
        .unwrap();
    File::create(&bin1_t2_path)
        .unwrap()
        .write_all(b"Final Fantasy VII (USA) (Disc 1) Track 2 Audio Content")
        .unwrap();

    let cue1_content = r#"FILE "Final Fantasy VII (USA) (Disc 1) (Track 1).bin" BINARY
  TRACK 01 MODE2/2352
    INDEX 01 00:00:00
FILE "Final Fantasy VII (USA) (Disc 1) (Track 2).bin" BINARY
  TRACK 02 AUDIO
    INDEX 00 00:00:00
    INDEX 01 00:02:00
"#;
    File::create(&cue1_path)
        .unwrap()
        .write_all(cue1_content.as_bytes())
        .unwrap();

    let cue2_path = input_dir.join("Final Fantasy VII (USA) (Disc 2).cue");
    let bin2_path = input_dir.join("Final Fantasy VII (USA) (Disc 2).bin");

    // sha1("Final Fantasy VII (USA) (Disc 2) Test Track Data") = 13e555c970aea8babfc7090bd65636767fa5946a
    File::create(&bin2_path)
        .unwrap()
        .write_all(b"Final Fantasy VII (USA) (Disc 2) Test Track Data")
        .unwrap();

    let cue2_content = r#"FILE "Final Fantasy VII (USA) (Disc 2).bin" BINARY
  TRACK 01 MODE2/2352
    INDEX 01 00:00:00
"#;
    File::create(&cue2_path)
        .unwrap()
        .write_all(cue2_content.as_bytes())
        .unwrap();

    // 2. Run scan_and_plan with AnbernicStock preset
    let plan = scan_and_plan(
        input_dir.to_string_lossy().to_string(),
        output_dir.to_string_lossy().to_string(),
        FrontendPreset::AnbernicStock,
        None,
        None,
    )
    .await
    .expect("scan_and_plan should succeed");

    // Verify discovery, Redump cache matching, multi-disc grouping
    assert_eq!(plan.games.len(), 1);
    let game = &plan.games[0];
    assert_eq!(game.canonical_title, "Final Fantasy VII");
    assert_eq!(game.platform, Platform::Psx);
    assert_eq!(game.region, "USA");
    assert!(game.is_multidisc);
    assert_eq!(game.confidence, 1.0);
    assert_eq!(game.source, ClassificationSource::RedumpCache);
    assert!(!game.needs_review);
    assert_eq!(game.discs.len(), 2);

    // Verify disc ordering
    assert_eq!(game.discs[0].disc_number, 1);
    assert_eq!(game.discs[1].disc_number, 2);

    // Verify Anbernic Stock OS path resolution:
    // CHDs must be in: <output_dir>/ROMS/PS/.discs/Final Fantasy VII (USA) (Disc N).chd
    // M3U must be in:  <output_dir>/ROMS/PS/Final Fantasy VII (USA).m3u
    let expected_chd1 = output_dir
        .join("ROMS")
        .join("PS")
        .join(".discs")
        .join("Final Fantasy VII (USA) (Disc 1).chd");
    let expected_chd2 = output_dir
        .join("ROMS")
        .join("PS")
        .join(".discs")
        .join("Final Fantasy VII (USA) (Disc 2).chd");
    let expected_m3u = output_dir
        .join("ROMS")
        .join("PS")
        .join("Final Fantasy VII (USA).m3u");

    assert_eq!(
        game.discs[0].target_chd_path.to_string_lossy().replace('\\', "/"),
        expected_chd1.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(
        game.discs[1].target_chd_path.to_string_lossy().replace('\\', "/"),
        expected_chd2.to_string_lossy().replace('\\', "/")
    );
    assert!(game.target_m3u_path.is_some());
    assert_eq!(
        game.target_m3u_path.as_ref().unwrap().to_string_lossy().replace('\\', "/"),
        expected_m3u.to_string_lossy().replace('\\', "/")
    );

    // 3. Execute plan with mock chdman runner
    let emitter = MockEventSink::new();
    let runner = ChdmanRunner::new(Some(get_mock_chdman_path()));

    let summary = execute_plan_internal(&emitter, plan, Some(runner), Some(2))
        .await
        .expect("execute_plan_internal should succeed");

    assert_eq!(summary.total_games, 1);
    assert_eq!(summary.successful_games, 1);
    assert_eq!(summary.failed_games, 0);
    assert_eq!(summary.total_discs, 2);
    assert_eq!(summary.processed_discs, 2);
    assert!(summary.total_output_bytes > 0);

    // Verify target CHD files exist and have valid MComprHD magic header
    assert!(expected_chd1.exists(), "Target CHD 1 must exist on disk");
    assert!(expected_chd2.exists(), "Target CHD 2 must exist on disk");
    assert!(
        verify_chd_header(&expected_chd1).expect("verify chd 1 header"),
        "Target CHD 1 header must be valid"
    );
    assert!(
        verify_chd_header(&expected_chd2).expect("verify chd 2 header"),
        "Target CHD 2 header must be valid"
    );

    // Verify target M3U playlist file exists and contains forward-slash relative paths
    assert!(expected_m3u.exists(), "Target M3U playlist must exist on disk");
    let m3u_content = std::fs::read_to_string(&expected_m3u).expect("read m3u content");
    assert!(!m3u_content.contains('\\'), "M3U playlist must never contain backslashes");
    let m3u_lines = parse_m3u_content(&m3u_content);
    assert_eq!(m3u_lines.len(), 2);
    assert_eq!(m3u_lines[0], ".discs/Final Fantasy VII (USA) (Disc 1).chd");
    assert_eq!(m3u_lines[1], ".discs/Final Fantasy VII (USA) (Disc 2).chd");

    // Verify emitted events
    let status_events = emitter.status_events.lock().unwrap().clone();
    assert!(status_events.iter().any(|e| e.status == TaskStatus::Compressing));
    assert!(status_events.iter().any(|e| e.status == TaskStatus::Verified));

    let progress_events = emitter.progress_events.lock().unwrap().clone();
    assert!(progress_events.iter().any(|e| e.progress == 100.0));

    // Verify source files collected for trashing include descriptors and all track bins
    assert!(summary.source_files_to_trash.contains(&cue1_path.to_string_lossy().to_string()));
    assert!(summary.source_files_to_trash.contains(&bin1_t1_path.to_string_lossy().to_string()));
    assert!(summary.source_files_to_trash.contains(&bin1_t2_path.to_string_lossy().to_string()));
    assert!(summary.source_files_to_trash.contains(&cue2_path.to_string_lossy().to_string()));
    assert!(summary.source_files_to_trash.contains(&bin2_path.to_string_lossy().to_string()));

    // 4. Test safe trash handling
    // Ensure all 5 files currently exist on disk before trashing
    assert!(cue1_path.exists());
    assert!(bin1_t1_path.exists());
    assert!(bin1_t2_path.exists());
    assert!(cue2_path.exists());
    assert!(bin2_path.exists());

    let trashed_count = trash_source_files(summary.source_files_to_trash, None)
        .expect("trash_source_files should succeed");
    assert_eq!(trashed_count, 5);

    // Verify source dumps have been safely removed from input directory
    assert!(!cue1_path.exists());
    assert!(!bin1_t1_path.exists());
    assert!(!bin1_t2_path.exists());
    assert!(!cue2_path.exists());
    assert!(!bin2_path.exists());
}

#[tokio::test]
async fn test_e2e_pipeline_multiplatform_mixed_presets() {
    let dir = tempdir().unwrap();
    let in_dir = dir.path().join("mixed_input");
    let out_dir = dir.path().join("mixed_output");
    std::fs::create_dir_all(&in_dir).unwrap();
    std::fs::create_dir_all(&out_dir).unwrap();

    // Create a Saturn single-disc game dump
    let saturn_dir = in_dir.join("saturn");
    std::fs::create_dir_all(&saturn_dir).unwrap();
    let saturn_cue = saturn_dir.join("Nights into Dreams... (USA).cue");
    let saturn_bin = saturn_dir.join("Nights into Dreams... (USA).bin");
    File::create(&saturn_bin).unwrap().write_all(b"saturn data").unwrap();
    File::create(&saturn_cue)
        .unwrap()
        .write_all(b"FILE \"Nights into Dreams... (USA).bin\" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n")
        .unwrap();

    // Create a Dreamcast GDI game dump
    let dc_dir = in_dir.join("dreamcast");
    std::fs::create_dir_all(&dc_dir).unwrap();
    let dc_gdi = dc_dir.join("Sonic Adventure (USA).gdi");
    let dc_t1 = dc_dir.join("track01.bin");
    let dc_t2 = dc_dir.join("track02.raw");
    let dc_t3 = dc_dir.join("track03.bin");
    File::create(&dc_t1).unwrap().write_all(b"dc track1").unwrap();
    File::create(&dc_t2).unwrap().write_all(b"dc track2").unwrap();
    File::create(&dc_t3).unwrap().write_all(b"dc track3").unwrap();
    let gdi_content = "3\n1 0 4 2352 track01.bin 0\n2 450 0 2352 track02.raw 0\n3 45000 4 2352 track03.bin 0\n";
    File::create(&dc_gdi).unwrap().write_all(gdi_content.as_bytes()).unwrap();

    // Test with OnionOS preset
    let plan = scan_and_plan(
        in_dir.to_string_lossy().to_string(),
        out_dir.to_string_lossy().to_string(),
        FrontendPreset::OnionOs,
        None,
        None,
    )
    .await
    .expect("mixed scan_and_plan");

    assert_eq!(plan.games.len(), 2);

    let saturn_game = plan.games.iter().find(|g| g.platform == Platform::Saturn).unwrap();
    assert_eq!(saturn_game.canonical_title, "Nights into Dreams...");
    assert!(!saturn_game.is_multidisc);
    let saturn_target = saturn_game.discs[0].target_chd_path.to_string_lossy().replace('\\', "/");
    assert!(saturn_target.contains("Roms/SEGASATURN"));
    assert!(!saturn_target.contains(".discs"));

    let dc_game = plan.games.iter().find(|g| g.platform == Platform::Dreamcast).unwrap();
    assert_eq!(dc_game.canonical_title, "Sonic Adventure");
    assert!(!dc_game.is_multidisc);
    let dc_target = dc_game.discs[0].target_chd_path.to_string_lossy().replace('\\', "/");
    assert!(dc_target.contains("Roms/DREAMCAST"));

    // Execute with mock runner
    let emitter = MockEventSink::new();
    let runner = ChdmanRunner::new(Some(get_mock_chdman_path()));
    let summary = execute_plan_internal(&emitter, plan, Some(runner), Some(2))
        .await
        .expect("execute mixed plan");

    assert_eq!(summary.total_games, 2);
    assert_eq!(summary.successful_games, 2);
    assert_eq!(summary.failed_games, 0);
    assert_eq!(summary.processed_discs, 2);

    // Dreamcast GDI had 3 tracks + 1 gdi = 4 files to trash
    // Saturn had 1 bin + 1 cue = 2 files to trash
    // Total source files to trash = 6
    assert_eq!(summary.source_files_to_trash.len(), 6);
}

#[tokio::test]
async fn test_e2e_pipeline_with_downloader_and_status_integration() {
    // 0. Ensure no prior custom path is configured
    set_stored_custom_chdman_path(None);

    // Initial check should not report a custom path
    let initial_status = check_chdman_status(None).await.expect("check_chdman_status");
    assert_ne!(initial_status.source, ChdmanSource::CustomPath);

    // 1. Setup mock server streaming a packaged chdman zip archive
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let dir = tempdir().unwrap();
    let zip_path = dir.path().join("payload.zip");
    let binary_name = if cfg!(windows) { "chdman.exe" } else { "chdman" };

    {
        let file = File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file(format!("chdman-0.268/{}", binary_name), options).unwrap();
        let mock_bytes = std::fs::read(get_mock_chdman_path()).expect("read mock chdman binary");
        zip.write_all(&mock_bytes).unwrap();
        zip.finish().unwrap();
    }

    let payload = std::fs::read(&zip_path).unwrap();
    let expected_sha256 = compute_file_sha256(&zip_path).unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let server_handle = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 1024];
        let _ = socket.read(&mut buf).await.unwrap();

        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
            payload.len()
        );
        socket.write_all(header.as_bytes()).await.unwrap();

        let mid = payload.len() / 2;
        socket.write_all(&payload[..mid]).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        socket.write_all(&payload[mid..]).await.unwrap();
        socket.flush().await.unwrap();
    });

    let download_sink = MockEventSink::new();
    let managed_bin_dir = dir.path().join("managed_bin");

    let manifest = PlatformManifest {
        os: if cfg!(windows) { "windows" } else if cfg!(target_os = "macos") { "macos" } else { "linux" },
        arch: "x86_64",
        download_url: "mock",
        expected_sha256: Box::leak(expected_sha256.into_boxed_str()),
        archive_format: ArchiveFormat::Zip,
        format: ArchiveFormat::Zip,
        binary_name,
        version: "0.268",
    };

    let url = format!("http://127.0.0.1:{}/chdman.zip", port);
    let downloaded_status = download_and_install_from_url(
        &url,
        &manifest,
        &managed_bin_dir,
        &download_sink,
    )
    .await
    .expect("download_and_install_from_url");

    server_handle.await.unwrap();

    // Verify downloaded status & events
    assert!(downloaded_status.ready);
    assert_eq!(downloaded_status.source, ChdmanSource::ManagedDirectory);
    assert!(downloaded_status.path.is_some());
    let installed_binary_path = downloaded_status.path.unwrap();
    assert!(PathBuf::from(&installed_binary_path).exists());
    assert!(!managed_bin_dir.join("chdman_download.tmp").exists());

    let dl_events = download_sink.download_events.lock().unwrap().clone();
    assert!(!dl_events.is_empty());
    assert_eq!(dl_events.last().unwrap().percentage, 100.0);

    // 2. Set as custom path and verify status query
    let custom_set_status = set_custom_chdman_path(installed_binary_path.clone())
        .await
        .expect("set_custom_chdman_path");
    assert!(custom_set_status.ready);
    assert_eq!(custom_set_status.source, ChdmanSource::CustomPath);

    let active_status = check_chdman_status(None).await.expect("check_chdman_status");
    assert!(active_status.ready);
    assert_eq!(active_status.source, ChdmanSource::CustomPath);
    assert_eq!(active_status.path.as_deref(), Some(installed_binary_path.as_str()));

    // 3. Run full scan and plan with Batocera preset
    let roms_in = dir.path().join("psx_in");
    let roms_out = dir.path().join("steamdeck_sd");
    std::fs::create_dir_all(&roms_in).unwrap();
    std::fs::create_dir_all(&roms_out).unwrap();

    let psx_in = roms_in.join("psx");
    std::fs::create_dir_all(&psx_in).unwrap();
    let cue_path = psx_in.join("Crash Bandicoot (USA).cue");
    let bin_path = psx_in.join("Crash Bandicoot (USA).bin");
    File::create(&bin_path).unwrap().write_all(b"crash bandicoot test disc track").unwrap();
    File::create(&cue_path)
        .unwrap()
        .write_all(b"FILE \"Crash Bandicoot (USA).bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n")
        .unwrap();

    let plan = scan_and_plan(
        roms_in.to_string_lossy().to_string(),
        roms_out.to_string_lossy().to_string(),
        FrontendPreset::Batocera,
        None,
        None,
    )
    .await
    .expect("scan_and_plan");

    assert_eq!(plan.games.len(), 1);
    assert_eq!(plan.games[0].canonical_title, "Crash Bandicoot");
    assert_eq!(plan.games[0].platform, Platform::Psx);
    assert_eq!(plan.games[0].discs.len(), 1);
    let target_chd = plan.games[0].discs[0].target_chd_path.clone();

    // 4. Execute plan with chdman_override = None to prove it uses detected custom chdman!
    let emitter = MockEventSink::new();
    let summary = execute_plan_internal(&emitter, plan, None, Some(2))
        .await
        .expect("execute_plan_internal using custom detected chdman");

    assert_eq!(summary.total_games, 1);
    assert_eq!(summary.successful_games, 1);
    assert_eq!(summary.failed_games, 0);
    assert_eq!(summary.processed_discs, 1);

    assert!(target_chd.exists(), "Target CHD must exist");
    assert!(
        verify_chd_header(&target_chd).expect("verify chd header"),
        "Target CHD header must have valid MComprHD magic"
    );

    // Verify progress events
    let progress = emitter.progress_events.lock().unwrap().clone();
    assert!(progress.iter().any(|p| p.progress == 100.0));

    // Trash source files
    let trashed = trash_source_files(summary.source_files_to_trash, None).expect("trash source files");
    assert_eq!(trashed, 2);
    assert!(!cue_path.exists());
    assert!(!bin_path.exists());

    // 5. Cleanup stored custom path
    set_stored_custom_chdman_path(None);
    let final_status = check_chdman_status(None).await.expect("check status after reset");
    assert_ne!(final_status.source, ChdmanSource::CustomPath);
}

