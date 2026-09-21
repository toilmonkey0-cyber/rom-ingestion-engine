use std::fs;
use std::path::Path;
use tempfile::tempdir;

use rom_ingest_core::commands::MockEventSink;
use rom_ingest_core::migrator::{execute_migration, plan_migration, MigrationItemKind};
use rom_ingest_core::models::FrontendPreset;

fn write(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, bytes).unwrap();
}

#[test]
fn test_plan_rejects_bad_input() {
    let err = plan_migration(Path::new("x"), FrontendPreset::Batocera, FrontendPreset::Batocera, None).unwrap_err();
    assert!(err.contains("identical"));

    let dir = tempdir().unwrap();
    let err = plan_migration(&dir.path().join("missing"), FrontendPreset::OnionOs, FrontendPreset::EsDe, None).unwrap_err();
    assert!(err.contains("not a directory"));
}

#[test]
fn test_migrate_onion_to_batocera_multidisc_with_art_and_gamelist() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("sdcard");
    // OnionOS layout
    let old_ps = root.join("Roms").join("PS");
    write(&old_ps.join(".discs").join("Game (USA) (Disc 1).chd"), b"chd1");
    write(&old_ps.join(".discs").join("Game (USA) (Disc 2).chd"), b"chd2");
    write(
        &old_ps.join("Game (USA).m3u"),
        b".discs/Game (USA) (Disc 1).chd\n.discs/Game (USA) (Disc 2).chd\n",
    );
    write(&old_ps.join("Game (USA).png"), b"pngdata");

    let plan = plan_migration(&root, FrontendPreset::OnionOs, FrontendPreset::Batocera, None)
        .expect("plan");
    assert_eq!(plan.games, 1);
    assert_eq!(
        plan.playlist_rewrites.len(),
        0,
        "subfolder is .discs in both presets; entries stay identical"
    );
    assert!(plan.items.iter().any(|i| i.kind == MigrationItemKind::Chd && i.source.ends_with("Game (USA) (Disc 1).chd")));
    assert!(plan.items.iter().any(|i| i.kind == MigrationItemKind::Playlist));
    assert!(plan.items.iter().any(|i| i.kind == MigrationItemKind::Artwork));

    let emitter = MockEventSink::new();
    let summary = execute_migration(&emitter, &plan).expect("execute");
    assert_eq!(summary.files_moved, 4); // 2 chd + m3u + art
    assert_eq!(summary.gamelists_written, 1);
    assert!(summary.skipped_existing.is_empty());

    // New Batocera layout
    let new_ps = root.join("roms").join("psx");
    assert!(new_ps.join(".discs").join("Game (USA) (Disc 1).chd").is_file());
    assert!(new_ps.join(".discs").join("Game (USA) (Disc 2).chd").is_file());
    assert!(new_ps.join("Game (USA).m3u").is_file());
    // Art moved into the gamelist media convention
    assert!(new_ps.join("media").join("images").join("Game (USA).png").is_file());
    assert!(!old_ps.join("Game (USA).m3u").exists());
    assert!(!old_ps.join("Game (USA).png").exists());

    // Gamelist references the playlist and the relocated art
    let xml = fs::read_to_string(new_ps.join("gamelist.xml")).unwrap();
    assert!(xml.contains("<path>./Game (USA).m3u</path>"));
    assert!(xml.contains("<image>./media/images/Game (USA).png</image>"));

    // Migration progress events emitted
    let events = emitter.migration_events.lock().unwrap().clone();
    assert_eq!(events.len(), 4);
    assert_eq!(events.last().unwrap().completed, 4);
}

#[test]
fn test_migrate_esde_to_anbernic_single_disc() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("card");
    let old_ps = root.join("roms").join("psx");
    write(&old_ps.join("Crash Bandicoot (USA).chd"), b"chd");
    write(&old_ps.join("media").join("images").join("Crash Bandicoot (USA).png"), b"art");
    write(&old_ps.join("gamelist.xml"), b"<gameList/>");

    let plan = plan_migration(&root, FrontendPreset::EsDe, FrontendPreset::AnbernicStock, None)
        .expect("plan");
    assert_eq!(plan.games, 1);

    let emitter = MockEventSink::new();
    let summary = execute_migration(&emitter, &plan).expect("execute");

    // Anbernic is not a gamelist preset: no gamelist written, art placed
    // adjacent to the ROM for auto-loading frontends.
    assert_eq!(summary.gamelists_written, 0);
    let new_ps = root.join("ROMS").join("PS");
    assert!(new_ps.join("Crash Bandicoot (USA).chd").is_file());
    assert!(new_ps.join("Crash Bandicoot (USA).png").is_file());
    assert!(!new_ps.join("gamelist.xml").exists());
    assert!(!old_ps.join("gamelist.xml").exists(), "stale gamelist removed");
    assert!(!old_ps.join("Crash Bandicoot (USA).chd").exists());
}

#[test]
fn test_migration_skips_existing_targets_without_overwriting() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("card");
    let old_ps = root.join("roms").join("psx");
    write(&old_ps.join("Solo.chd"), b"original");

    let plan = plan_migration(&root, FrontendPreset::EsDe, FrontendPreset::OnionOs, None).expect("plan");

    // Pre-create the target so the move collides.
    let new_ps = root.join("Roms").join("PS");
    write(&new_ps.join("Solo.chd"), b"already here");

    let emitter = MockEventSink::new();
    let summary = execute_migration(&emitter, &plan).expect("execute");
    assert_eq!(summary.skipped_existing.len(), 1);
    assert_eq!(fs::read(new_ps.join("Solo.chd")).unwrap(), b"already here");
    assert!(old_ps.join("Solo.chd").exists(), "source untouched on collision");
}

#[test]
fn test_migration_is_idempotent_on_rerun() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("card");
    let old_ps = root.join("roms").join("psx");
    write(&old_ps.join("Solo.chd"), b"x");

    let plan = plan_migration(&root, FrontendPreset::EsDe, FrontendPreset::OnionOs, None).expect("plan");
    let emitter = MockEventSink::new();
    execute_migration(&emitter, &plan).expect("first run");

    // Re-running the same plan is a no-op (sources already gone).
    let summary2 = execute_migration(&emitter, &plan).expect("second run");
    assert_eq!(summary2.files_moved, 0);
    assert!(root.join("Roms").join("PS").join("Solo.chd").is_file());
}

#[test]
fn test_migration_handles_root_relative_entries_and_reports_broken_playlists() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("card");
    // Real-world Anbernic card shape: playlist entries are root-relative
    // ("/_hidden/multi-disc/...") and — as found on actual cards — some
    // referenced CHDs are missing entirely (dead playlists).
    let ps = root.join("Roms").join("PS");
    write(
        &ps.join("Healthy (USA).m3u"),
        b"/_hidden/multi-disc/Healthy (USA) (Disc 1).chd\n/_hidden/multi-disc/Healthy (USA) (Disc 2).chd\n",
    );
    write(&root.join("_hidden").join("multi-disc").join("Healthy (USA) (Disc 1).chd"), b"a");
    write(&root.join("_hidden").join("multi-disc").join("Healthy (USA) (Disc 2).chd"), b"b");
    write(
        &ps.join("Dead Game (USA).m3u"),
        b"/_hidden/multi-disc/Dead Game (USA) (Disc 1).chd\n",
    );

    let plan = plan_migration(&root, FrontendPreset::AnbernicStock, FrontendPreset::Batocera, None)
        .expect("plan");

    // The healthy playlist's CHDs are found via root-relative resolution.
    assert!(plan
        .items
        .iter()
        .any(|i| i.kind == MigrationItemKind::Chd
            && i.source.ends_with("Healthy (USA) (Disc 1).chd")));
    // Entries are rewritten to the target subfolder convention.
    assert_eq!(plan.playlist_rewrites.len(), 2);

    // The dead playlist is reported, not silently dropped.
    assert_eq!(plan.broken_playlists.len(), 1);
    assert!(plan.broken_playlists[0].playlist.ends_with("Dead Game (USA).m3u"));
    assert_eq!(plan.broken_playlists[0].missing_entries.len(), 1);

    let emitter = MockEventSink::new();
    execute_migration(&emitter, &plan).expect("execute");
    let new_ps = root.join("roms").join("psx");
    assert!(new_ps.join(".discs").join("Healthy (USA) (Disc 1).chd").is_file());
    let m3u = fs::read_to_string(new_ps.join("Healthy (USA).m3u")).unwrap();
    assert!(m3u.contains(".discs/Healthy (USA) (Disc 1).chd"));
    assert!(!m3u.contains("_hidden"), "entries must be rewritten, not copied");
}
