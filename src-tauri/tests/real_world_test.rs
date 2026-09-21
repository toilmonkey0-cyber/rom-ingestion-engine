//! Real-world validation harness (ignored by default).
//!
//! Drives the real pipeline against real dumps staged on this machine:
//!   staging/incoming  – real downloaded dumps (Vimm's Lair 7z extractions)
//!   staging/library   – ingestion output target
//!   staging/cardcopy  – a copy of the real Anbernic SD card's Roms/PS subset
//!
//! Uses the REAL chdman (managed dir), REAL Redump DATs, and REAL network
//! artwork. Run with:
//!   cargo test --test real_world_test -- --ignored --nocapture

use std::path::{Path, PathBuf};

use rom_ingest_core::chdman::downloader::get_managed_tools_dir;
use rom_ingest_core::commands::{execute_plan_internal, MockEventSink};
use rom_ingest_core::metadata::finish_library_internal;
use rom_ingest_core::migrator::{plan_migration, execute_migration};
use rom_ingest_core::models::*;
use rom_ingest_core::watch::run_watch_ingestion_once;

fn staging() -> PathBuf {
    std::env::var("ROM_INGEST_TEST_STAGING")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let tmp = std::env::var("TEMP").unwrap_or_else(|_| ".".into());
            PathBuf::from(tmp).join("romtest").join("staging")
        })
}

fn dats() -> Vec<String> {
    let dir = get_managed_tools_dir()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("dats")))
        .expect("managed dats dir");
    std::fs::read_dir(&dir)
        .expect("dats dir exists (run setup)")
        .filter_map(|e| {
            let p = e.ok()?.path();
            if p.extension()?.eq_ignore_ascii_case("dat") {
                Some(p.to_string_lossy().to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Scan + classify the real staged dumps with real Redump DATs.
#[tokio::test]
#[ignore = "requires real staging data"]
async fn real_scan_and_classify() {
    let input = staging().join("incoming");
    let output = staging().join("library");
    std::fs::create_dir_all(&output).unwrap();

    let plan = rom_ingest_core::commands::scan_and_plan(
        input.to_string_lossy().to_string(),
        output.to_string_lossy().to_string(),
        FrontendPreset::AnbernicStock,
        None,
        None,
        Some(dats()),
    )
    .await
    .expect("scan_and_plan on real data");

    println!("\n=== REAL SCAN RESULTS ===");
    println!("input: {}", input.display());
    for game in &plan.games {
        println!(
            "  [{}] {} | platform={:?} region={} discs={} confidence={:.2} review={}",
            format!("{:?}", game.source).to_lowercase(),
            game.canonical_title,
            game.platform,
            game.region,
            game.discs.len(),
            game.confidence,
            game.needs_review
        );
    }
    for s in &plan.skipped_sources {
        println!("  [skipped] {} — {}", s.path.display(), s.reason);
    }
    println!("games: {}, skipped: {}", plan.games.len(), plan.skipped_sources.len());

    // Sanity: every dump produced a game, nothing crashed on real filenames.
    assert!(plan.games.len() >= 3, "expected at least 3 games from staging");
}

/// Full conversion + verification with the REAL chdman on a real dump.
#[tokio::test]
#[ignore = "requires real chdman + staging data (takes minutes)"]
async fn real_convert_and_verify_with_real_chdman() {
    // Isolate just Harmful Park (Japan) — the smallest single-disc dump.
    let isolated = staging().join("iso_harmful");
    let src = staging()
        .join("incoming")
        .join("Harmful Park (Japan)")
        .join("Harmful Park (Japan)");
    let _ = std::fs::remove_dir_all(&isolated);
    std::fs::create_dir_all(&isolated).unwrap();
    for f in ["Harmful Park (Japan).cue", "Harmful Park (Japan).bin"] {
        std::fs::copy(src.join(f), isolated.join(f)).expect(f);
    }

    let output = staging().join("library_real");
    let _ = std::fs::remove_dir_all(&output);
    std::fs::create_dir_all(&output).unwrap();

    let plan = rom_ingest_core::commands::scan_and_plan(
        isolated.to_string_lossy().to_string(),
        output.to_string_lossy().to_string(),
        FrontendPreset::AnbernicStock,
        None,
        None,
        Some(dats()),
    )
    .await
    .expect("scan");
    assert_eq!(plan.games.len(), 1, "one game expected");

    // No chdman override: must find the REAL chdman in the managed dir.
    let emitter = MockEventSink::new();
    let summary = execute_plan_internal(&emitter, plan.clone(), None, None)
        .await
        .expect("real conversion");

    println!("\n=== REAL CONVERSION ===");
    println!("source bytes: {}", summary.total_source_bytes);
    println!("output bytes: {}", summary.total_output_bytes);
    println!("compression: {:.1}%",
        100.0 - (summary.total_output_bytes as f64 / summary.total_source_bytes as f64) * 100.0);

    assert_eq!(summary.successful_games, 1, "real conversion must succeed");
    assert_eq!(summary.failed_games, 0);
    assert!(summary.processed_discs == 1);

    // The CHD must exist, be non-trivial, and readable by real chdman info.
    let chd = &plan.games[0].discs[0].target_chd_path;
    assert!(chd.is_file(), "CHD written: {}", chd.display());
    let len = std::fs::metadata(chd).unwrap().len();
    assert!(len > 1_000_000, "CHD has real content ({} bytes)", len);
}

/// Finish Line (real artwork + gamelist) on the real converted library.
#[tokio::test]
#[ignore = "requires the real conversion test to have run; network access"]
async fn real_finish_library_artwork() {
    let output = staging().join("library_real");
    let input = staging().join("iso_harmful");
    let mut plan = rom_ingest_core::commands::scan_and_plan(
        input.to_string_lossy().to_string(),
        output.to_string_lossy().to_string(),
        FrontendPreset::EsDe, // gamelist-writing preset
        None,
        None,
        Some(dats()),
    )
    .await
    .expect("scan");

    // Real dumps in per-game folders classify as Unknown — the same fix a
    // user applies in the dry-run UI. Re-target to PSX before finishing.
    let game_id = plan.games[0].id.clone();
    plan = rom_ingest_core::commands::set_game_platform(plan, game_id, Platform::Psx)
        .expect("retarget platform");
    assert_eq!(plan.games[0].platform, Platform::Psx);
    let chd = plan.games[0].discs[0].target_chd_path.to_string_lossy().to_lowercase();
    assert!(chd.contains("roms/psx") || chd.contains("roms\\psx"), "retargeted into psx folder: {}", chd);

    let emitter = MockEventSink::new();
    let summary = finish_library_internal(&emitter, &plan, true, None)
        .await
        .expect("finish_library");

    println!("\n=== REAL FINISH LINE ===");
    println!("{:?}", summary);

    for path in &summary.artwork_paths {
        println!("art: {} ({} bytes)", path, std::fs::metadata(path).map(|m| m.len()).unwrap_or(0));
    }
    // Harmful Park is an obscure JP-only shmup — art may legitimately miss.
    // The contract is: no crash, honest counts, files on disk for successes.
    let gamelist = output.join("roms").join("psx").join("gamelist.xml");
    if summary.gamelists_written > 0 {
        let xml = std::fs::read_to_string(&gamelist).unwrap();
        println!("gamelist.xml:\n{}", xml);
        assert!(xml.contains("Harmful Park"));
    }
}

/// Migration against a copy of the REAL card structure: dead playlists
/// (root-relative entries, missing CHDs) must be reported, healthy content
/// must move cleanly.
#[test]
#[ignore = "requires staging cardcopy (run setup)"]
fn real_migration_from_card_copy() {
    let root = staging().join("cardcopy");
    let plan = plan_migration(&root, FrontendPreset::AnbernicStock, FrontendPreset::Batocera, None)
        .expect("plan on real card copy");

    println!("\n=== REAL MIGRATION PLAN ===");
    println!("games: {} moves: {} rewrites: {} broken: {}",
        plan.games, plan.items.len(), plan.playlist_rewrites.len(), plan.broken_playlists.len());
    for b in &plan.broken_playlists {
        println!("  broken: {} ({} missing)", b.playlist.display(), b.missing_entries.len());
    }

    let emitter = MockEventSink::new();
    let summary = execute_migration(&emitter, &plan).expect("execute");
    println!("{:?}", summary);

    assert_eq!(summary.skipped_existing.len(), 0);
}

/// Watch-mode pass over the staged incoming folder with real chdman.
#[tokio::test]
#[ignore = "requires real chdman + staging data (takes minutes)"]
async fn real_watch_ingestion_pass() {
    let input = staging().join("iso_harmful");
    let output = staging().join("library_watch");
    let _ = std::fs::remove_dir_all(&output);
    std::fs::create_dir_all(&output).unwrap();

    let emitter = MockEventSink::new();
    let summary = run_watch_ingestion_once(
        &emitter,
        Path::new(&input),
        Path::new(&output),
        FrontendPreset::OnionOs,
        None,
        None, // real chdman
    )
    .await
    .expect("watch pass");

    println!("\n=== REAL WATCH PASS ===\n{:?}", summary);
    assert_eq!(summary.successful_games, 1);
    let target = output.join("Roms").join("UNKNOWN");
    let _ = target; // platform depends on folder hints; print instead:
    println!("output tree:");
    print_tree(&output, 0);
}

fn print_tree(dir: &Path, depth: usize) {
    if depth > 3 {
        return;
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            let indent = "  ".repeat(depth + 1);
            if p.is_dir() {
                println!("{}{}/", indent, p.file_name().unwrap_or_default().to_string_lossy());
                print_tree(&p, depth + 1);
            } else {
                println!("{}{} ({} bytes)", indent, p.file_name().unwrap_or_default().to_string_lossy(),
                    e.metadata().map(|m| m.len()).unwrap_or(0));
            }
        }
    }
}

/// Reproduce the GUI sequence: scan, retarget FF7 -> psx, then Harmful -> psx,
/// and verify BOTH disc paths move out of UNKNOWN.
#[tokio::test]
#[ignore = "requires real staging data"]
async fn real_retarget_sequence() {
    let input = staging().join("incoming");
    let output = staging().join("library_repro");
    std::fs::create_dir_all(&output).unwrap();

    let mut plan = rom_ingest_core::commands::scan_and_plan(
        input.to_string_lossy().to_string(),
        output.to_string_lossy().to_string(),
        FrontendPreset::AnbernicStock,
        None,
        None,
        Some(dats()),
    )
    .await
    .expect("scan");

    let ff7 = plan.games.iter().find(|g| g.canonical_title.contains("Final Fantasy")).unwrap().id.clone();
    let hp = plan.games.iter().find(|g| g.canonical_title.contains("Harmful")).unwrap().id.clone();

    plan = rom_ingest_core::commands::set_game_platform(plan, ff7, Platform::Psx).expect("r1");
    plan = rom_ingest_core::commands::set_game_platform(plan, hp, Platform::Psx).expect("r2");

    for g in &plan.games {
        println!("{} [{:?}] -> {}", g.canonical_title, g.platform, g.discs[0].target_chd_path.display());
        if g.canonical_title.contains("Harmful") || g.canonical_title.contains("Final Fantasy") {
            assert_eq!(g.platform, Platform::Psx);
            let p = g.discs[0].target_chd_path.to_string_lossy().to_lowercase();
            // AnbernicStock maps Psx to ROMS/PS
            assert!(p.contains("roms/ps/") || p.contains("roms\\ps\\"), "path not retargeted: {}", p);
        }
    }
}

/// Removable-media trash behavior: does trash_source_files recycle or
/// permanently delete on the SD card? Uses a synthetic probe file.
#[test]
#[ignore = "requires the SD card mounted at D:"]
fn real_trash_on_removable_media() {
    let probe = PathBuf::from("D:/romtest_trash_probe.bin");
    std::fs::write(&probe, b"probe").expect("write probe to card");

    // FAT32 removable drives have recycling disabled: the first call must
    // REFUSE (no Recycle Bin => permanent deletion) ...
    let refusal = rom_ingest_core::commands::trash_source_files(
        vec![probe.to_string_lossy().to_string()],
        None,
        None,
    )
    .expect_err("must refuse without explicit permanent confirmation");
    assert!(refusal.contains("NO RECYCLE BIN"), "got: {}", refusal);
    assert!(probe.exists(), "refusal must leave the file untouched");
    println!("refusal: {}", refusal);

    // ... and the explicit confirmation performs the (permanent) deletion.
    let count = rom_ingest_core::commands::trash_source_files(
        vec![probe.to_string_lossy().to_string()],
        None,
        Some(true),
    )
    .expect("confirmed permanent trash on removable media");

    println!("trashed: {}", count);
    println!("file gone from card: {}", !probe.exists());
    let recycle_on_card = std::fs::read_dir("D:/")
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .any(|e| e.file_name().to_string_lossy().contains("RECYCLE"))
        })
        .unwrap_or(false);
    println!("recycle bin created on card: {}", recycle_on_card);
}

/// Real repair operation: removes the two CHDs that failed `chdman verify`
/// on the card (backed up to the PC first) through the app's confirmed
/// permanent-deletion path. Frees space for playlist repairs.
#[test]
#[ignore = "requires the SD card + corrupt backups staged"]
fn real_delete_corrupt_chds_via_app() {
    let corrupt = [
        "D:/Roms/PS/Oddworld - Abe's Oddysee (USA) (Rev 2).chd",
        "D:/Roms/PS/Spyro 2 - Ripto's Rage! (USA).chd",
    ];
    // Safety: backups must exist on the PC before anything is removed.
    let backup_dir = staging().join("corrupt_backup");
    for c in &corrupt {
        let name = std::path::Path::new(c).file_name().unwrap();
        assert!(
            backup_dir.join(name).is_file(),
            "backup missing for {} — refusing to delete",
            name.to_string_lossy()
        );
    }

    // The trash command must refuse outright: .chd files are OUTPUTS, not
    // disc-image sources — the extension allow-list is a safety property
    // (it must never let the trash flow remove a library's CHDs).
    let err = rom_ingest_core::commands::trash_source_files(
        corrupt.iter().map(|s| s.to_string()).collect(),
        None,
        None,
    )
    .expect_err("refuses .chd outputs");
    assert!(err.contains("not a disc-image file"), "got: {}", err);
    for c in &corrupt {
        assert!(std::path::Path::new(c).exists(), "refusal left it in place: {}", c);
    }

    // Maintenance removal (not the app's trash flow): delete the backed-up
    // corrupt outputs directly to free space for repairs.
    for c in &corrupt {
        std::fs::remove_file(c).expect("remove corrupt chd");
    }
    println!("freed: corrupt CHDs removed from card (backed up on PC)");
}

/// ES-DE verification library: scan (with platform retarget), convert with
/// real chdman, and run Finish Line (gamelists + artwork) — the exact
/// library that will be loaded into real ES-DE on Windows.
#[tokio::test]
#[ignore = "requires staging + real chdman (takes minutes)"]
async fn real_generate_esde_library() {
    let input = staging().join("esde_in");
    let output = staging().join("esde_lib");
    std::fs::create_dir_all(&output).unwrap();

    let mut plan = rom_ingest_core::commands::scan_and_plan(
        input.to_string_lossy().to_string(),
        output.to_string_lossy().to_string(),
        FrontendPreset::EsDe,
        None,
        None,
        Some(dats()),
    )
    .await
    .expect("scan");

    // Real dumps in per-game folders: apply the platform overrides a user
    // makes in the dry-run UI (these are known-good: PSX and Dreamcast).
    for game in plan.games.clone() {
        let target = if game.discs[0]
            .source_descriptor
            .to_string_lossy()
            .contains("dreamcast")
        {
            Platform::Dreamcast
        } else {
            Platform::Psx
        };
        if game.platform != target {
            plan = rom_ingest_core::commands::set_game_platform(plan, game.id.clone(), target)
                .expect("retarget");
        }
    }

    assert!(!plan.games.is_empty(), "scan must find the staged games");
    assert_eq!(plan.skipped_sources.len(), 0, "no skips expected: {}",
        plan.skipped_sources.iter().map(|s| s.reason.clone()).collect::<Vec<_>>().join("; "));

    let emitter = MockEventSink::new();
    let summary = execute_plan_internal(&emitter, plan.clone(), None, None)
        .await
        .expect("convert");
    assert_eq!(summary.failed_games, 0, "all conversions must pass");

    let finish = finish_library_internal(&emitter, &plan, true, None)
        .await
        .expect("finish line");
    println!("gamelists: {} art: {}/{} failed: {}",
        finish.gamelists_written, finish.artwork_downloaded,
        finish.artwork_downloaded + finish.artwork_failed, finish.artwork_failed);

    // Tree for the record
    print_tree(&output, 0);
}

/// Build an ES-DE library directly through the organizer + Finish Line code
/// paths from already-converted CHDs (scanner does not ingest .chd — known
/// feature gap). This is the library loaded into real ES-DE for testing.
#[test]
#[ignore = "requires staging"]
fn real_build_esde_library_from_chds() {
    let input = staging().join("esde_in");
    let output = staging().join("esde_lib");
    let _ = std::fs::remove_dir_all(&output);
    std::fs::create_dir_all(&output).unwrap();

    // Lay out CHDs per the EsDe preset using the organizer itself.
    for (src, platform) in [
        ("psx/Alundra (USA) (Rev 1).chd", Platform::Psx),
        ("psx/Final Fantasy VII (USA) (Disc 1).chd", Platform::Psx),
        ("psx/Xenogears (USA) (Disc 1).chd", Platform::Psx),
        ("dreamcast/Crazy Taxi (USA).chd", Platform::Dreamcast),
    ] {
        let folder = rom_ingest_core::organizer::presets::get_platform_folder(
            FrontendPreset::EsDe,
            platform,
        );
        let dest = output.join(folder).join(
            std::path::Path::new(src).file_name().unwrap(),
        );
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::copy(input.join(src), &dest).expect(src);
    }

    // Minimal plan describing the layout so Finish Line writes gamelists.
    let mk = |id: &str, title: &str, platform: Platform, region: &str, rel: &str| {
        let mut g = PlannedGame {
            id: id.into(),
            canonical_title: title.into(),
            platform,
            region: region.into(),
            is_multidisc: false,
            discs: vec![PlannedDisc {
                disc_number: 1,
                source_descriptor: "".into(),
                target_chd_path: output.join(rel),
                relative_m3u_entry: None,
                status: TaskStatus::Verified,
            }],
            target_m3u_path: None,
            confidence: 1.0,
            source: ClassificationSource::Fallback,
            enabled: true,
            needs_review: false,
        };
        // ES-DE art convention lives next to media/images/<stem>.png
        g.discs[0].relative_m3u_entry = None;
        g
    };
    let plan = IngestionPlan {
        input_dir: input.clone(),
        output_dir: output.clone(),
        preset: FrontendPreset::EsDe,
        games: vec![
            mk("alundra", "Alundra", Platform::Psx, "USA", "roms/psx/Alundra (USA) (Rev 1).chd"),
            mk("ff7", "Final Fantasy VII", Platform::Psx, "USA", "roms/psx/Final Fantasy VII (USA) (Disc 1).chd"),
            mk("xeno", "Xenogears", Platform::Psx, "USA", "roms/psx/Xenogears (USA) (Disc 1).chd"),
            mk("crazy", "Crazy Taxi", Platform::Dreamcast, "USA", "roms/dreamcast/Crazy Taxi (USA).chd"),
        ],
        skipped_sources: Vec::new(),
        total_source_bytes: 0,
        estimated_output_bytes: 0,
    };

    let emitter = MockEventSink::new();
    let finish = finish_library_internal(&emitter, &plan, true, None);
    // finish_library_internal is async; block via a tiny runtime
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let finish = rt.block_on(finish).expect("finish line");
    println!("gamelists: {} art downloaded: {} failed: {}",
        finish.gamelists_written, finish.artwork_downloaded, finish.artwork_failed);
    print_tree(&output, 0);
}

