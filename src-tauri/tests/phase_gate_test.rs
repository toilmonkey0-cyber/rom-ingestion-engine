use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tempfile::tempdir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use rom_ingest_core::chdman::runner::ChdmanRunner;
use rom_ingest_core::classifier::redump::extract_edition_tag;
use rom_ingest_core::classifier::serial::read_catalog_serial;
use rom_ingest_core::commands::*;
use rom_ingest_core::library::{
    apply_dat_choice, apply_region_priority, deploy_library, load_ledger, propose_relative_cue,
};
use rom_ingest_core::models::*;
use rom_ingest_core::organizer::media::{
    generate_candidate_urls, resolve_artwork_url_from_candidates, MediaType,
};
use rom_ingest_core::plan_builder::build_ingestion_plan;

fn mock_chdman() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mock_chdman"))
}

fn fingerprint(path: &str, tracks: &[&str], bytes: u64) -> DiscFingerprint {
    DiscFingerprint {
        primary_file: PathBuf::from(path),
        binary_tracks: tracks.iter().map(PathBuf::from).collect(),
        detected_platform: Platform::Psx,
        calculated_sha1: None,
        total_bytes: bytes,
        scan_error: None,
    }
}

fn class(title: &str, region: &str, disc: Option<u8>) -> GameClassification {
    GameClassification {
        canonical_title: title.to_string(),
        platform: Platform::Psx,
        region: region.to_string(),
        is_multidisc: disc.is_some_and(|n| n > 0),
        disc_number: disc,
        total_discs: disc,
        confidence: 0.99,
        source: ClassificationSource::RedumpCache,
    }
}

#[test]
fn test_revision_tags_stay_separate_games() {
    assert_eq!(extract_edition_tag("Game (USA) (Rev 1)"), "rev 1");
    let rev1 = fingerprint("in/Game (USA) (Rev 1).cue", &["in/a.bin"], 10);
    let rev2 = fingerprint("in/Game (USA) (Rev 2).cue", &["in/b.bin"], 10);
    let plan = build_ingestion_plan(
        PathBuf::from("in"),
        PathBuf::from("out"),
        FrontendPreset::EsDe,
        None,
        vec![
            (rev1, class("Game", "USA", Some(1))),
            (rev2, class("Game", "USA", Some(1))),
        ],
        Vec::new(),
    );
    assert_eq!(plan.games.len(), 2, "revisions must not merge");
}

#[tokio::test]
async fn test_without_dat_fallback_starts_disabled() {
    let dir = tempdir().unwrap();
    let cue = dir.path().join("Mystery (USA).cue");
    let bin = dir.path().join("Mystery (USA).bin");
    File::create(&bin).unwrap().write_all(b"mystery-bytes").unwrap();
    File::create(&cue)
        .unwrap()
        .write_all(b"FILE \"Mystery (USA).bin\" BINARY\n  TRACK 01 MODE1/2352\n")
        .unwrap();
    let plan = scan_and_plan(
        dir.path().to_string_lossy().to_string(),
        dir.path().join("out").to_string_lossy().to_string(),
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
    assert_eq!(plan.games[0].source, ClassificationSource::Fallback);
    assert!(!plan.games[0].enabled);
}

#[tokio::test]
async fn test_dat_checksum_names_the_fixture() {
    let dir = tempdir().unwrap();
    let bin = dir.path().join("ff7_d1.bin");
    let cue = dir.path().join("ff7_d1.cue");
    File::create(&bin).unwrap().write_all(b"unique-dat-bytes").unwrap();
    File::create(&cue)
        .unwrap()
        .write_all(b"FILE \"ff7_d1.bin\" BINARY\n  TRACK 01 MODE1/2352\n")
        .unwrap();
    let sha = rom_ingest_core::scanner::calculate_full_sha1(&bin).unwrap();
    let dat = dir.path().join("games.csv");
    std::fs::write(
        &dat,
        format!("sha1,title,platform,region,disc_number,total_discs\n{sha},Final Fantasy VII (USA),psx,USA,1,1\n"),
    )
    .unwrap();
    let out = dir.path().join("out");
    let plan = scan_and_plan(
        dir.path().to_string_lossy().to_string(),
        out.to_string_lossy().to_string(),
        FrontendPreset::EsDe,
        None,
        None,
        Some(dat.to_string_lossy().to_string()),
        None,
        None,
        None, // needle_base_url
    None,
    None,
    )
    .await
    .unwrap();
    assert_eq!(plan.games[0].canonical_title, "Final Fantasy VII");
    assert_eq!(plan.games[0].source, ClassificationSource::RedumpCache);
    assert!(plan.games[0].enabled);
}

#[test]
fn test_serial_from_image_beats_filename_via_dat() {
    let bytes = b"BOOT2 = cdrom:\\SLUS_007.08;1";
    assert_eq!(read_catalog_serial(bytes).as_deref(), Some("SLUS_007.08"));
    let mut db = rom_ingest_core::classifier::redump::RedumpDatabase::new();
    db.load_csv_or_tsv(
        "sha1,title,platform,region,serial\n0123456789abcdef0123456789abcdef01234567,Final Fantasy VII,psx,USA,SLUS_007.08\n"
            .as_bytes(),
    )
    .unwrap();
    let named = db.lookup_serial("SLUS_007.08").unwrap();
    assert_eq!(named.canonical_title, "Final Fantasy VII");
    assert!(db.lookup_sha1("not-the-file").is_none());
}

#[tokio::test]
async fn test_scan_serial_hit_beats_the_filename() {
    let dir = tempdir().unwrap();
    let bin = dir.path().join("ff7_d1.bin");
    let cue = dir.path().join("ff7_d1.cue");
    File::create(&bin)
        .unwrap()
        .write_all(b"BOOT2 = cdrom:\\SLUS_007.08;1")
        .unwrap();
    File::create(&cue)
        .unwrap()
        .write_all(b"FILE \"ff7_d1.bin\" BINARY\n  TRACK 01 MODE1/2352\n")
        .unwrap();
    let dat = dir.path().join("serial.csv");
    std::fs::write(
        &dat,
        "sha1,title,platform,region,serial\n0123456789abcdef0123456789abcdef01234567,Final Fantasy VII,psx,USA,SLUS_007.08\n",
    )
    .unwrap();
    let plan = scan_and_plan(
        dir.path().to_string_lossy().to_string(),
        dir.path().join("out").to_string_lossy().to_string(),
        FrontendPreset::EsDe,
        None,
        None,
        Some(dat.to_string_lossy().to_string()),
        None,
        None,
        None, // needle_base_url
    None,
    None,
    )
    .await
    .unwrap();
    assert_eq!(plan.games[0].canonical_title, "Final Fantasy VII");
    assert_eq!(plan.games[0].source, ClassificationSource::RedumpCache);
}

#[tokio::test]
async fn test_jev_error_is_visible_and_does_not_merge_discs() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let mut buf = [0u8; 2048];
            let _ = socket.read(&mut buf).await;
            let body = b"unauthorized";
            let header = format!(
                "HTTP/1.1 401 Unauthorized\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = socket.write_all(header.as_bytes()).await;
            let _ = socket.write_all(body).await;
        }
    });
    let dir = tempdir().unwrap();
    for name in ["Alpha (USA)", "Beta (USA)"] {
        let bin = dir.path().join(format!("{name}.bin"));
        let cue = dir.path().join(format!("{name}.cue"));
        File::create(&bin).unwrap().write_all(name.as_bytes()).unwrap();
        let cue_body = format!("FILE \"{name}.bin\" BINARY\n  TRACK 01 MODE1/2352\n");
        File::create(&cue).unwrap().write_all(cue_body.as_bytes()).unwrap();
    }
    let plan = scan_and_plan(
        dir.path().to_string_lossy().to_string(),
        dir.path().join("out").to_string_lossy().to_string(),
        FrontendPreset::EsDe,
        Some("test-key".to_string()),
        None,
        None,
        None,
        Some(format!("http://127.0.0.1:{port}/v1/systemone")),
        None, // needle_base_url
    None,
    None,
    )
    .await
    .unwrap();
    assert_eq!(plan.games.len(), 2);
    assert!(plan.games.iter().all(|game| {
        game.status_note
            .as_deref()
            .is_some_and(|note| note.contains("Jev classification failed"))
    }));
}

#[tokio::test]
async fn test_dat_choice_changes_the_written_chd_path() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("in");
    let output = dir.path().join("out");
    std::fs::create_dir_all(&input).unwrap();
    let cue = input.join("ff7_d1.cue");
    File::create(&cue).unwrap().write_all(b"FILE \"ff7_d1.bin\" BINARY\n").unwrap();
    File::create(input.join("ff7_d1.bin")).unwrap().write_all(b"bin").unwrap();
    let mut game = PlannedGame {
        id: "raw".to_string(),
        canonical_title: "ff7_d1".to_string(),
        platform: Platform::Psx,
        region: "Unknown".to_string(),
        is_multidisc: false,
        discs: vec![PlannedDisc {
            disc_number: 1,
            source_descriptor: cue,
            target_chd_path: output.join("wrong.chd"),
            status: TaskStatus::Pending,
            binary_tracks: vec![input.join("ff7_d1.bin")],
            chdman_command: "createcd".to_string(),
            relative_m3u_entry: None,
        }],
        target_m3u_path: None,
        confidence: 0.7,
        source: ClassificationSource::Fallback,
        enabled: true,
        needs_review: true,
        status_note: None,
        role: String::new(),
        artwork_url: None,
        target_media_paths: Vec::new(),
    };
    apply_dat_choice(
        &mut game,
        "Final Fantasy VII",
        "USA",
        Platform::Psx,
        Some(1),
        &output,
        FrontendPreset::EsDe,
    );
    let shown = game.discs[0].target_chd_path.clone();
    assert!(shown.to_string_lossy().replace('\\', "/").ends_with("ROMs/psx/Final Fantasy VII (USA).chd"));
    let plan = IngestionPlan {
        input_dir: input,
        output_dir: output.clone(),
        preset: FrontendPreset::EsDe,
        games: vec![game],
        skipped_sources: Vec::new(),
        total_source_bytes: 3,
        estimated_output_bytes: 1,
    };
    let summary = execute_plan_internal(
        &MockEventSink::new(),
        plan,
        Some(ChdmanRunner::new(Some(mock_chdman()))),
        Some(1),
    )
    .await
    .unwrap();
    assert_eq!(summary.successful_games, 1);
    assert!(shown.exists(), "execution must write the path shown before execute");
}

#[tokio::test]
async fn test_artwork_candidates_differ_and_spaced_url_gets_http_200() {
    let boxart = generate_candidate_urls(Platform::Psx, "Final Fantasy VII", "USA", MediaType::BoxArt);
    let snaps = generate_candidate_urls(Platform::Psx, "Final Fantasy VII", "USA", MediaType::Screenshots);
    assert!(boxart[0].contains("Named_Boxarts"));
    assert!(snaps[0].contains("Named_Snaps"));
    assert_ne!(boxart[0], snaps[0]);
    assert!(boxart[0].contains("%20"));
    assert!(!boxart[0].contains(' '));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 1024];
        let _ = socket.read(&mut buf).await;
        let body = b"\x89PNG\r\n";
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: image/png\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = socket.write_all(header.as_bytes()).await;
        let _ = socket.write_all(body).await;
    });
    let url = format!("http://127.0.0.1:{port}/Final%20Fantasy%20VII.png");
    let client = reqwest::Client::new();
    let found = resolve_artwork_url_from_candidates(&client, &[url.clone()]).await;
    assert_eq!(found.as_deref(), Some(url.as_str()));
}

#[tokio::test]
async fn test_second_scan_of_unchanged_folder_proposes_zero_converts() {
    let dir = tempdir().unwrap();
    let bin = dir.path().join("Keep (USA).bin");
    let cue = dir.path().join("Keep (USA).cue");
    File::create(&bin).unwrap().write_all(b"keep-bytes").unwrap();
    File::create(&cue)
        .unwrap()
        .write_all(b"FILE \"Keep (USA).bin\" BINARY\n  TRACK 01 MODE1/2352\n")
        .unwrap();
    let sha = rom_ingest_core::scanner::calculate_full_sha1(&bin).unwrap();
    let dat = dir.path().join("keep.csv");
    std::fs::write(
        &dat,
        format!("sha1,title,platform,region,disc_number,total_discs\n{sha},Keep,psx,USA,1,1\n"),
    )
    .unwrap();
    let out = dir.path().join("library");
    let plan = scan_and_plan(
        dir.path().to_string_lossy().to_string(),
        out.to_string_lossy().to_string(),
        FrontendPreset::EsDe,
        None,
        None,
        Some(dat.to_string_lossy().to_string()),
        None,
        None,
        None, // needle_base_url
    None,
    None,
    )
    .await
    .unwrap();
    execute_plan_internal(
        &MockEventSink::new(),
        plan,
        Some(ChdmanRunner::new(Some(mock_chdman()))),
        Some(1),
    )
    .await
    .unwrap();
    let again = scan_and_plan(
        dir.path().to_string_lossy().to_string(),
        out.to_string_lossy().to_string(),
        FrontendPreset::EsDe,
        None,
        None,
        Some(dat.to_string_lossy().to_string()),
        None,
        None,
        None, // needle_base_url
    None,
    None,
    )
    .await
    .unwrap();
    assert!(again.games.iter().all(|game| !game.enabled));
    assert!(again
        .games
        .iter()
        .flat_map(|game| game.discs.iter())
        .all(|disc| disc.status == TaskStatus::Skipped));
}

#[tokio::test]
async fn test_region_priority_keeps_alternate_out_of_trash() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("in");
    std::fs::create_dir_all(&input).unwrap();
    for (name, _region) in [("USA", "USA"), ("Japan", "Japan")] {
        let bin = input.join(format!("Game ({name}).bin"));
        let cue = input.join(format!("Game ({name}).cue"));
        File::create(&bin).unwrap().write_all(name.as_bytes()).unwrap();
        let body = format!("FILE \"Game ({name}).bin\" BINARY\n  TRACK 01 MODE1/2352\n");
        File::create(&cue).unwrap().write_all(body.as_bytes()).unwrap();
    }
    let mut plan = scan_and_plan(
        input.to_string_lossy().to_string(),
        dir.path().join("out").to_string_lossy().to_string(),
        FrontendPreset::EsDe,
        None,
        None,
        None,
        Some(vec!["USA".to_string(), "Japan".to_string()]),
        None,
        None, // needle_base_url
    None,
    None,
    )
    .await
    .unwrap();
    apply_region_priority(&mut plan.games, &["USA".to_string(), "Japan".to_string()]);
    let japan = plan.games.iter().find(|game| game.region == "Japan").unwrap();
    assert_eq!(japan.role, "alternate");
    assert!(!japan.enabled);
    let japan_cue = japan.discs[0].source_descriptor.clone();
    for game in &mut plan.games {
        if game.role != "alternate" {
            game.enabled = true;
        }
    }
    let summary = execute_plan_internal(
        &MockEventSink::new(),
        plan,
        Some(ChdmanRunner::new(Some(mock_chdman()))),
        Some(2),
    )
    .await
    .unwrap();
    assert!(!summary
        .source_files_to_trash
        .iter()
        .any(|path| Path::new(path) == japan_cue));
}

#[test]
fn test_deploy_refuses_when_free_space_is_short_and_copies_when_it_fits() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("library");
    std::fs::create_dir_all(source.join("roms")).unwrap();
    std::fs::write(source.join("roms").join("Game.chd"), b"MComprHDabcdef").unwrap();
    let too_small = dir.path().join("card-small");
    let err = deploy_library(&source, &too_small, Some(1)).unwrap_err();
    assert!(err.contains("not enough free space"));
    assert!(!too_small.exists());
    let dest = dir.path().join("card-ok");
    let names = deploy_library(&source, &dest, Some(u64::MAX)).unwrap();
    assert_eq!(names, vec!["roms/Game.chd".to_string()]);
    assert!(dest.join("roms").join("Game.chd").is_file());
}

#[test]
fn test_cue_rewrite_uses_a_sibling_filename() {
    let dir = tempdir().unwrap();
    File::create(dir.path().join("Track.bin")).unwrap().write_all(b"x").unwrap();
    let cue = "FILE \"D:/other/Track.bin\" BINARY\n  TRACK 01 MODE1/2352\n";
    let rewritten = propose_relative_cue(cue, dir.path()).unwrap();
    assert!(rewritten.contains("FILE \"Track.bin\""));
    assert!(!rewritten.contains("D:/other"));
}

#[tokio::test]
async fn test_trash_stays_off_until_verify_succeeds() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("in");
    let output = dir.path().join("out");
    std::fs::create_dir_all(&input).unwrap();
    let bad = input.join("bad_header_game.cue");
    File::create(&bad).unwrap().write_all(b"FILE \"bad_header_game.bin\" BINARY\n").unwrap();
    File::create(input.join("bad_header_game.bin")).unwrap().write_all(b"nope").unwrap();
    let bad_plan = IngestionPlan {
        input_dir: input.clone(),
        output_dir: output.clone(),
        preset: FrontendPreset::EsDe,
        skipped_sources: Vec::new(),
        games: vec![PlannedGame {
            id: "bad".to_string(),
            canonical_title: "Bad Header".to_string(),
            platform: Platform::Psx,
            region: "USA".to_string(),
            is_multidisc: false,
            discs: vec![PlannedDisc {
                disc_number: 1,
                source_descriptor: bad,
                target_chd_path: output.join("bad.chd"),
                status: TaskStatus::Pending,
                binary_tracks: Vec::new(),
                chdman_command: "createcd".to_string(),
                relative_m3u_entry: None,
            }],
            target_m3u_path: None,
            confidence: 1.0,
            source: ClassificationSource::RedumpCache,
            enabled: true,
            needs_review: false,
            status_note: None,
            role: String::new(),
            artwork_url: None,
            target_media_paths: Vec::new(),
        }],
        total_source_bytes: 4,
        estimated_output_bytes: 1,
    };
    let failed = execute_plan_internal(
        &MockEventSink::new(),
        bad_plan,
        Some(ChdmanRunner::new(Some(mock_chdman()))),
        Some(1),
    )
    .await
    .unwrap();
    assert_eq!(failed.failed_games, 1);
    assert!(failed.source_files_to_trash.is_empty());

    let good_cue = input.join("good_game.cue");
    let good_bin = input.join("good_game.bin");
    File::create(&good_cue).unwrap().write_all(b"FILE \"good_game.bin\" BINARY\n").unwrap();
    File::create(&good_bin).unwrap().write_all(b"yes").unwrap();
    let good_plan = IngestionPlan {
        input_dir: input,
        output_dir: output,
        preset: FrontendPreset::EsDe,
        skipped_sources: Vec::new(),
        games: vec![PlannedGame {
            id: "good".to_string(),
            canonical_title: "Good Game".to_string(),
            platform: Platform::Psx,
            region: "USA".to_string(),
            is_multidisc: false,
            discs: vec![PlannedDisc {
                disc_number: 1,
                source_descriptor: good_cue.clone(),
                target_chd_path: PathBuf::new(),
                status: TaskStatus::Pending,
                binary_tracks: vec![good_bin],
                chdman_command: "createcd".to_string(),
                relative_m3u_entry: None,
            }],
            target_m3u_path: None,
            confidence: 1.0,
            source: ClassificationSource::RedumpCache,
            enabled: true,
            needs_review: false,
            status_note: None,
            role: String::new(),
            artwork_url: None,
            target_media_paths: Vec::new(),
        }],
        total_source_bytes: 3,
        estimated_output_bytes: 1,
    };
    let ok = execute_plan_internal(
        &MockEventSink::new(),
        good_plan,
        Some(ChdmanRunner::new(Some(mock_chdman()))),
        Some(1),
    )
    .await
    .unwrap();
    assert_eq!(ok.successful_games, 1);
    assert!(ok.source_files_to_trash.iter().any(|path| Path::new(path) == good_cue));
}

#[tokio::test]
async fn test_two_games_start_chdman_together() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("in");
    let output = dir.path().join("out");
    std::fs::create_dir_all(&input).unwrap();
    let mut discs = Vec::new();
    for name in ["slowdisc-a", "slowdisc-b"] {
        let cue = input.join(format!("{name}.cue"));
        File::create(&cue).unwrap().write_all(b"FILE \"x.bin\" BINARY\n").unwrap();
        discs.push(PlannedDisc {
            disc_number: 1,
            source_descriptor: cue,
            target_chd_path: output.join(format!("{name}.chd")),
            status: TaskStatus::Pending,
            binary_tracks: Vec::new(),
            chdman_command: "createcd".to_string(),
            relative_m3u_entry: None,
        });
    }
    let games = discs
        .into_iter()
        .enumerate()
        .map(|(index, disc)| PlannedGame {
            id: format!("g{index}"),
            canonical_title: format!("Slow {index}"),
            platform: Platform::Psx,
            region: "USA".to_string(),
            is_multidisc: false,
            discs: vec![disc],
            target_m3u_path: None,
            confidence: 1.0,
            source: ClassificationSource::RedumpCache,
            enabled: true,
            needs_review: false,
            status_note: None,
            role: String::new(),
            artwork_url: None,
            target_media_paths: Vec::new(),
        })
        .collect();
    let plan = IngestionPlan {
        input_dir: input.clone(),
        output_dir: output,
        preset: FrontendPreset::EsDe,
        skipped_sources: Vec::new(),
        games,
        total_source_bytes: 1,
        estimated_output_bytes: 1,
    };
    let started_a = input.join("slowdisc-a.started");
    let started_b = input.join("slowdisc-b.started");
    let runner = ChdmanRunner::new(Some(mock_chdman()));
    let handle = tokio::spawn(async move {
        execute_plan_internal(&MockEventSink::new(), plan, Some(runner), Some(2)).await
    });
    let began = Instant::now();
    while !started_a.exists() || !started_b.exists() {
        if began.elapsed() > Duration::from_secs(3) {
            panic!("both chdman workers did not start within 3s");
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(
        began.elapsed() < Duration::from_millis(900),
        "workers started {:?} apart, so they did not share a pool",
        began.elapsed()
    );
    handle.await.unwrap().unwrap();
}

#[test]
fn test_same_region_editions_stay_keepers() {
    let rev1 = fingerprint("in/Game (USA) (Rev 1).cue", &["in/a.bin"], 10);
    let rev2 = fingerprint("in/Game (USA) (Rev 2).cue", &["in/b.bin"], 10);
    let japan = fingerprint("in/Game (Japan) (Rev 1).cue", &["in/c.bin"], 10);
    let mut plan = build_ingestion_plan(
        PathBuf::from("in"),
        PathBuf::from("out"),
        FrontendPreset::EsDe,
        None,
        vec![
            (rev1, class("Game", "USA", None)),
            (rev2, class("Game", "USA", None)),
            (japan, class("Game", "Japan", None)),
        ],
        Vec::new(),
    );
    assert_eq!(plan.games.len(), 3);
    apply_region_priority(&mut plan.games, &["USA".to_string(), "Japan".to_string()]);
    for game in &plan.games {
        let source = game.discs[0].source_descriptor.to_string_lossy();
        if source.contains("(USA)") {
            assert_eq!(game.role, "keeper", "{source}");
            assert!(game.enabled, "{source}");
        } else {
            assert_eq!(game.role, "alternate");
            assert!(!game.enabled);
        }
    }
}

#[tokio::test]
async fn test_shared_bin_referenced_by_two_cues_is_not_trashed() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("in");
    std::fs::create_dir_all(&input).unwrap();
    let shared = input.join("shared.bin");
    File::create(&shared).unwrap().write_all(b"shared-track-bytes").unwrap();
    for name in ["Alpha (USA)", "Beta (USA)"] {
        let cue = input.join(format!("{name}.cue"));
        let body = "FILE \"shared.bin\" BINARY\n  TRACK 01 MODE1/2352\n";
        File::create(&cue).unwrap().write_all(body.as_bytes()).unwrap();
    }
    let mut plan = scan_and_plan(
        input.to_string_lossy().to_string(),
        dir.path().join("out").to_string_lossy().to_string(),
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
    assert_eq!(plan.games.len(), 2);
    for game in &mut plan.games {
        game.enabled = true;
    }
    let summary = execute_plan_internal(
        &MockEventSink::new(),
        plan,
        Some(ChdmanRunner::new(Some(mock_chdman()))),
        Some(2),
    )
    .await
    .unwrap();
    assert_eq!(summary.successful_games, 2);
    assert!(
        !summary
            .source_files_to_trash
            .iter()
            .any(|path| Path::new(path).file_name().and_then(|n| n.to_str()) == Some("shared.bin")),
        "shared bin stayed on the trash list: {:?}",
        summary.source_files_to_trash
    );
    assert!(summary.source_files_to_trash.iter().any(|path| path.contains("Alpha")));
    assert!(summary.source_files_to_trash.iter().any(|path| path.contains("Beta")));
}

#[tokio::test]
async fn test_ledger_records_source_size_serial_version_and_real_command() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("in");
    let output = dir.path().join("out");
    std::fs::create_dir_all(&input).unwrap();
    let track_body = b"SEGA SEGAKATANA HDR-01234 catalog-and-padding-bytes";
    let track = input.join("shenmue.track");
    File::create(&track).unwrap().write_all(track_body).unwrap();
    let gdi = input.join("Shenmue (USA).gdi");
    File::create(&gdi)
        .unwrap()
        .write_all(b"1\n1 0 4 2352 shenmue.track 0\n")
        .unwrap();
    let plan = IngestionPlan {
        input_dir: input,
        output_dir: output.clone(),
        preset: FrontendPreset::EsDe,
        skipped_sources: Vec::new(),
        games: vec![PlannedGame {
            id: "shenmue".to_string(),
            canonical_title: "Shenmue".to_string(),
            platform: Platform::Dreamcast,
            region: "USA".to_string(),
            is_multidisc: false,
            discs: vec![PlannedDisc {
                disc_number: 1,
                source_descriptor: gdi,
                target_chd_path: PathBuf::new(),
                status: TaskStatus::Pending,
                binary_tracks: vec![track],
                chdman_command: "createdvd".to_string(),
                relative_m3u_entry: None,
            }],
            target_m3u_path: None,
            confidence: 1.0,
            source: ClassificationSource::RedumpCache,
            enabled: true,
            needs_review: false,
            status_note: None,
            role: String::new(),
            artwork_url: None,
            target_media_paths: Vec::new(),
        }],
        total_source_bytes: track_body.len() as u64,
        estimated_output_bytes: 1,
    };
    let summary = execute_plan_internal(
        &MockEventSink::new(),
        plan,
        Some(ChdmanRunner::new(Some(mock_chdman()))),
        Some(1),
    )
    .await
    .unwrap();
    assert_eq!(summary.successful_games, 1);
    let rows = load_ledger(&output);
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.source_bytes, track_body.len() as u64);
    assert_ne!(row.source_bytes, summary.total_output_bytes);
    assert_eq!(row.serial.as_deref(), Some("HDR-01234"));
    assert_eq!(row.command, "createdvd");
    assert_ne!(row.chdman_version, "unknown");
    assert_eq!(row.chdman_version, "0.268");
    assert_eq!(row.result, "ok");
}

#[test]
fn test_accept_cue_rewrite_persists_the_sibling_name() {
    let dir = tempdir().unwrap();
    File::create(dir.path().join("Track.bin")).unwrap().write_all(b"x").unwrap();
    let cue = dir.path().join("Game.cue");
    File::create(&cue)
        .unwrap()
        .write_all(b"FILE \"D:/other/Track.bin\" BINARY\n  TRACK 01 MODE1/2352\n")
        .unwrap();
    let rewritten = accept_cue_rewrite(cue.to_string_lossy().to_string()).unwrap();
    assert!(rewritten.contains("FILE \"Track.bin\""));
    assert!(!rewritten.contains("D:/other"));
    assert_eq!(std::fs::read_to_string(&cue).unwrap(), rewritten);
}

fn header_with_product(marker: &str, field_at: usize, product: &str) -> Vec<u8> {
    let mut bytes = vec![0u8; field_at + 16];
    let marker = marker.as_bytes();
    bytes[..marker.len()].copy_from_slice(marker);
    let product = product.as_bytes();
    bytes[field_at..field_at + product.len()].copy_from_slice(product);
    bytes
}

#[test]
fn test_header_product_fields_match_documented_serials() {
    assert_eq!(
        read_catalog_serial(&header_with_product("SEGA SEGAKATANA ", 0x40, "HDR-0176")).as_deref(),
        Some("HDR-0176")
    );
    assert_eq!(
        read_catalog_serial(&header_with_product("SEGA SEGAKATANA ", 0x40, "MK-51000")).as_deref(),
        Some("MK-51000")
    );
    assert_eq!(
        read_catalog_serial(&header_with_product("SEGA SEGAKATANA ", 0x40, "T-9714N")).as_deref(),
        Some("T-9714N")
    );
    assert_eq!(
        read_catalog_serial(&header_with_product("SEGA SEGASATURN ", 0x20, "MK-81076")).as_deref(),
        Some("MK-81076")
    );
    let mut segacd = vec![0u8; 0x200];
    segacd[..14].copy_from_slice(b"SEGADISCSYSTEM");
    segacd[0x180..0x18A].copy_from_slice(b"GM T-12345");
    assert_eq!(read_catalog_serial(&segacd).as_deref(), Some("T-12345"));
}

#[tokio::test]
async fn test_scan_dat_serials_from_real_disc_headers_beat_filenames() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("in");
    std::fs::create_dir_all(&input).unwrap();

    std::fs::write(input.join("track01.bin"), b"lead-in-without-catalog").unwrap();
    std::fs::write(input.join("track02.raw"), b"low-density-gap").unwrap();
    std::fs::write(
        input.join("track03.bin"),
        header_with_product("SEGA SEGAKATANA ", 0x40, "HDR-0176"),
    )
    .unwrap();
    std::fs::write(
        input.join("Wrong Name (USA).gdi"),
        "3\n1 0 4 2352 track01.bin 0\n2 450 0 2352 track02.raw 0\n3 450 4 2352 track03.bin 0\n",
    )
    .unwrap();

    let saturn = header_with_product("SEGA SEGASATURN ", 0x20, "MK-81076");
    std::fs::write(input.join("mystery.bin"), &saturn).unwrap();
    std::fs::write(
        input.join("Mystery (USA).cue"),
        "FILE \"mystery.bin\" BINARY\n  TRACK 01 MODE1/2352\n",
    )
    .unwrap();

    let mut segacd = vec![0u8; 0x200];
    segacd[..14].copy_from_slice(b"SEGADISCSYSTEM");
    segacd[0x180..0x18A].copy_from_slice(b"GM T-12345");
    std::fs::write(input.join("not-sonic.bin"), &segacd).unwrap();
    std::fs::write(
        input.join("not-sonic (USA).cue"),
        "FILE \"not-sonic.bin\" BINARY\n  TRACK 01 MODE1/2352\n",
    )
    .unwrap();

    let dat = dir.path().join("headers.csv");
    std::fs::write(
        &dat,
        "\
sha1,title,platform,region,serial
0123456789abcdef0123456789abcdef01234567,Shenmue,dreamcast,USA,HDR-0176
deadbeefdeadbeefdeadbeefdeadbeefdeadbeef,Panzer Dragoon,saturn,USA,MK-81076
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,Sonic CD,segacd,USA,T-12345
",
    )
    .unwrap();

    let plan = scan_and_plan(
        input.to_string_lossy().to_string(),
        dir.path().join("out").to_string_lossy().to_string(),
        FrontendPreset::EsDe,
        None,
        None,
        Some(dat.to_string_lossy().to_string()),
        None,
        None,
        None, // needle_base_url
    None,
    None,
    )
    .await
    .unwrap();

    let title = |name: &str| {
        plan.games
            .iter()
            .find(|game| game.canonical_title == name)
            .unwrap_or_else(|| panic!("missing DAT title {name}: {:?}", plan.games))
    };
    let shenmue = title("Shenmue");
    assert_eq!(shenmue.platform, Platform::Dreamcast);
    assert_eq!(shenmue.source, ClassificationSource::RedumpCache);
    let panzer = title("Panzer Dragoon");
    assert_eq!(panzer.platform, Platform::Saturn);
    assert_eq!(panzer.source, ClassificationSource::RedumpCache);
    let sonic = title("Sonic CD");
    assert_eq!(sonic.platform, Platform::SegaCd);
    assert_eq!(sonic.source, ClassificationSource::RedumpCache);
    assert!(plan.games.iter().all(|game| {
        game.canonical_title != "Wrong Name" && game.canonical_title != "Mystery" && game.canonical_title != "not-sonic"
    }));
}

#[tokio::test]
async fn test_renamed_title_writes_the_chd_m3u_and_png_shown_before_execute() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let mut buf = [0u8; 2048];
            let _ = socket.read(&mut buf).await;
            let body = [0x89, b'P', b'N', b'G', 1, 2, 3, 4];
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: image/png\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = socket.write_all(header.as_bytes()).await;
            let _ = socket.write_all(&body).await;
        }
    });

    let dir = tempdir().unwrap();
    let input = dir.path().join("in");
    let output = dir.path().join("out");
    std::fs::create_dir_all(&input).unwrap();
    let cue1 = input.join("old-1.cue");
    let cue2 = input.join("old-2.cue");
    std::fs::write(&cue1, b"FILE \"a.bin\" BINARY\n  TRACK 01 MODE1/2352\n").unwrap();
    std::fs::write(&cue2, b"FILE \"b.bin\" BINARY\n  TRACK 01 MODE1/2352\n").unwrap();
    std::fs::write(input.join("a.bin"), b"aaa").unwrap();
    std::fs::write(input.join("b.bin"), b"bbb").unwrap();
    let stale_png = output.join("Old Name (USA).png");
    let game = PlannedGame {
        id: "old".to_string(),
        canonical_title: "Old Name".to_string(),
        platform: Platform::Psx,
        region: "USA".to_string(),
        is_multidisc: true,
        discs: vec![
            PlannedDisc {
                disc_number: 1,
                source_descriptor: cue1,
                target_chd_path: output.join("Old Name (USA) (Disc 1).chd"),
                status: TaskStatus::Pending,
                binary_tracks: vec![input.join("a.bin")],
                chdman_command: "createcd".to_string(),
                relative_m3u_entry: None,
            },
            PlannedDisc {
                disc_number: 2,
                source_descriptor: cue2,
                target_chd_path: output.join("Old Name (USA) (Disc 2).chd"),
                status: TaskStatus::Pending,
                binary_tracks: vec![input.join("b.bin")],
                chdman_command: "createcd".to_string(),
                relative_m3u_entry: None,
            },
        ],
        target_m3u_path: Some(output.join("Old Name (USA).m3u")),
        confidence: 1.0,
        source: ClassificationSource::RedumpCache,
        enabled: true,
        needs_review: false,
        status_note: None,
        role: String::new(),
        artwork_url: None,
        target_media_paths: vec![stale_png.clone()],
    };
    let mut renamed = rename_planned_game(
        game,
        "New: Title".to_string(),
        output.to_string_lossy().to_string(),
        FrontendPreset::EsDe,
        Some(MediaOptions::default()),
    );
    let stem = "New Title (USA)";
    let stem_sanitized = "New_ Title (USA)";
    assert!(
        renamed.discs[0]
            .target_chd_path
            .to_string_lossy()
            .replace('\\', "/")
            .ends_with(format!(".discs/{stem} (Disc 1).chd").as_str()),
        "{}",
        renamed.discs[0].target_chd_path.display()
    );
    assert!(
        renamed.discs[1]
            .target_chd_path
            .to_string_lossy()
            .replace('\\', "/")
            .ends_with(format!(".discs/{stem} (Disc 2).chd").as_str()),
        "{}",
        renamed.discs[1].target_chd_path.display()
    );
    let m3u = renamed.target_m3u_path.clone().expect("multi-disc m3u");
    assert!(
        m3u.to_string_lossy()
            .replace('\\', "/")
            .ends_with(format!("{stem}.m3u").as_str()),
        "{}",
        m3u.display()
    );
    assert_eq!(renamed.target_media_paths.len(), 1);
    assert!(
        renamed.target_media_paths[0]
            .to_string_lossy()
            .replace('\\', "/")
            .ends_with(format!("covers/{stem_sanitized}.png").as_str()),
        "media path {}",
        renamed.target_media_paths[0].display()
    );
    assert!(!renamed.discs[0].target_chd_path.to_string_lossy().contains("Old Name"));
    renamed.artwork_url = Some(format!("http://127.0.0.1:{port}/cover.png"));
    let shown_chd = renamed.discs[0].target_chd_path.clone();
    let shown_png = renamed.target_media_paths[0].clone();
    let shown_m3u = m3u.clone();
    let plan = IngestionPlan {
        input_dir: input,
        output_dir: output,
        preset: FrontendPreset::EsDe,
        games: vec![renamed],
        skipped_sources: Vec::new(),
        total_source_bytes: 6,
        estimated_output_bytes: 1,
    };
    let summary = execute_plan_internal(
        &MockEventSink::new(),
        plan,
        Some(ChdmanRunner::new(Some(mock_chdman()))),
        Some(2),
    )
    .await
    .unwrap();
    assert_eq!(summary.successful_games, 1);
    assert!(shown_chd.is_file(), "written CHD {}", shown_chd.display());
    assert!(shown_m3u.is_file(), "written M3U {}", shown_m3u.display());
    assert!(shown_png.is_file(), "written PNG {}", shown_png.display());
    assert!(!stale_png.exists());
    server.abort();
}
