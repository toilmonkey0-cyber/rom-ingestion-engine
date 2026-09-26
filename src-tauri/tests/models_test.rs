use std::path::PathBuf;
use rom_ingest_core::models::*;

#[test]
fn test_models_json_roundtrip() {
    let disc = PlannedDisc {
        disc_number: 1,
        source_descriptor: PathBuf::from("C:/Roms/FF7_Disc1.cue"),
        target_chd_path: PathBuf::from("C:/Output/psx/.discs/Final Fantasy VII (USA) (Disc 1).chd"),
        relative_m3u_entry: None,
        status: TaskStatus::Pending,
            binary_tracks: Vec::new(),
            chdman_command: String::new(),
    };

    let game = PlannedGame {
        id: "game-1".to_string(),
        canonical_title: "Final Fantasy VII".to_string(),
        platform: Platform::Psx,
        region: "USA".to_string(),
        is_multidisc: true,
        discs: vec![disc],
        target_m3u_path: Some(PathBuf::from("C:/Output/psx/Final Fantasy VII (USA).m3u")),
        confidence: 0.95,
        source: ClassificationSource::JevAI,
        enabled: true,
        needs_review: false,
        status_note: None,
        role: String::new(),
        artwork_url: Some("https://raw.githubusercontent.com/libretro-thumbnails/Sony_-_PlayStation/master/Named_Boxarts/Final%20Fantasy%20VII%20(USA).png".to_string()),
        target_media_paths: vec![PathBuf::from("C:/Output/ROMS/PS/Imgs/Final Fantasy VII (USA).png")],
    };

    let plan = IngestionPlan {
        input_dir: PathBuf::from("C:/Roms"),
        output_dir: PathBuf::from("C:/Output"),
        preset: FrontendPreset::AnbernicStock,
        skipped_sources: Vec::new(),
        games: vec![game],
        total_source_bytes: 700_000_000,
        estimated_output_bytes: 450_000_000,
    };

    let serialized = serde_json::to_string(&plan).expect("Failed to serialize");
    let deserialized: IngestionPlan = serde_json::from_str(&serialized).expect("Failed to deserialize");
    assert_eq!(deserialized.games[0].canonical_title, "Final Fantasy VII");
    assert_eq!(deserialized.preset, FrontendPreset::AnbernicStock);
}

#[test]
fn test_platform_variants_serialization() {
    let platforms = vec![
        (Platform::Psx, "\"psx\""),
        (Platform::Saturn, "\"saturn\""),
        (Platform::Dreamcast, "\"dreamcast\""),
        (Platform::SegaCd, "\"segacd\""),
        (Platform::PceCd, "\"pcecd\""),
        (Platform::Unknown, "\"unknown\""),
    ];

    for (variant, expected_json) in platforms {
        let serialized = serde_json::to_string(&variant).expect("serialize platform");
        assert_eq!(serialized, expected_json);
        let deserialized: Platform = serde_json::from_str(&serialized).expect("deserialize platform");
        assert_eq!(deserialized, variant);
    }
}

#[test]
fn test_frontend_preset_variants_serialization() {
    let presets = vec![
        (FrontendPreset::EsDe, "\"esde\""),
        (FrontendPreset::OnionOs, "\"onionos\""),
        (FrontendPreset::AnbernicStock, "\"anbernicstock\""),
        (FrontendPreset::Batocera, "\"batocera\""),
        (FrontendPreset::Custom, "\"custom\""),
    ];

    for (variant, expected_json) in presets {
        let serialized = serde_json::to_string(&variant).expect("serialize preset");
        assert_eq!(serialized, expected_json);
        let deserialized: FrontendPreset = serde_json::from_str(&serialized).expect("deserialize preset");
        assert_eq!(deserialized, variant);
    }
}

#[test]
fn test_classification_source_and_task_status_serialization() {
    let sources = vec![
        (ClassificationSource::RedumpCache, "\"redumpcache\""),
        (ClassificationSource::JevAI, "\"jevai\""),
        (ClassificationSource::Fallback, "\"fallback\""),
    ];

    for (source, expected_json) in sources {
        let serialized = serde_json::to_string(&source).expect("serialize source");
        assert_eq!(serialized, expected_json);
        let deserialized: ClassificationSource = serde_json::from_str(&serialized).expect("deserialize source");
        assert_eq!(deserialized, source);
    }

    let statuses = vec![
        (TaskStatus::Pending, "\"pending\""),
        (TaskStatus::Compressing, "\"compressing\""),
        (TaskStatus::Verified, "\"verified\""),
        (TaskStatus::Failed, "\"failed\""),
        (TaskStatus::Skipped, "\"skipped\""),
    ];

    for (status, expected_json) in statuses {
        let serialized = serde_json::to_string(&status).expect("serialize status");
        assert_eq!(serialized, expected_json);
        let deserialized: TaskStatus = serde_json::from_str(&serialized).expect("deserialize status");
        assert_eq!(deserialized, status);
    }
}

#[test]
fn test_disc_fingerprint_and_game_classification_roundtrip() {
    let fingerprint = DiscFingerprint {
        primary_file: PathBuf::from("/games/psx/game.cue"),
        binary_tracks: vec![PathBuf::from("/games/psx/game.bin")],
        detected_platform: Platform::Psx,
        calculated_sha1: Some("da39a3ee5e6b4b0d3255bfef95601890afd80709".to_string()),
        total_bytes: 650_000_000,
        scan_error: None,
    };

    let serialized_fp = serde_json::to_string(&fingerprint).expect("serialize fingerprint");
    let deserialized_fp: DiscFingerprint = serde_json::from_str(&serialized_fp).expect("deserialize fingerprint");
    assert_eq!(deserialized_fp, fingerprint);

    let classification = GameClassification {
        canonical_title: "Castlevania: Symphony of the Night".to_string(),
        platform: Platform::Psx,
        region: "USA".to_string(),
        is_multidisc: false,
        disc_number: Some(1),
        total_discs: Some(1),
        confidence: 0.99,
        source: ClassificationSource::RedumpCache,
    };

    let serialized_gc = serde_json::to_string(&classification).expect("serialize classification");
    let deserialized_gc: GameClassification = serde_json::from_str(&serialized_gc).expect("deserialize classification");
    assert_eq!(deserialized_gc.canonical_title, classification.canonical_title);
    assert_eq!(deserialized_gc.confidence, classification.confidence);
}

#[test]
fn test_media_type_and_options_serialization() {
    let types = vec![
        (MediaType::BoxArt, "\"boxart\""),
        (MediaType::Screenshots, "\"screenshots\""),
        (MediaType::TitleScreens, "\"titlescreens\""),
    ];

    for (variant, expected_json) in types {
        let serialized = serde_json::to_string(&variant).expect("serialize media type");
        assert_eq!(serialized, expected_json);
        let deserialized: MediaType = serde_json::from_str(&serialized).expect("deserialize media type");
        assert_eq!(deserialized, variant);
    }

    let default_options = MediaOptions::default();
    assert!(default_options.download_boxart);
    assert!(!default_options.download_screenshots);
    assert!(!default_options.download_titles);

    let serialized_opt = serde_json::to_string(&default_options).expect("serialize media options");
    let deserialized_opt: MediaOptions = serde_json::from_str(&serialized_opt).expect("deserialize media options");
    assert_eq!(deserialized_opt, default_options);
}
