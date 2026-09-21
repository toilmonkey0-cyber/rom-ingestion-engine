// src-tauri/tests/plan_builder_test.rs
use std::path::PathBuf;
use rom_ingest_core::models::*;
use rom_ingest_core::plan_builder::build_ingestion_plan;

#[test]
fn test_multi_disc_plan_grouping() {
    let disc1 = DiscFingerprint {
        primary_file: PathBuf::from("in/FF7_1.cue"),
        binary_tracks: vec![PathBuf::from("in/FF7_1.bin")],
        detected_platform: Platform::Psx,
        calculated_sha1: None,
        total_bytes: 700_000_000,
    };
    let disc2 = DiscFingerprint {
        primary_file: PathBuf::from("in/FF7_2.cue"),
        binary_tracks: vec![PathBuf::from("in/FF7_2.bin")],
        detected_platform: Platform::Psx,
        calculated_sha1: None,
        total_bytes: 700_000_000,
    };

    let class1 = GameClassification {
        canonical_title: "Final Fantasy VII".to_string(),
        platform: Platform::Psx,
        region: "USA".to_string(),
        is_multidisc: true,
        disc_number: Some(1),
        total_discs: Some(3),
        confidence: 0.95,
        source: ClassificationSource::JevAI,
    };
    let class2 = GameClassification {
        canonical_title: "Final Fantasy VII".to_string(),
        platform: Platform::Psx,
        region: "USA".to_string(),
        is_multidisc: true,
        disc_number: Some(2),
        total_discs: Some(3),
        confidence: 0.95,
        source: ClassificationSource::JevAI,
    };

    let plan = build_ingestion_plan(
        PathBuf::from("in"),
        PathBuf::from("out"),
        FrontendPreset::AnbernicStock,
        None,
        vec![(disc1, class1), (disc2, class2)],
        Vec::new(),
    );

    assert_eq!(plan.games.len(), 1);
    assert_eq!(plan.games[0].discs.len(), 2);
    assert_eq!(plan.games[0].discs[0].disc_number, 1);
    assert_eq!(plan.games[0].discs[1].disc_number, 2);
    assert!(plan.games[0].target_m3u_path.is_some());
    assert_eq!(plan.preset, FrontendPreset::AnbernicStock);
    assert!(plan.estimated_output_bytes < plan.total_source_bytes);
    assert_eq!(plan.total_source_bytes, 1_400_000_000);
    assert_eq!(plan.estimated_output_bytes, 840_000_000);
    assert!(!plan.games[0].needs_review);
}

#[test]
fn test_single_disc_plan_has_no_m3u() {
    let disc = DiscFingerprint {
        primary_file: PathBuf::from("in/Tekken3.cue"),
        binary_tracks: vec![PathBuf::from("in/Tekken3.bin")],
        detected_platform: Platform::Psx,
        calculated_sha1: None,
        total_bytes: 650_000_000,
    };
    let class = GameClassification {
        canonical_title: "Tekken 3".to_string(),
        platform: Platform::Psx,
        region: "USA".to_string(),
        is_multidisc: false,
        disc_number: None,
        total_discs: None,
        confidence: 1.0,
        source: ClassificationSource::RedumpCache,
    };

    let plan = build_ingestion_plan(
        PathBuf::from("in"),
        PathBuf::from("out"),
        FrontendPreset::EsDe,
        None,
        vec![(disc, class)],
        Vec::new(),
    );

    assert_eq!(plan.games.len(), 1);
    assert_eq!(plan.games[0].discs.len(), 1);
    assert_eq!(plan.games[0].discs[0].disc_number, 1);
    assert!(plan.games[0].target_m3u_path.is_none());
    assert!(!plan.games[0].needs_review);
    assert_eq!(plan.total_source_bytes, 650_000_000);
    assert_eq!(plan.estimated_output_bytes, 390_000_000);
}

#[test]
fn test_discs_out_of_order_sorted_by_disc_number() {
    let disc2 = DiscFingerprint {
        primary_file: PathBuf::from("in/RE2_Disc2.cue"),
        binary_tracks: vec![PathBuf::from("in/RE2_Disc2.bin")],
        detected_platform: Platform::Psx,
        calculated_sha1: None,
        total_bytes: 600_000_000,
    };
    let disc1 = DiscFingerprint {
        primary_file: PathBuf::from("in/RE2_Disc1.cue"),
        binary_tracks: vec![PathBuf::from("in/RE2_Disc1.bin")],
        detected_platform: Platform::Psx,
        calculated_sha1: None,
        total_bytes: 600_000_000,
    };

    let class2 = GameClassification {
        canonical_title: "Resident Evil 2".to_string(),
        platform: Platform::Psx,
        region: "USA".to_string(),
        is_multidisc: true,
        disc_number: Some(2),
        total_discs: Some(2),
        confidence: 0.90,
        source: ClassificationSource::RedumpCache,
    };
    let class1 = GameClassification {
        canonical_title: "Resident Evil 2".to_string(),
        platform: Platform::Psx,
        region: "USA".to_string(),
        is_multidisc: true,
        disc_number: Some(1),
        total_discs: Some(2),
        confidence: 0.90,
        source: ClassificationSource::RedumpCache,
    };

    let plan = build_ingestion_plan(
        PathBuf::from("in"),
        PathBuf::from("out"),
        FrontendPreset::OnionOs,
        None,
        vec![(disc2, class2), (disc1, class1)],
        Vec::new(),
    );

    assert_eq!(plan.games.len(), 1);
    assert_eq!(plan.games[0].discs[0].disc_number, 1);
    assert_eq!(plan.games[0].discs[1].disc_number, 2);
    assert_eq!(plan.games[0].discs[0].source_descriptor, PathBuf::from("in/RE2_Disc1.cue"));
    assert_eq!(plan.games[0].discs[1].source_descriptor, PathBuf::from("in/RE2_Disc2.cue"));
}

#[test]
fn test_low_confidence_sets_needs_review() {
    let disc = DiscFingerprint {
        primary_file: PathBuf::from("in/UnknownGame.cue"),
        binary_tracks: vec![],
        detected_platform: Platform::Psx,
        calculated_sha1: None,
        total_bytes: 500_000_000,
    };
    let class = GameClassification {
        canonical_title: "Unknown Game".to_string(),
        platform: Platform::Psx,
        region: "USA".to_string(),
        is_multidisc: false,
        disc_number: None,
        total_discs: None,
        confidence: 0.72,
        source: ClassificationSource::Fallback,
    };

    let plan = build_ingestion_plan(
        PathBuf::from("in"),
        PathBuf::from("out"),
        FrontendPreset::EsDe,
        None,
        vec![(disc, class)],
        Vec::new(),
    );

    assert_eq!(plan.games.len(), 1);
    assert!(plan.games[0].needs_review);
}

#[test]
fn test_multiple_discs_infer_multidisc_even_if_flagged_false() {
    let disc1 = DiscFingerprint {
        primary_file: PathBuf::from("in/Lunar1.cue"),
        binary_tracks: vec![],
        detected_platform: Platform::SegaCd,
        calculated_sha1: None,
        total_bytes: 400_000_000,
    };
    let disc2 = DiscFingerprint {
        primary_file: PathBuf::from("in/Lunar2.cue"),
        binary_tracks: vec![],
        detected_platform: Platform::SegaCd,
        calculated_sha1: None,
        total_bytes: 400_000_000,
    };

    let class1 = GameClassification {
        canonical_title: "Lunar - The Silver Star".to_string(),
        platform: Platform::SegaCd,
        region: "USA".to_string(),
        is_multidisc: false, // flagged false erroneously
        disc_number: Some(1),
        total_discs: None,
        confidence: 0.88,
        source: ClassificationSource::Fallback,
    };
    let class2 = GameClassification {
        canonical_title: "Lunar - The Silver Star".to_string(),
        platform: Platform::SegaCd,
        region: "USA".to_string(),
        is_multidisc: false,
        disc_number: Some(2),
        total_discs: None,
        confidence: 0.88,
        source: ClassificationSource::Fallback,
    };

    let plan = build_ingestion_plan(
        PathBuf::from("in"),
        PathBuf::from("out"),
        FrontendPreset::Batocera,
        None,
        vec![(disc1, class1), (disc2, class2)],
        Vec::new(),
    );

    assert_eq!(plan.games.len(), 1);
    assert!(plan.games[0].is_multidisc);
    assert_eq!(plan.games[0].discs.len(), 2);
    assert!(plan.games[0].target_m3u_path.is_some());
}

#[test]
fn test_retarget_title_and_platform_rebuild_paths() {
    use rom_ingest_core::models::*;
    use rom_ingest_core::plan_builder::{retarget_game_platform, retarget_game_title};

    let plan = build_ingestion_plan(
        PathBuf::from("in"),
        PathBuf::from("out"),
        FrontendPreset::AnbernicStock,
        None,
        vec![(
            DiscFingerprint {
                primary_file: PathBuf::from("in/psx/Game (USA).cue"),
                binary_tracks: vec![PathBuf::from("in/psx/Game (USA).bin")],
                detected_platform: Platform::Unknown,
                calculated_sha1: None,
                total_bytes: 100,
            },
            GameClassification {
                canonical_title: "Wrong Title".into(),
                platform: Platform::Unknown,
                region: "USA".into(),
                is_multidisc: false,
                disc_number: None,
                total_discs: None,
                confidence: 0.7,
                source: ClassificationSource::Fallback,
            },
        )],
        Vec::new(),
    );
    let id = plan.games[0].id.clone();

    let mut plan = plan;
    retarget_game_title(&mut plan, &id, "Correct Title: Special Edition");
    let p = plan.games[0].discs[0].target_chd_path.to_string_lossy().to_string();
    assert!(p.contains("Correct Title Special Edition"), "illegal chars sanitized + renamed: {}", p);

    retarget_game_platform(&mut plan, &id, Platform::Psx);
    let p2 = plan.games[0].discs[0].target_chd_path.to_string_lossy().to_lowercase();
    assert!(p2.contains("roms/ps/") || p2.contains("roms\\ps\\"), "platform folder applied: {}", p2);
    assert_eq!(plan.games[0].platform, Platform::Psx);
}
