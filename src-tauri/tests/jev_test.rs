use rom_ingest_core::classifier::jev::*;
use rom_ingest_core::models::{ClassificationSource, Platform};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[test]
fn test_jev_request_building_and_response_parsing() {
    let payload = build_jev_request("Final Fantasy VII (USA) (Disc 2).cue", "PSX");
    assert_eq!(payload.model, "jev-latest");
    assert!(payload.questions.contains_key("is_multidisc"));
    assert!(payload.questions.contains_key("disc_number"));
    assert!(payload.questions.contains_key("platform"));
    assert!(payload.questions.contains_key("region"));

    let mock_response_json = r#"{
      "model": "jev-1.13.0",
      "answers": {
        "platform": {
          "type": "choice",
          "choice": "psx",
          "probabilities": { "psx": 0.99, "saturn": 0.01 },
          "confidence": 0.98
        },
        "is_multidisc": {
          "type": "noul",
          "noul": 0.99
        },
        "disc_number": {
          "type": "choice",
          "choice": "2",
          "probabilities": { "2": 0.97, "1": 0.03 },
          "confidence": 0.95
        },
        "region": {
          "type": "choice",
          "choice": "USA",
          "probabilities": { "USA": 0.99 },
          "confidence": 0.99
        }
      },
      "usage": { "input_tokens": 250, "output_tokens": 30 }
    }"#;

    let classification =
        parse_jev_response(mock_response_json, "Final Fantasy VII (USA) (Disc 2).cue").unwrap();
    assert_eq!(classification.disc_number, Some(2));
    assert!(classification.is_multidisc);
    assert_eq!(classification.region, "USA");
    assert_eq!(classification.platform, Platform::Psx);
    assert_eq!(classification.source, ClassificationSource::JevAI);
    assert!(classification.confidence >= 0.9);
}

#[test]
fn test_jev_single_disc_response_parsing() {
    let mock_response_json = r#"{
      "model": "jev-1.13.0",
      "answers": {
        "platform": {
          "type": "choice",
          "choice": "saturn",
          "probabilities": { "saturn": 0.95 },
          "confidence": 0.95
        },
        "is_multidisc": {
          "type": "noul",
          "noul": 0.02
        },
        "disc_number": {
          "type": "choice",
          "choice": "single",
          "probabilities": { "single": 0.98 },
          "confidence": 0.98
        },
        "region": {
          "type": "choice",
          "choice": "JPN",
          "probabilities": { "JPN": 0.95 },
          "confidence": 0.95
        }
      },
      "usage": { "input_tokens": 200, "output_tokens": 25 }
    }"#;

    let classification =
        parse_jev_response(mock_response_json, "Nights into Dreams (Japan).cue").unwrap();
    assert_eq!(classification.disc_number, None);
    assert!(!classification.is_multidisc);
    assert_eq!(classification.region, "Japan");
    assert_eq!(classification.platform, Platform::Saturn);
}

#[test]
fn test_jev_request_serialization() {
    let req = build_jev_request("Shenmue (USA) (Disc 1).gdi", "Dreamcast");
    let serialized = serde_json::to_string(&req).expect("Failed to serialize JevRequest");

    assert!(serialized.contains("\"model\":\"jev-latest\""));
    assert!(serialized.contains("\"filename\":\"Shenmue (USA) (Disc 1).gdi\""));
    assert!(serialized.contains("\"folder\":\"Dreamcast\""));
    assert!(serialized.contains("\"clean_filename\":\"Shenmue\""));

    let deserialized: JevRequest =
        serde_json::from_str(&serialized).expect("Failed to deserialize JevRequest");
    assert_eq!(deserialized.model, "jev-latest");
    assert_eq!(deserialized.questions.len(), 4);

    let platform_q = deserialized.questions.get("platform").unwrap();
    assert_eq!(platform_q.primitive(), "choice");
    assert!(platform_q.criteria().is_some());

    let multidisc_q = deserialized.questions.get("is_multidisc").unwrap();
    assert_eq!(multidisc_q.primitive(), "noul");
}

#[test]
fn test_jev_other_platforms() {
    // Dreamcast
    let dc_json = r#"{
      "model": "jev-latest",
      "answers": {
        "platform": { "type": "choice", "choice": "dreamcast", "confidence": 0.99 },
        "is_multidisc": { "type": "noul", "noul": 0.95 },
        "disc_number": { "type": "choice", "choice": "1", "confidence": 0.99 },
        "region": { "type": "choice", "choice": "USA", "confidence": 0.99 }
      }
    }"#;
    let dc = parse_jev_response(dc_json, "Shenmue (USA) (Disc 1).gdi").unwrap();
    assert_eq!(dc.platform, Platform::Dreamcast);
    assert_eq!(dc.disc_number, Some(1));
    assert!(dc.is_multidisc);

    // Sega CD
    let scd_json = r#"{
      "model": "jev-latest",
      "answers": {
        "platform": { "type": "choice", "choice": "segacd", "confidence": 0.95 },
        "is_multidisc": { "type": "noul", "noul": 0.01 },
        "disc_number": { "type": "choice", "choice": "single", "confidence": 0.98 },
        "region": { "type": "choice", "choice": "USA", "confidence": 0.95 }
      }
    }"#;
    let scd = parse_jev_response(scd_json, "Sonic CD (USA).cue").unwrap();
    assert_eq!(scd.platform, Platform::SegaCd);
    assert_eq!(scd.disc_number, None);
    assert!(!scd.is_multidisc);

    // PC Engine CD
    let pce_json = r#"{
      "model": "jev-latest",
      "answers": {
        "platform": { "type": "choice", "choice": "pcecd", "confidence": 0.97 },
        "is_multidisc": { "type": "noul", "noul": 0.05 },
        "disc_number": { "type": "choice", "choice": "single", "confidence": 0.97 },
        "region": { "type": "choice", "choice": "Japan", "confidence": 0.97 }
      }
    }"#;
    let pce = parse_jev_response(pce_json, "Akumajou Dracula X - Chi no Rondo (Japan).cue").unwrap();
    assert_eq!(pce.platform, Platform::PceCd);
    assert_eq!(pce.disc_number, None);
    assert!(!pce.is_multidisc);
}

#[test]
fn test_jev_error_handling() {
    // Malformed JSON
    let bad_json = "{ invalid json }";
    assert!(matches!(
        parse_jev_response(bad_json, "Game.cue"),
        Err(JevError::JsonError(_))
    ));

    // Missing platform
    let missing_platform = r#"{
      "model": "jev-latest",
      "answers": {
        "is_multidisc": { "type": "noul", "noul": 0.0 }
      }
    }"#;
    assert!(matches!(
        parse_jev_response(missing_platform, "Game.cue"),
        Err(JevError::MissingAnswer(q)) if q == "platform"
    ));

    // Invalid answer type for platform (noul instead of choice)
    let invalid_type = r#"{
      "model": "jev-latest",
      "answers": {
        "platform": { "type": "noul", "noul": 0.8 }
      }
    }"#;
    assert!(matches!(
        parse_jev_response(invalid_type, "Game.cue"),
        Err(JevError::InvalidAnswerType { .. })
    ));
}

#[test]
fn test_jev_client_config() {
    let client = JevClient::new("test_api_key".to_string());
    assert_eq!(client.base_url(), JEV_DEFAULT_ENDPOINT);

    let custom = JevClient::with_base_url(
        "test_api_key".to_string(),
        "http://localhost:8080/v1/systemone".to_string(),
    )
    .with_backoff(Duration::from_millis(10))
    .with_max_retries(5);

    assert_eq!(custom.base_url(), "http://localhost:8080/v1/systemone");
}

#[tokio::test]
async fn test_jev_client_mock_server_success() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://127.0.0.1:{}/v1/systemone", addr.port());

    let received_auth = Arc::new(tokio::sync::Mutex::new(String::new()));
    let received_auth_clone = Arc::clone(&received_auth);

    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0u8; 4096];
            let n = socket.read(&mut buf).await.unwrap_or(0);
            let req_str = String::from_utf8_lossy(&buf[..n]).to_string();

            for line in req_str.lines() {
                if line.to_lowercase().starts_with("authorization:") {
                    let mut lock = received_auth_clone.lock().await;
                    *lock = line.trim().to_string();
                }
            }

            let body = r#"{
              "model": "jev-1.13.0",
              "answers": {
                "platform": { "type": "choice", "choice": "psx", "confidence": 0.98 },
                "is_multidisc": { "type": "noul", "noul": 0.99 },
                "disc_number": { "type": "choice", "choice": "1", "confidence": 0.97 },
                "region": { "type": "choice", "choice": "USA", "confidence": 0.99 }
              },
              "usage": { "input_tokens": 100, "output_tokens": 20 }
            }"#;

            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = socket.write_all(resp.as_bytes()).await;
        }
    });

    let client = JevClient::with_base_url("test-secret-key-123".to_string(), base_url);
    let result = client
        .evaluate("Resident Evil 2 (USA) (Disc 1).cue", "PSX")
        .await
        .expect("Evaluation should succeed");

    assert_eq!(result.platform, Platform::Psx);
    assert_eq!(result.disc_number, Some(1));
    assert!(result.is_multidisc);
    assert_eq!(result.canonical_title, "Resident Evil 2");

    let auth_header = received_auth.lock().await;
    assert_eq!(*auth_header, "authorization: Bearer test-secret-key-123");
}

#[tokio::test]
async fn test_jev_client_mock_server_retry_on_429() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://127.0.0.1:{}/v1/systemone", addr.port());

    let attempt_count = Arc::new(AtomicUsize::new(0));
    let attempt_count_clone = Arc::clone(&attempt_count);

    tokio::spawn(async move {
        loop {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = [0u8; 4096];
                let _ = socket.read(&mut buf).await;
                let current_attempt = attempt_count_clone.fetch_add(1, Ordering::SeqCst);

                if current_attempt == 0 {
                    // First attempt returns 429 Too Many Requests
                    let body = r#"{"error": "rate limit exceeded"}"#;
                    let resp = format!(
                        "HTTP/1.1 429 Too Many Requests\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = socket.write_all(resp.as_bytes()).await;
                } else {
                    // Second attempt succeeds
                    let body = r#"{
                      "model": "jev-1.13.0",
                      "answers": {
                        "platform": { "type": "choice", "choice": "saturn", "confidence": 0.95 },
                        "is_multidisc": { "type": "noul", "noul": 0.01 },
                        "disc_number": { "type": "choice", "choice": "single", "confidence": 0.95 },
                        "region": { "type": "choice", "choice": "USA", "confidence": 0.95 }
                      }
                    }"#;
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = socket.write_all(resp.as_bytes()).await;
                    break;
                }
            }
        }
    });

    let client = JevClient::with_base_url("key".to_string(), base_url)
        .with_backoff(Duration::from_millis(10))
        .with_max_retries(3);

    let result = client
        .evaluate_game_filename("Panzer Dragoon (USA).cue", "Saturn")
        .await
        .expect("Evaluation should succeed after retry");

    assert_eq!(result.platform, Platform::Saturn);
    assert_eq!(attempt_count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_jev_client_mock_server_api_error_401() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://127.0.0.1:{}/v1/systemone", addr.port());

    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0u8; 4096];
            let _ = socket.read(&mut buf).await;
            let body = r#"{"error": "Invalid API Key"}"#;
            let resp = format!(
                "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = socket.write_all(resp.as_bytes()).await;
        }
    });

    let client = JevClient::with_base_url("bad_key".to_string(), base_url);
    let result = client.evaluate("Game.cue", "Folder").await;

    assert!(matches!(
        result,
        Err(JevError::ApiError {
            status_code: 401,
            ..
        })
    ));
}
