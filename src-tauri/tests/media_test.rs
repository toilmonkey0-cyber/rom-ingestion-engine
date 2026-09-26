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
            "https://raw.githubusercontent.com/libretro-thumbnails/Sony_-_PlayStation/master/Named_Boxarts/Final%20Fantasy%20VII%20(USA).png",
            "https://raw.githubusercontent.com/libretro-thumbnails/Sony_-_PlayStation/master/Named_Boxarts/Final%20Fantasy%20VII.png",
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
            "https://raw.githubusercontent.com/libretro-thumbnails/Sony_-_PlayStation/master/Named_Boxarts/Castlevania_%20Symphony%20of%20the%20Night%20(USA).png",
            "https://raw.githubusercontent.com/libretro-thumbnails/Sony_-_PlayStation/master/Named_Boxarts/Castlevania_%20Symphony%20of%20the%20Night.png",
            "https://raw.githubusercontent.com/libretro-thumbnails/Sony_-_PlayStation/master/Named_Boxarts/Castlevania%20(USA).png",
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
            "https://raw.githubusercontent.com/libretro-thumbnails/Sega_-_Dreamcast/master/Named_Snaps/Sonic%20Adventure%20-%20Limited%20Edition%20(USA).png",
            "https://raw.githubusercontent.com/libretro-thumbnails/Sega_-_Dreamcast/master/Named_Snaps/Sonic%20Adventure%20-%20Limited%20Edition.png",
            "https://raw.githubusercontent.com/libretro-thumbnails/Sega_-_Dreamcast/master/Named_Snaps/Sonic%20Adventure%20(USA).png",
            "https://raw.githubusercontent.com/libretro-thumbnails/Sega_-_Dreamcast/master/Named_Snaps/Sonic%20Adventure.png",
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
            "https://raw.githubusercontent.com/libretro-thumbnails/Sony_-_PlayStation/master/Named_Boxarts/Metal%20Gear%20Solid%20(USA).png",
            "https://raw.githubusercontent.com/libretro-thumbnails/Sony_-_PlayStation/master/Named_Boxarts/Metal%20Gear%20Solid.png",
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
            binary_tracks: Vec::new(),
            chdman_command: String::new(),
        }],
        target_m3u_path: Some(PathBuf::from("out/roms/psx/Final Fantasy VII (USA).m3u")),
        confidence: 0.99,
        source: ClassificationSource::RedumpCache,
        enabled: true,
        needs_review: false,
        status_note: None,
        role: String::new(),
        artwork_url: Some("https://raw.githubusercontent.com/libretro-thumbnails/Sony_-_PlayStation/master/Named_Boxarts/Final%20Fantasy%20VII%20(USA).png".to_string()),
        target_media_paths: vec![PathBuf::from("out/roms/psx/media/covers/Final Fantasy VII (USA).png")],
    };

    assert_eq!(game.artwork_url.as_deref(), Some("https://raw.githubusercontent.com/libretro-thumbnails/Sony_-_PlayStation/master/Named_Boxarts/Final%20Fantasy%20VII%20(USA).png"));
    assert_eq!(game.target_media_paths.len(), 1);
}

#[tokio::test]
async fn test_resolve_artwork_candidate_success_and_fallback() {
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

            if req.starts_with("HEAD /candidate1") {
                let resp = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                let _ = socket.write_all(resp.as_bytes()).await;
            } else if req.starts_with("HEAD /candidate2") {
                let resp = "HTTP/1.1 200 OK\r\nContent-Length: 1024\r\nContent-Type: image/png\r\nConnection: close\r\n\r\n";
                let _ = socket.write_all(resp.as_bytes()).await;
            } else if req.starts_with("HEAD /candidate3") {
                let resp = "HTTP/1.1 200 OK\r\nContent-Length: 2048\r\nContent-Type: image/png\r\nConnection: close\r\n\r\n";
                let _ = socket.write_all(resp.as_bytes()).await;
            } else {
                let resp = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                let _ = socket.write_all(resp.as_bytes()).await;
            }
            let _ = socket.flush().await;
        }
    });

    let client = reqwest::Client::new();
    let c1 = format!("http://127.0.0.1:{}/candidate1", port);
    let c2 = format!("http://127.0.0.1:{}/candidate2", port);
    let c3 = format!("http://127.0.0.1:{}/candidate3", port);

    // Candidates in order: c1 (404), c2 (200), c3 (200) -> should resolve to c2
    let resolved = resolve_artwork_url_from_candidates(&client, &[c1.clone(), c2.clone(), c3.clone()]).await;
    assert_eq!(resolved, Some(c2));

    // Candidate c1 only (404) -> should return None
    let resolved_none = resolve_artwork_url_from_candidates(&client, &[c1]).await;
    assert_eq!(resolved_none, None);

    // Empty candidates list -> should return None
    let empty: Vec<String> = Vec::new();
    assert_eq!(resolve_artwork_url_from_candidates(&client, &empty).await, None);

    server.abort();
}

#[tokio::test]
async fn test_download_media_file_atomic_and_error_cleanup() {
    use tempfile::tempdir;
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

            if req.starts_with("GET /success.png") {
                let body = [0x89, b'P', b'N', b'G', 1, 2, 3, 4];
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: image/png\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = socket.write_all(resp.as_bytes()).await;
                let _ = socket.write_all(&body).await;
            } else if req.starts_with("GET /error.png") {
                let resp = "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                let _ = socket.write_all(resp.as_bytes()).await;
            } else if req.starts_with("GET /broken.png") {
                // Send partial body and close connection abruptly
                let resp = "HTTP/1.1 200 OK\r\nContent-Length: 100\r\nContent-Type: image/png\r\nConnection: close\r\n\r\n";
                let _ = socket.write_all(resp.as_bytes()).await;
                let _ = socket.write_all(&[1, 2, 3, 4, 5]).await;
                let _ = socket.flush().await;
                drop(socket);
                continue;
            }
            let _ = socket.flush().await;
        }
    });

    let client = reqwest::Client::new();
    let dir = tempdir().unwrap();

    // 1. Successful atomic download
    let success_url = format!("http://127.0.0.1:{}/success.png", port);
    let success_path = dir.path().join("subfolder").join("cover.png");
    let res = download_media_file(&client, &success_url, &success_path).await;
    assert!(res.is_ok(), "Download failed: {:?}", res);
    assert!(success_path.exists());
    let part_path = PathBuf::from(format!("{}.part", success_path.to_string_lossy()));
    assert!(!part_path.exists(), ".part file should not remain after successful download");
    let downloaded_bytes = std::fs::read(&success_path).unwrap();
    assert_eq!(downloaded_bytes, vec![0x89, b'P', b'N', b'G', 1, 2, 3, 4]);

    // 2. HTTP 500 error cleanup
    let error_url = format!("http://127.0.0.1:{}/error.png", port);
    let error_path = dir.path().join("failed.png");
    let res = download_media_file(&client, &error_url, &error_path).await;
    assert!(res.is_err());
    assert!(!error_path.exists(), "Target file should not exist on error");
    let error_part = PathBuf::from(format!("{}.part", error_path.to_string_lossy()));
    assert!(!error_part.exists(), ".part file should be cleaned up on error");

    // 3. Broken mid-stream transfer cleanup
    let broken_url = format!("http://127.0.0.1:{}/broken.png", port);
    let broken_path = dir.path().join("broken.png");
    let res = download_media_file(&client, &broken_url, &broken_path).await;
    assert!(res.is_err());
    assert!(!broken_path.exists(), "Target file should not exist on stream error");
    let broken_part = PathBuf::from(format!("{}.part", broken_path.to_string_lossy()));
    assert!(!broken_part.exists(), ".part file should be deleted on stream failure");

    server.abort();
}

#[test]
fn test_screenshot_and_title_candidates_are_not_boxart_urls() {
    let boxart = generate_candidate_urls(
        Platform::Psx,
        "Final Fantasy VII",
        "USA",
        MediaType::BoxArt,
    );
    let snaps = generate_candidate_urls(
        Platform::Psx,
        "Final Fantasy VII",
        "USA",
        MediaType::Screenshots,
    );
    let titles = generate_candidate_urls(
        Platform::Psx,
        "Final Fantasy VII",
        "USA",
        MediaType::TitleScreens,
    );
    assert!(boxart[0].contains("/Named_Boxarts/"));
    assert!(snaps[0].contains("/Named_Snaps/"));
    assert!(titles[0].contains("/Named_Titles/"));
    assert_ne!(boxart[0], snaps[0]);
    assert_ne!(boxart[0], titles[0]);
    assert!(boxart[0].contains("Final%20Fantasy%20VII%20(USA)"));

    let shot = PathBuf::from("E:/games/roms/psx/media/screenshots/Final Fantasy VII (USA).png");
    let title = PathBuf::from("E:/games/roms/psx/media/titlescreens/Final Fantasy VII (USA).png");
    let cover = PathBuf::from("E:/games/roms/psx/media/covers/Final Fantasy VII (USA).png");
    assert_eq!(media_type_for_path(&shot), MediaType::Screenshots);
    assert_eq!(media_type_for_path(&title), MediaType::TitleScreens);
    assert_eq!(media_type_for_path(&cover), MediaType::BoxArt);
    assert!(generate_candidate_urls(
        Platform::Psx,
        "Final Fantasy VII",
        "USA",
        media_type_for_path(&shot),
    )[0]
        .contains("/Named_Snaps/"));
    assert!(generate_candidate_urls(
        Platform::Psx,
        "Final Fantasy VII",
        "USA",
        media_type_for_path(&title),
    )[0]
        .contains("/Named_Titles/"));
}

#[tokio::test]
async fn test_non_image_http_200_is_not_saved_as_art() {
    use tempfile::tempdir;
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
            if req.starts_with("GET /note.txt") {
                let body = b"this is text, not an image";
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = socket.write_all(resp.as_bytes()).await;
                let _ = socket.write_all(body).await;
            }
            let _ = socket.flush().await;
        }
    });

    let client = reqwest::Client::new();
    let dir = tempdir().unwrap();
    let dest = dir.path().join("Named_Snaps").join("Game.png");
    let result = download_media_file(
        &client,
        &format!("http://127.0.0.1:{port}/note.txt"),
        &dest,
    )
    .await;
    assert!(result.is_err(), "text body must not be accepted as art");
    assert!(!dest.exists(), "non-image response was saved as art");
    let part = PathBuf::from(format!("{}.part", dest.to_string_lossy()));
    assert!(!part.exists());
    server.abort();
}

#[test]
fn test_resolve_media_paths_for_game_options() {
    let out = PathBuf::from("E:/games");

    // All enabled
    let opts_all = MediaOptions {
        download_boxart: true,
        download_screenshots: true,
        download_titles: true,
    };
    let paths = resolve_media_paths_for_game(
        &out,
        FrontendPreset::EsDe,
        Platform::Psx,
        "Final Fantasy VII",
        "USA",
        &opts_all,
    );
    assert_eq!(paths.len(), 3);
    assert_eq!(paths[0], PathBuf::from("E:/games/roms/psx/media/covers/Final Fantasy VII (USA).png"));
    assert_eq!(paths[1], PathBuf::from("E:/games/roms/psx/media/screenshots/Final Fantasy VII (USA).png"));
    assert_eq!(paths[2], PathBuf::from("E:/games/roms/psx/media/titlescreens/Final Fantasy VII (USA).png"));

    // Boxart only (default)
    let opts_boxart = MediaOptions::default();
    let paths_boxart = resolve_media_paths_for_game(
        &out,
        FrontendPreset::AnbernicStock,
        Platform::Saturn,
        "Panzer Dragoon (USA)",
        "USA",
        &opts_boxart,
    );
    assert_eq!(paths_boxart.len(), 1);
    assert_eq!(paths_boxart[0], PathBuf::from("E:/games/ROMS/SATURN/Imgs/Panzer Dragoon (USA).png"));

    // None enabled
    let opts_none = MediaOptions {
        download_boxart: false,
        download_screenshots: false,
        download_titles: false,
    };
    let paths_none = resolve_media_paths_for_game(
        &out,
        FrontendPreset::Batocera,
        Platform::Dreamcast,
        "Shenmue",
        "USA",
        &opts_none,
    );
    assert!(paths_none.is_empty());
}

