use std::path::PathBuf;
use rom_ingest_core::models::{ClassificationSource, FrontendPreset, GameClassification, Platform};
use rom_ingest_core::organizer::m3u::*;
use rom_ingest_core::organizer::presets::*;

#[test]
fn test_m3u_formatting() {
    let paths = vec![
        ".discs/Final Fantasy VII (USA) (Disc 1).chd",
        ".discs/Final Fantasy VII (USA) (Disc 2).chd",
        ".discs/Final Fantasy VII (USA) (Disc 3).chd",
    ];
    let content = generate_m3u_content(&paths);
    assert_eq!(
        content,
        ".discs/Final Fantasy VII (USA) (Disc 1).chd\n.discs/Final Fantasy VII (USA) (Disc 2).chd\n.discs/Final Fantasy VII (USA) (Disc 3).chd\n"
    );
}

#[test]
fn test_preset_platform_folders() {
    assert_eq!(get_platform_folder(FrontendPreset::AnbernicStock, Platform::Psx), "ROMS/PS");
    assert_eq!(get_platform_folder(FrontendPreset::AnbernicStock, Platform::Saturn), "ROMS/SATURN");
    assert_eq!(get_platform_folder(FrontendPreset::AnbernicStock, Platform::Dreamcast), "ROMS/DC");
    assert_eq!(get_platform_folder(FrontendPreset::AnbernicStock, Platform::SegaCd), "ROMS/MDCD");
    assert_eq!(get_platform_folder(FrontendPreset::AnbernicStock, Platform::PceCd), "ROMS/PCE");

    assert_eq!(get_platform_folder(FrontendPreset::EsDe, Platform::Psx), "ROMs/psx");
    assert_eq!(get_platform_folder(FrontendPreset::OnionOs, Platform::Psx), "Roms/PS");
    assert_eq!(get_platform_folder(FrontendPreset::Batocera, Platform::Psx), "roms/psx");
}

#[test]
fn test_target_paths_resolution() {
    let out = PathBuf::from("E:/");
    let paths = resolve_target_paths(
        &out,
        FrontendPreset::AnbernicStock,
        Platform::Psx,
        "Final Fantasy VII",
        "USA",
        true,
        Some(1),
        Some(3),
    );

    assert_eq!(
        paths.chd_path,
        PathBuf::from("E:/ROMS/PS/.discs/Final Fantasy VII (USA) (Disc 1).chd")
    );
    assert_eq!(
        paths.m3u_path,
        Some(PathBuf::from("E:/ROMS/PS/Final Fantasy VII (USA).m3u"))
    );
    assert_eq!(
        paths.relative_m3u_entry,
        Some(".discs/Final Fantasy VII (USA) (Disc 1).chd".to_string())
    );
}

#[test]
fn test_all_frontend_preset_platform_mappings() {
    // ES-DE
    assert_eq!(get_platform_folder(FrontendPreset::EsDe, Platform::Psx), "ROMs/psx");
    assert_eq!(get_platform_folder(FrontendPreset::EsDe, Platform::Saturn), "ROMs/saturn");
    assert_eq!(get_platform_folder(FrontendPreset::EsDe, Platform::Dreamcast), "ROMs/dreamcast");
    assert_eq!(get_platform_folder(FrontendPreset::EsDe, Platform::SegaCd), "ROMs/segacd");
    assert_eq!(get_platform_folder(FrontendPreset::EsDe, Platform::PceCd), "ROMs/pcenginecd");
    assert_eq!(get_platform_folder(FrontendPreset::EsDe, Platform::Unknown), "ROMs/unknown");

    // OnionOS
    assert_eq!(get_platform_folder(FrontendPreset::OnionOs, Platform::Psx), "Roms/PS");
    assert_eq!(get_platform_folder(FrontendPreset::OnionOs, Platform::Saturn), "Roms/SEGASATURN");
    assert_eq!(get_platform_folder(FrontendPreset::OnionOs, Platform::Dreamcast), "Roms/DREAMCAST");
    assert_eq!(get_platform_folder(FrontendPreset::OnionOs, Platform::SegaCd), "Roms/SEGACD");
    assert_eq!(get_platform_folder(FrontendPreset::OnionOs, Platform::PceCd), "Roms/PCECD");
    assert_eq!(get_platform_folder(FrontendPreset::OnionOs, Platform::Unknown), "Roms/UNKNOWN");

    // Anbernic Stock
    assert_eq!(get_platform_folder(FrontendPreset::AnbernicStock, Platform::Psx), "ROMS/PS");
    assert_eq!(get_platform_folder(FrontendPreset::AnbernicStock, Platform::Saturn), "ROMS/SATURN");
    assert_eq!(get_platform_folder(FrontendPreset::AnbernicStock, Platform::Dreamcast), "ROMS/DC");
    assert_eq!(get_platform_folder(FrontendPreset::AnbernicStock, Platform::SegaCd), "ROMS/MDCD");
    assert_eq!(get_platform_folder(FrontendPreset::AnbernicStock, Platform::PceCd), "ROMS/PCE");
    assert_eq!(get_platform_folder(FrontendPreset::AnbernicStock, Platform::Unknown), "ROMS/UNKNOWN");

    // Batocera
    assert_eq!(get_platform_folder(FrontendPreset::Batocera, Platform::Psx), "roms/psx");
    assert_eq!(get_platform_folder(FrontendPreset::Batocera, Platform::Saturn), "roms/saturn");
    assert_eq!(get_platform_folder(FrontendPreset::Batocera, Platform::Dreamcast), "roms/dreamcast");
    assert_eq!(get_platform_folder(FrontendPreset::Batocera, Platform::SegaCd), "roms/segacd");
    assert_eq!(get_platform_folder(FrontendPreset::Batocera, Platform::PceCd), "roms/pcenginecd");
    assert_eq!(get_platform_folder(FrontendPreset::Batocera, Platform::Unknown), "roms/unknown");

    // Custom
    assert_eq!(get_platform_folder(FrontendPreset::Custom, Platform::Psx), "roms/psx");
    assert_eq!(get_platform_folder(FrontendPreset::Custom, Platform::Saturn), "roms/saturn");
    assert_eq!(get_platform_folder(FrontendPreset::Custom, Platform::Dreamcast), "roms/dreamcast");
    assert_eq!(get_platform_folder(FrontendPreset::Custom, Platform::SegaCd), "roms/segacd");
    assert_eq!(get_platform_folder(FrontendPreset::Custom, Platform::PceCd), "roms/pcenginecd");
    assert_eq!(get_platform_folder(FrontendPreset::Custom, Platform::Unknown), "roms/unknown");

    // Multidisc subfolder check for all
    assert_eq!(get_multidisc_subfolder(FrontendPreset::EsDe), ".discs");
    assert_eq!(get_multidisc_subfolder(FrontendPreset::OnionOs), ".discs");
    assert_eq!(get_multidisc_subfolder(FrontendPreset::AnbernicStock), ".discs");
    assert_eq!(get_multidisc_subfolder(FrontendPreset::Batocera), ".discs");
    assert_eq!(get_multidisc_subfolder(FrontendPreset::Custom), ".discs");
}

#[test]
fn test_single_disc_resolution() {
    let out = PathBuf::from("E:/");
    let paths = resolve_target_paths(
        &out,
        FrontendPreset::OnionOs,
        Platform::Saturn,
        "Nights into Dreams",
        "USA",
        false,
        None,
        None,
    );

    assert_eq!(
        paths.chd_path,
        PathBuf::from("E:/Roms/SEGASATURN/Nights into Dreams (USA).chd")
    );
    assert_eq!(paths.m3u_path, None);
    assert_eq!(paths.relative_m3u_entry, None);
}

#[test]
fn test_target_paths_resolution_for_classification() {
    let out = PathBuf::from("D:/RomsCollection");
    let classification = GameClassification {
        canonical_title: "Shenmue".to_string(),
        platform: Platform::Dreamcast,
        region: "USA".to_string(),
        is_multidisc: true,
        disc_number: Some(2),
        total_discs: Some(4),
        confidence: 0.99,
        source: ClassificationSource::RedumpCache,
    };

    let paths = resolve_target_paths_for_classification(
        &out,
        FrontendPreset::EsDe,
        &classification,
    );

    assert_eq!(
        paths.chd_path,
        PathBuf::from("D:/RomsCollection/ROMs/dreamcast/.discs/Shenmue (USA) (Disc 2).chd")
    );
    assert_eq!(
        paths.m3u_path,
        Some(PathBuf::from("D:/RomsCollection/ROMs/dreamcast/Shenmue (USA).m3u"))
    );
    assert_eq!(
        paths.relative_m3u_entry,
        Some(".discs/Shenmue (USA) (Disc 2).chd".to_string())
    );
}

#[test]
fn test_title_and_region_formatting_edge_cases() {
    let out = PathBuf::from("E:/");

    // Empty region
    let paths_no_region = resolve_target_paths(
        &out,
        FrontendPreset::Batocera,
        Platform::Psx,
        "Spyro the Dragon",
        "",
        false,
        None,
        None,
    );
    assert_eq!(
        paths_no_region.chd_path,
        PathBuf::from("E:/roms/psx/Spyro the Dragon.chd")
    );

    // Title already has region
    let paths_already_has_region = resolve_target_paths(
        &out,
        FrontendPreset::Batocera,
        Platform::Psx,
        "Spyro the Dragon (USA)",
        "USA",
        false,
        None,
        None,
    );
    assert_eq!(
        paths_already_has_region.chd_path,
        PathBuf::from("E:/roms/psx/Spyro the Dragon (USA).chd")
    );

    // Multi-disc without disc_number
    let paths_multi_no_disc_num = resolve_target_paths(
        &out,
        FrontendPreset::Batocera,
        Platform::Psx,
        "Grand Theft Auto 2",
        "USA",
        true,
        None,
        None,
    );
    assert_eq!(
        paths_multi_no_disc_num.chd_path,
        PathBuf::from("E:/roms/psx/.discs/Grand Theft Auto 2 (USA).chd")
    );
    assert_eq!(
        paths_multi_no_disc_num.m3u_path,
        Some(PathBuf::from("E:/roms/psx/Grand Theft Auto 2 (USA).m3u"))
    );
}

#[test]
fn test_m3u_file_write_and_parse_roundtrip() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let m3u_file_path = tmp.path().join("subfolder").join("Metal Gear Solid (USA).m3u");

    let entries = vec![
        r".discs\Metal Gear Solid (USA) (Disc 1).chd",
        r".discs\Metal Gear Solid (USA) (Disc 2).chd",
    ];

    let write_res = write_m3u_file(&m3u_file_path, &entries);
    assert!(write_res.is_ok(), "write_m3u_file failed: {:?}", write_res);

    assert!(m3u_file_path.exists());
    let written = std::fs::read_to_string(&m3u_file_path).expect("read written file");

    let expected = ".discs/Metal Gear Solid (USA) (Disc 1).chd\n.discs/Metal Gear Solid (USA) (Disc 2).chd\n";
    assert_eq!(written, expected);

    let parsed = parse_m3u_content(&written);
    assert_eq!(
        parsed,
        vec![
            ".discs/Metal Gear Solid (USA) (Disc 1).chd",
            ".discs/Metal Gear Solid (USA) (Disc 2).chd",
        ]
    );
}

#[test]
fn test_custom_preset_configuration() {
    let mut config = CustomPresetConfig::default();
    config.psx = "custom_psx".to_string();
    config.multidisc_subfolder = "multi_discs".to_string();

    assert_eq!(config.get_platform_folder(Platform::Psx), "custom_psx");
    assert_eq!(config.get_platform_folder(Platform::Saturn), "roms/saturn");
    assert_eq!(config.get_multidisc_subfolder(), "multi_discs");
}
