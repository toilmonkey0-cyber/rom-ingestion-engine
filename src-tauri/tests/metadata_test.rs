use tempfile::tempdir;

use rom_ingest_core::commands::MockEventSink;
use rom_ingest_core::metadata::finish_library_internal;
use rom_ingest_core::models::*;

fn make_plan(dir: &std::path::Path, preset: FrontendPreset) -> IngestionPlan {
    let psx = dir.join("roms").join("psx");
    std::fs::create_dir_all(&psx).unwrap();

    IngestionPlan {
        input_dir: dir.join("in"),
        output_dir: dir.to_path_buf(),
        preset,
        games: vec![
            PlannedGame {
                id: "g1".into(),
                canonical_title: "Final Fantasy VII".into(),
                platform: Platform::Psx,
                region: "USA".into(),
                is_multidisc: true,
                discs: vec![
                    PlannedDisc {
                        disc_number: 1,
                        source_descriptor: dir.join("a.cue"),
                        target_chd_path: psx.join(".discs").join("Final Fantasy VII (USA) (Disc 1).chd"),
                        relative_m3u_entry: Some(".discs/Final Fantasy VII (USA) (Disc 1).chd".into()),
                        binary_tracks: Vec::new(),
                        chdman_command: String::new(),
                        status: TaskStatus::Verified,
                    },
                    PlannedDisc {
                        disc_number: 2,
                        source_descriptor: dir.join("b.cue"),
                        target_chd_path: psx.join(".discs").join("Final Fantasy VII (USA) (Disc 2).chd"),
                        relative_m3u_entry: Some(".discs/Final Fantasy VII (USA) (Disc 2).chd".into()),
                        binary_tracks: Vec::new(),
                        chdman_command: String::new(),
                        status: TaskStatus::Verified,
                    },
                ],
                target_m3u_path: Some(psx.join("Final Fantasy VII (USA).m3u")),
                confidence: 1.0,
                source: ClassificationSource::RedumpCache,
                enabled: true,
                needs_review: false,
                status_note: None,
                role: String::new(),
                artwork_url: None,
                target_media_paths: Vec::new(),
            },
            PlannedGame {
                id: "g2".into(),
                canonical_title: "No Art Game".into(),
                platform: Platform::Psx,
                region: "USA".into(),
                is_multidisc: false,
                discs: vec![PlannedDisc {
                    disc_number: 1,
                    source_descriptor: dir.join("c.cue"),
                    target_chd_path: psx.join("No Art Game (USA).chd"),
                    relative_m3u_entry: None,
                    binary_tracks: Vec::new(),
                    chdman_command: String::new(),
                    status: TaskStatus::Verified,
                }],
                target_m3u_path: None,
                confidence: 0.7,
                source: ClassificationSource::Fallback,
                enabled: true,
                needs_review: false,
                status_note: None,
                role: String::new(),
                artwork_url: None,
                target_media_paths: Vec::new(),
            },
        ],
        skipped_sources: Vec::new(),
        total_source_bytes: 100,
        estimated_output_bytes: 60,
    }
}

/// PNG-ish payload comfortably over the minimum-size guard.
fn art_bytes() -> Vec<u8> {
    let mut b = b"\x89PNG\r\n\x1a\n".to_vec();
    b.extend(std::iter::repeat_n(0x42u8, 400));
    b
}

#[tokio::test]
async fn test_finish_library_esde_gamelist_and_artwork() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let dir = tempdir().unwrap();
    let plan = make_plan(dir.path(), FrontendPreset::EsDe);
    let art = art_bytes();

    // Local stand-in for the libretro thumbnail service: serves box art for
    // "Final Fantasy VII", 404s everything else (forcing the fallback chain
    // to exhaust for "No Art Game").
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let art_for_server = art.clone();
    let handle = tokio::spawn(async move {
        for _ in 0..20 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut req = [0u8; 2048];
            let n = socket.read(&mut req).await.unwrap_or(0);
            let req = String::from_utf8_lossy(&req[..n]).to_string();
            let path = req.split_whitespace().nth(1).unwrap_or_default().to_string();

            let (status, body): (&str, Vec<u8>) = if path.contains("Final%20Fantasy%20VII") {
                ("200 OK", art_for_server.clone())
            } else {
                ("404 Not Found", Vec::new())
            };
            let response = format!(
                "HTTP/1.1 {}\r\nContent-Length: {}\r\nContent-Type: image/png\r\nConnection: close\r\n\r\n",
                status,
                body.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            socket.write_all(&body).await.unwrap();
            socket.flush().await.unwrap();
        }
    });

    let base = format!("http://127.0.0.1:{}", port);
    let emitter = MockEventSink::new();
    let summary = finish_library_internal(&emitter, &plan, true, Some(&base))
        .await
        .expect("finish_library_internal");

    handle.abort();

    assert_eq!(summary.artwork_downloaded, 1);
    assert_eq!(summary.artwork_failed, 1);
    assert_eq!(summary.gamelists_written, 1);

    // Art landed in media/images named after the display file (the m3u stem).
    let art_path = dir
        .path()
        .join("roms")
        .join("psx")
        .join("media")
        .join("images")
        .join("Final Fantasy VII (USA).png");
    assert!(art_path.exists());
    assert_eq!(std::fs::read(&art_path).unwrap(), art);

    // Gamelist is centralized in the ES-DE home tree (ES-DE 3.x ignores
    // ROM-folder gamelists — verified live against 3.4.1).
    let gamelist = dir.path().join("ES-DE").join("gamelists").join("psx").join("gamelist.xml");
    let xml = std::fs::read_to_string(&gamelist).unwrap();
    assert!(xml.contains("<path>./Final Fantasy VII (USA).m3u</path>"));
    assert!(xml.contains("<name>Final Fantasy VII</name>"));
    assert!(xml.contains("<image>./media/images/Final Fantasy VII (USA).png</image>"));
    assert!(xml.contains("<path>./No Art Game (USA).chd</path>"));
    assert!(!xml.contains("<name>No Art Game</name>\n    <image>"));

    // Progress events cover both outcomes.
    let events = emitter.artwork_events.lock().unwrap().clone();
    assert!(events.iter().any(|e| e.status == ArtworkStatus::Done));
    assert!(events.iter().any(|e| e.status == ArtworkStatus::Failed));
    assert!(events.iter().any(|e| e.status == ArtworkStatus::Downloading));

    // Second run: existing art is skipped, not re-downloaded.
    let summary2 = finish_library_internal(&emitter, &plan, true, Some(&base))
        .await
        .expect("second run");
    assert_eq!(summary2.artwork_skipped, 1);
    assert_eq!(summary2.artwork_failed, 1);
}

#[tokio::test]
async fn test_finish_library_non_gamelist_preset_places_adjacent_art() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let dir = tempdir().unwrap();
    let plan = make_plan(dir.path(), FrontendPreset::AnbernicStock);
    let art = art_bytes();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let art_for_server = art.clone();
    let handle = tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
        let mut req = [0u8; 2048];
        let n = socket.read(&mut req).await.unwrap_or(0);
        let req = String::from_utf8_lossy(&req[..n]).to_string();
        let path = req.split_whitespace().nth(1).unwrap_or_default().to_string();
        let (status, body): (&str, Vec<u8>) = if path.contains("Final%20Fantasy%20VII") {
            ("200 OK", art_for_server.clone())
        } else {
            ("404 Not Found", Vec::new())
        };
        let response = format!(
            "HTTP/1.1 {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            status,
            body.len()
        );
        socket.write_all(response.as_bytes()).await.unwrap();
        socket.write_all(&body).await.unwrap();
        socket.flush().await.unwrap();
        }
    });

    let base = format!("http://127.0.0.1:{}", port);
    let emitter = MockEventSink::new();
    let summary = finish_library_internal(&emitter, &plan, true, Some(&base))
        .await
        .expect("finish_library_internal");
    handle.abort();

    // No gamelist for Anbernic stock; art sits next to the playlist.
    assert_eq!(summary.gamelists_written, 0);
    assert!(!dir.path().join("roms").join("psx").join("gamelist.xml").exists());
    let adjacent = dir
        .path()
        .join("roms")
        .join("psx")
        .join("Final Fantasy VII (USA).png");
    assert!(adjacent.exists());
    assert_eq!(std::fs::read(&adjacent).unwrap(), art);
}

#[tokio::test]
async fn test_finish_library_without_download_only_writes_gamelists() {
    let dir = tempdir().unwrap();
    let plan = make_plan(dir.path(), FrontendPreset::Batocera);

    let emitter = MockEventSink::new();
    let summary = finish_library_internal(&emitter, &plan, false, Some("http://127.0.0.1:1"))
        .await
        .expect("finish_library_internal without artwork");

    assert_eq!(summary.gamelists_written, 1);
    assert_eq!(summary.artwork_downloaded, 0);
    assert!(!dir
        .path()
        .join("roms")
        .join("psx")
        .join("media")
        .join("images")
        .exists());
}

#[tokio::test]
async fn test_finish_library_onion_artwork_goes_to_imgs_folder() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let dir = tempdir().unwrap();
    let plan = make_plan(dir.path(), FrontendPreset::OnionOs);
    let art = art_bytes();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let art_for_server = art.clone();
    let handle = tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let mut req = [0u8; 2048];
            let n = socket.read(&mut req).await.unwrap_or(0);
            let req = String::from_utf8_lossy(&req[..n]).to_string();
            let path = req.split_whitespace().nth(1).unwrap_or_default().to_string();
            let (status, body): (&str, Vec<u8>) = if path.contains("Final%20Fantasy%20VII") {
                ("200 OK", art_for_server.clone())
            } else {
                ("404 Not Found", Vec::new())
            };
            let response = format!(
                "HTTP/1.1 {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                status,
                body.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            socket.write_all(&body).await.unwrap();
            socket.flush().await.unwrap();
        }
    });

    let base = format!("http://127.0.0.1:{}", port);
    let emitter = MockEventSink::new();
    let summary = finish_library_internal(&emitter, &plan, true, Some(&base))
        .await
        .expect("finish");
    handle.abort();

    // Onion convention (verified on a real Onion card): Imgs/ subfolder.
    let imgs = dir
        .path()
        .join("roms")
        .join("psx")
        .join("Imgs")
        .join("Final Fantasy VII (USA).png");
    assert!(imgs.exists(), "art must land in Imgs/: {}", imgs.display());
    assert!(!dir
        .path()
        .join("roms")
        .join("psx")
        .join("Final Fantasy VII (USA).png")
        .exists());
}
