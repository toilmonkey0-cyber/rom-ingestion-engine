use std::fs::{self, File};
use std::io::Write;
use tempfile::tempdir;
use rom_ingest_core::scanner::cue_parser::parse_cue_references;
use rom_ingest_core::scanner::{scan_directory, ScannerError};
use rom_ingest_core::models::Platform;

#[test]
fn test_parse_cue_single_and_multitrack() {
    let single_track_cue = r#"
FILE "Game (USA).bin" BINARY
  TRACK 01 MODE2/2352
    INDEX 01 00:00:00
"#;
    let refs = parse_cue_references(single_track_cue);
    assert_eq!(refs, vec!["Game (USA).bin"]);

    let multi_track_cue = r#"
FILE "Ridge Racer (USA) (Track 1).bin" BINARY
  TRACK 01 MODE2/2352
    INDEX 01 00:00:00
FILE "Ridge Racer (USA) (Track 2).bin" BINARY
  TRACK 02 AUDIO
    INDEX 00 00:00:00
    INDEX 01 00:02:00
"#;
    let refs = parse_cue_references(multi_track_cue);
    assert_eq!(refs, vec!["Ridge Racer (USA) (Track 1).bin", "Ridge Racer (USA) (Track 2).bin"]);
}

#[test]
fn test_scan_directory_discovers_cue_and_pairs_bins() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let cue_path = root.join("Game (USA).cue");
    let bin_path = root.join("Game (USA).bin");

    let mut bin = File::create(&bin_path).unwrap();
    bin.write_all(b"dummy binary data for disc").unwrap();

    let mut cue = File::create(&cue_path).unwrap();
    write!(cue, "FILE \"Game (USA).bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n").unwrap();

    let discs = scan_directory(root).expect("scan should succeed").fingerprints;
    assert_eq!(discs.len(), 1);
    assert_eq!(discs[0].primary_file, cue_path);
    assert_eq!(discs[0].binary_tracks.len(), 1);
    assert_eq!(discs[0].binary_tracks[0], bin_path);
    assert!(discs[0].total_bytes > 0);
    assert!(discs[0].calculated_sha1.is_some());
}

#[test]
fn test_scan_directory_gdi_disc() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let gdi_path = root.join("disc.gdi");
    let t1_path = root.join("track01.bin");
    let t2_path = root.join("track02.raw");
    let t3_path = root.join("track03.bin");

    File::create(&t1_path).unwrap().write_all(b"track 1 data").unwrap();
    File::create(&t2_path).unwrap().write_all(b"track 2 audio data").unwrap();
    File::create(&t3_path).unwrap().write_all(b"track 3 high density data").unwrap();

    let gdi_content = "3\n1 0 4 2352 \"track01.bin\" 0\n2 450 0 2352 \"track02.raw\" 0\n3 45000 4 2352 \"track03.bin\" 0\n";
    File::create(&gdi_path).unwrap().write_all(gdi_content.as_bytes()).unwrap();

    let discs = scan_directory(root).expect("gdi scan should succeed").fingerprints;
    assert_eq!(discs.len(), 1);
    assert_eq!(discs[0].primary_file, gdi_path);
    assert_eq!(discs[0].binary_tracks.len(), 3);
    assert_eq!(discs[0].binary_tracks[0], t1_path);
    assert_eq!(discs[0].binary_tracks[1], t2_path);
    assert_eq!(discs[0].binary_tracks[2], t3_path);
    assert_eq!(discs[0].detected_platform, Platform::Dreamcast);
}

#[test]
fn test_scan_directory_standalone_iso() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let iso_path = root.join("standalone_game.iso");
    File::create(&iso_path).unwrap().write_all(b"iso binary content").unwrap();

    let discs = scan_directory(root).expect("iso scan should succeed").fingerprints;
    assert_eq!(discs.len(), 1);
    assert_eq!(discs[0].primary_file, iso_path);
    assert_eq!(discs[0].binary_tracks, vec![iso_path]);
    assert!(discs[0].total_bytes > 0);
}

#[test]
fn test_scan_directory_platform_detection_from_folder_hints() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let folders_and_platforms = vec![
        ("psx", Platform::Psx),
        ("ps1", Platform::Psx),
        ("PlayStation", Platform::Psx),
        ("saturn", Platform::Saturn),
        ("Sega Saturn", Platform::Saturn),
        ("dreamcast", Platform::Dreamcast),
        ("dc", Platform::Dreamcast),
        ("segacd", Platform::SegaCd),
        ("Sega CD", Platform::SegaCd),
        ("pcecd", Platform::PceCd),
        ("TurboGrafx-CD", Platform::PceCd),
        ("unrecognized_system", Platform::Unknown),
    ];

    for (folder, expected_platform) in folders_and_platforms {
        let sub = root.join(folder);
        fs::create_dir_all(&sub).unwrap();
        let cue = sub.join("Game.cue");
        let bin = sub.join("Game.bin");
        File::create(&bin).unwrap().write_all(b"game data").unwrap();
        File::create(&cue).unwrap().write_all(b"FILE \"Game.bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n").unwrap();

        let discs = scan_directory(&sub).expect("scan subfolder").fingerprints;
        assert_eq!(discs.len(), 1, "Failed for folder: {}", folder);
        assert_eq!(discs[0].detected_platform, expected_platform, "Failed for folder: {}", folder);
    }
}

#[test]
fn test_scan_directory_missing_track_skips_instead_of_aborting() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // One broken sheet (missing track) and one good one: the scan must still
    // return the good disc and report the broken sheet as skipped.
    let bad_cue = root.join("Missing.cue");
    File::create(&bad_cue)
        .unwrap()
        .write_all(b"FILE \"DoesNotExist.bin\" BINARY\n  TRACK 01 MODE2/2352\n")
        .unwrap();

    let good_cue = root.join("Good.cue");
    let good_bin = root.join("Good.bin");
    File::create(&good_bin).unwrap().write_all(b"good data").unwrap();
    File::create(&good_cue)
        .unwrap()
        .write_all(b"FILE \"Good.bin\" BINARY\n  TRACK 01 MODE2/2352\n")
        .unwrap();

    let result = scan_directory(root).expect("scan should succeed despite broken sheet");
    assert_eq!(result.fingerprints.len(), 1);
    assert_eq!(result.fingerprints[0].primary_file, good_cue);
    assert_eq!(result.skipped.len(), 1);
    assert_eq!(result.skipped[0].descriptor, bad_cue);
    assert!(result.skipped[0].reason.contains("Referenced track not found"));
}

#[test]
fn test_scan_directory_skips_track_references_escaping_root() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let library = root.join("library");
    fs::create_dir_all(&library).unwrap();

    // A hostile cue that references a file outside the scanned root.
    let outside_bin = root.join("outside.bin");
    File::create(&outside_bin).unwrap().write_all(b"outside data").unwrap();

    let cue_path = library.join("Evil.cue");
    File::create(&cue_path)
        .unwrap()
        .write_all(b"FILE \"../outside.bin\" BINARY\n  TRACK 01 MODE2/2352\n")
        .unwrap();

    let result = scan_directory(&library).expect("scan should succeed");
    assert!(
        result.fingerprints.is_empty(),
        "escaping reference must not produce a disc"
    );
    assert_eq!(result.skipped.len(), 1);
    assert_eq!(result.skipped[0].descriptor, cue_path);
    assert!(result.skipped[0].reason.contains("escapes the scan root"));
}

#[test]
fn test_sha1_calculation_matches_known_hash() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let cue_path = root.join("HashTest.cue");
    let bin_path = root.join("HashTest.bin");

    // "hello world\n" has known SHA-1: 22596363b3dec40b06f75e16ac730f4a829e332a
    File::create(&bin_path).unwrap().write_all(b"hello world\n").unwrap();
    File::create(&cue_path).unwrap().write_all(b"FILE \"HashTest.bin\" BINARY\n  TRACK 01 MODE1/2352\n").unwrap();

    let discs = scan_directory(root).expect("scan should succeed").fingerprints;
    assert_eq!(discs.len(), 1);
    assert_eq!(discs[0].calculated_sha1, Some("22596363b3de40b06f981fb85d82312e8c0ed511".to_string()));
}

#[test]
fn test_cue_parser_helpers_and_cuesheet_struct() {
    use rom_ingest_core::scanner::cue_parser::{parse_cue_content, parse_gdi_references, CueSheet};

    let cue_text = "FILE \"Track1.bin\" BINARY\nFILE \"Track2.bin\" BINARY\n";
    let sheet = CueSheet::parse(cue_text);
    assert_eq!(sheet.files, vec!["Track1.bin", "Track2.bin"]);

    let dir = tempdir().unwrap();
    let root = dir.path();
    let t1 = root.join("Track1.bin");
    let t2 = root.join("Track2.bin");
    File::create(&t1).unwrap().write_all(b"111").unwrap();
    File::create(&t2).unwrap().write_all(b"2222").unwrap();

    let resolved = parse_cue_content(cue_text, root).expect("should resolve tracks");
    assert_eq!(resolved, vec![t1, t2]);

    let bad_text = "FILE \"MissingTrack.bin\" BINARY\n";
    let err = parse_cue_content(bad_text, root);
    assert!(matches!(err, Err(ScannerError::MissingTrack(_))));

    let gdi_text = "2\n1 0 4 2352 track1.bin 0\n2 100 0 2352 \"track2 with spaces.raw\" 0\n";
    let gdi_refs = parse_gdi_references(gdi_text);
    assert_eq!(gdi_refs, vec!["track1.bin", "track2 with spaces.raw"]);
}

#[test]
fn test_multi_track_total_bytes_and_iso_deduplication() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let cue_path = root.join("Multi.cue");
    let t1_path = root.join("Track 1.iso");
    let t2_path = root.join("Track 2.bin");

    File::create(&t1_path).unwrap().write_all(&vec![0u8; 1000]).unwrap();
    File::create(&t2_path).unwrap().write_all(&vec![0u8; 2500]).unwrap();

    let cue_content = "FILE \"Track 1.iso\" BINARY\n  TRACK 01 MODE1/2048\nFILE \"Track 2.bin\" BINARY\n  TRACK 02 AUDIO\n";
    File::create(&cue_path).unwrap().write_all(cue_content.as_bytes()).unwrap();

    let discs = scan_directory(root).expect("scan should succeed").fingerprints;
    // Track 1.iso should NOT be treated as a standalone disc
    assert_eq!(discs.len(), 1);
    assert_eq!(discs[0].primary_file, cue_path);
    assert_eq!(discs[0].binary_tracks.len(), 2);
    assert_eq!(discs[0].total_bytes, 3500);
}

#[test]
fn test_scan_directory_non_existent_and_empty() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let non_existent = root.join("non_existent_folder");
    let err = scan_directory(&non_existent);
    assert!(matches!(err, Err(ScannerError::Io(_))));

    let empty_dir = root.join("empty_folder");
    fs::create_dir(&empty_dir).unwrap();
    let discs = scan_directory(&empty_dir).expect("empty scan should succeed").fingerprints;
    assert!(discs.is_empty());
}
