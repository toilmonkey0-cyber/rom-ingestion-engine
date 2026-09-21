use std::path::PathBuf;
use rom_ingest_core::models::{FrontendPreset, PlannedDisc, PlannedGame, Platform, TaskStatus, ClassificationSource};
use rom_ingest_core::organizer::media::*;

#[test]
fn test_libretro_system_mapping() {
    assert_eq!(platform_to_libretro_system(Platform::Psx), Some("Sony - PlayStation"));
    assert_eq!(platform_to_libretro_system(Platform::Saturn), Some("Sega - Saturn"));
    assert_eq!(platform_to_libretro_system(Platform::Dreamcast), Some("Sega - Dreamcast"));
    assert_eq!(platform_to_libretro_system(Platform::SegaCd), Some("Sega - Mega-CD - Sega CD"));
    assert_eq!(platform_to_libretro_system(Platform::PceCd), Some("NEC - PC Engine CD - TurboGrafx-CD"));
    assert_eq!(platform_to_libretro_system(Platform::Unknown), None);
}

#[test]
fn test_libretro_title_sanitization() {
    assert_eq!(sanitize_libretro_title("Castlevania: Symphony of the Night"), "Castlevania_ Symphony of the Night");
    assert_eq!(sanitize_libretro_title("Sonic CD & Knuckles?"), "Sonic CD _ Knuckles_");
    assert_eq!(sanitize_libretro_title("Game <1> *2* /3/ \\4\\ |5|"), "Game _1_ _2_ _3_ _4_ _5_");
}

#[test]
fn test_preset_media_path_resolution() {
    let out = PathBuf::from("E:/");
    let anbernic_path = resolve_preset_media_path(
        &out,
        FrontendPreset::AnbernicStock,
        Platform::Psx,
        "Metal Gear Solid (USA)",
        MediaType::BoxArt,
    );
    assert_eq!(anbernic_path, PathBuf::from("E:/ROMS/PS/Imgs/Metal Gear Solid (USA).png"));

    let esde_path = resolve_preset_media_path(
        &out,
        FrontendPreset::EsDe,
        Platform::Psx,
        "Metal Gear Solid (USA)",
        MediaType::BoxArt,
    );
    assert_eq!(esde_path, PathBuf::from("E:/roms/psx/media/covers/Metal Gear Solid (USA).png"));
}

#[test]
fn test_preset_media_path_resolution_all_presets_and_types() {
    let out = PathBuf::from("E:/games");

    // OnionOS
    let onion_boxart = resolve_preset_media_path(
        &out,
        FrontendPreset::OnionOs,
        Platform::Psx,
        "Final Fantasy VII (USA)",
        MediaType::BoxArt,
    );
    assert_eq!(onion_boxart, PathBuf::from("E:/games/Roms/PS/Imgs/Final Fantasy VII (USA).png"));

    // Batocera BoxArt
    let batocera_boxart = resolve_preset_media_path(
        &out,
        FrontendPreset::Batocera,
        Platform::Saturn,
        "Panzer Dragoon (USA)",
        MediaType::BoxArt,
    );
    assert_eq!(batocera_boxart, PathBuf::from("E:/games/roms/saturn/images/Panzer Dragoon (USA)-thumb.png"));

    // Batocera Screenshots & TitleScreens
    let batocera_snap = resolve_preset_media_path(
        &out,
        FrontendPreset::Batocera,
        Platform::Saturn,
        "Panzer Dragoon (USA)",
        MediaType::Screenshots,
    );
    assert_eq!(batocera_snap, PathBuf::from("E:/games/roms/saturn/images/Panzer Dragoon (USA)-screenshot.png"));

    let batocera_title = resolve_preset_media_path(
        &out,
        FrontendPreset::Batocera,
        Platform::Saturn,
        "Panzer Dragoon (USA)",
        MediaType::TitleScreens,
    );
    assert_eq!(batocera_title, PathBuf::from("E:/games/roms/saturn/images/Panzer Dragoon (USA)-titlescreen.png"));

    // ES-DE Screenshots & TitleScreens
    let esde_snap = resolve_preset_media_path(
        &out,
        FrontendPreset::EsDe,
        Platform::Dreamcast,
        "Shenmue (USA)",
        MediaType::Screenshots,
    );
    assert_eq!(esde_snap, PathBuf::from("E:/games/roms/dreamcast/media/screenshots/Shenmue (USA).png"));

    let esde_title = resolve_preset_media_path(
        &out,
        FrontendPreset::EsDe,
        Platform::Dreamcast,
        "Shenmue (USA)",
        MediaType::TitleScreens,
    );
    assert_eq!(esde_title, PathBuf::from("E:/games/roms/dreamcast/media/titlescreens/Shenmue (USA).png"));

    // Custom
    let custom_path = resolve_preset_media_path(
        &out,
        FrontendPreset::Custom,
        Platform::SegaCd,
        "Sonic CD (USA)",
        MediaType::BoxArt,
    );
    assert_eq!(custom_path, PathBuf::from("E:/games/roms/segacd/media/covers/Sonic CD (USA).png"));

    // PCE-CD on Anbernic
    let pce_path = resolve_preset_media_path(
        &out,
        FrontendPreset::AnbernicStock,
        Platform::PceCd,
        "Castlevania - Rondo of Blood (Japan).png", // stem with .png extension stripped cleanly
        MediaType::BoxArt,
    );
    assert_eq!(pce_path, PathBuf::from("E:/games/ROMS/PCE/Imgs/Castlevania - Rondo of Blood (Japan).png"));
}

#[test]
fn test_generate_candidate_urls_standard() {
    let urls = generate_candidate_urls(
        Platform::Psx,
        "Final Fantasy VII",
        "USA",
        MediaType::BoxArt,
    );
    assert_eq!(
        urls,
        vec![
            "https://raw.githubusercontent.com/libretro-thumbnails/Sony_-_PlayStation/master/Named_Boxarts/Final Fantasy VII (USA).png",
            "https://raw.githubusercontent.com/libretro-thumbnails/Sony_-_PlayStation/master/Named_Boxarts/Final Fantasy VII.png",
        ]
    );
}

#[test]
fn test_generate_candidate_urls_with_subtitle_and_sanitization() {
    let urls = generate_candidate_urls(
        Platform::Psx,
        "Castlevania: Symphony of the Night",
        "USA",
        MediaType::BoxArt,
    );
    assert_eq!(
        urls,
        vec![
            "https://raw.githubusercontent.com/libretro-thumbnails/Sony_-_PlayStation/master/Named_Boxarts/Castlevania_ Symphony of the Night (USA).png",
            "https://raw.githubusercontent.com/libretro-thumbnails/Sony_-_PlayStation/master/Named_Boxarts/Castlevania_ Symphony of the Night.png",
            "https://raw.githubusercontent.com/libretro-thumbnails/Sony_-_PlayStation/master/Named_Boxarts/Castlevania (USA).png",
            "https://raw.githubusercontent.com/libretro-thumbnails/Sony_-_PlayStation/master/Named_Boxarts/Castlevania.png",
        ]
    );
}

#[test]
fn test_generate_candidate_urls_with_dash_subtitle() {
    let urls = generate_candidate_urls(
        Platform::Dreamcast,
        "Sonic Adventure - Limited Edition",
        "USA",
        MediaType::Screenshots,
    );
    assert_eq!(
        urls,
        vec![
            "https://raw.githubusercontent.com/libretro-thumbnails/Sega_-_Dreamcast/master/Named_Snaps/Sonic Adventure - Limited Edition (USA).png",
            "https://raw.githubusercontent.com/libretro-thumbnails/Sega_-_Dreamcast/master/Named_Snaps/Sonic Adventure - Limited Edition.png",
            "https://raw.githubusercontent.com/libretro-thumbnails/Sega_-_Dreamcast/master/Named_Snaps/Sonic Adventure (USA).png",
            "https://raw.githubusercontent.com/libretro-thumbnails/Sega_-_Dreamcast/master/Named_Snaps/Sonic Adventure.png",
        ]
    );
}

#[test]
fn test_generate_candidate_urls_title_already_has_region() {
    let urls = generate_candidate_urls(
        Platform::Psx,
        "Metal Gear Solid (USA)",
        "USA",
        MediaType::BoxArt,
    );
    assert_eq!(
        urls,
        vec![
            "https://raw.githubusercontent.com/libretro-thumbnails/Sony_-_PlayStation/master/Named_Boxarts/Metal Gear Solid (USA).png",
            "https://raw.githubusercontent.com/libretro-thumbnails/Sony_-_PlayStation/master/Named_Boxarts/Metal Gear Solid.png",
        ]
    );
}

#[test]
fn test_generate_candidate_urls_unknown_platform() {
    let urls = generate_candidate_urls(
        Platform::Unknown,
        "Some Game",
        "USA",
        MediaType::BoxArt,
    );
    assert!(urls.is_empty());
}

#[test]
fn test_planned_game_model_with_media() {
    let game = PlannedGame {
        id: "game-1".to_string(),
        canonical_title: "Final Fantasy VII".to_string(),
        platform: Platform::Psx,
        region: "USA".to_string(),
        is_multidisc: true,
        discs: vec![PlannedDisc {
            disc_number: 1,
            source_descriptor: PathBuf::from("in/FF7_Disc1.cue"),
            target_chd_path: PathBuf::from("out/roms/psx/.discs/Final Fantasy VII (USA) (Disc 1).chd"),
            status: TaskStatus::Pending,
        }],
        target_m3u_path: Some(PathBuf::from("out/roms/psx/Final Fantasy VII (USA).m3u")),
        confidence: 0.99,
        source: ClassificationSource::RedumpCache,
        enabled: true,
        needs_review: false,
        artwork_url: Some("https://raw.githubusercontent.com/libretro-thumbnails/Sony_-_PlayStation/master/Named_Boxarts/Final Fantasy VII (USA).png".to_string()),
        target_media_paths: vec![PathBuf::from("out/roms/psx/media/covers/Final Fantasy VII (USA).png")],
    };

    assert_eq!(game.artwork_url.as_deref(), Some("https://raw.githubusercontent.com/libretro-thumbnails/Sony_-_PlayStation/master/Named_Boxarts/Final Fantasy VII (USA).png"));
    assert_eq!(game.target_media_paths.len(), 1);
}
