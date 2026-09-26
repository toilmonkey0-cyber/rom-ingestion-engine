use rom_ingest_core::classifier::needle::*;
use rom_ingest_core::models::{ClassificationSource, Platform};
use std::time::Duration;

#[test]
fn test_needle_response_parsing_success() {
    let response_json = r#"{
      "type": "call",
      "success": true,
      "function_calls": [
        {
          "name": "classify_rom",
          "arguments": {
            "platform": "psx",
            "is_multidisc": true,
            "disc_number": "2",
            "region": "USA"
          }
        }
      ],
      "suppressed_calls": [],
      "confidence": 0.94,
      "reasoning": "platform 'psx' from query; disc_number 2 from 'Disc 2'",
      "prefill_tps": 1400.0,
      "decode_tps": 690.0,
      "peak_ram_mb": 80.8
    }"#;

    let classification =
        parse_needle_response(response_json, "Final Fantasy VII (USA) (Disc 2).cue").unwrap();
    assert_eq!(classification.platform, Platform::Psx);
    assert_eq!(classification.region, "USA");
    assert!(classification.is_multidisc);
    assert_eq!(classification.disc_number, Some(2));
    assert_eq!(classification.source, ClassificationSource::NeedleAI);
    assert_eq!(classification.canonical_title, "Final Fantasy VII");
    assert!((classification.confidence - 0.94).abs() < 1e-4);
}

#[test]
fn test_needle_response_parsing_withheld_call() {
    // The engine withholds low-confidence calls into suppressed_calls;
    // the arguments are still there, so the classification is usable but
    // its confidence must be discounted.
    let response_json = r#"{
      "type": "call",
      "success": true,
      "function_calls": [],
      "suppressed_calls": [
        {
          "name": "classify_rom",
          "arguments": {
            "platform": "saturn",
            "is_multidisc": false,
            "disc_number": "single",
            "region": "Japan"
          }
        }
      ],
      "confidence": 0.60,
      "reasoning": "unsure"
    }"#;

    let classification =
        parse_needle_response(response_json, "Nights into Dreams (Japan).cue").unwrap();
    assert_eq!(classification.platform, Platform::Saturn);
    assert_eq!(classification.region, "Japan");
    assert!(!classification.is_multidisc);
    assert_eq!(classification.disc_number, None);
    // withheld: 0.60 * 0.5 = 0.30
    assert!((classification.confidence - 0.30).abs() < 1e-4);
}

#[test]
fn test_needle_response_no_call_is_error() {
    // Off-topic input returns empty calls everywhere: the caller must fall
    // back to the heuristic tier.
    let response_json = r#"{
      "type": "call",
      "success": true,
      "function_calls": [],
      "suppressed_calls": [],
      "confidence": 0.9,
      "reasoning": ""
    }"#;

    let err = parse_needle_response(response_json, "System Firmware 3.15 Update.iso").unwrap_err();
    assert!(matches!(err, NeedleError::NoCall));
}

#[test]
fn test_needle_out_of_vocabulary_platform_is_unknown_and_penalized() {
    // The decode grammar should make this impossible; the Rust parser
    // defends anyway so a hostile or buggy sidecar cannot inject an
    // unvalidated platform.
    let response_json = r#"{
      "type": "call",
      "success": true,
      "function_calls": [
        {
          "name": "classify_rom",
          "arguments": {
            "platform": "nintendo-switch",
            "is_multidisc": false,
            "disc_number": "single",
            "region": "World"
          }
        }
      ],
      "suppressed_calls": [],
      "confidence": 0.95,
      "reasoning": ""
    }"#;

    let classification =
        parse_needle_response(response_json, "Weird Homebrew Thing (World).cue").unwrap();
    assert_eq!(classification.platform, Platform::Unknown);
    // penalized like the Jev parser does for out-of-vocabulary answers
    assert!(classification.confidence <= 0.30);
}

#[test]
fn test_needle_region_and_disc_fallbacks() {
    // Region answers outside the vocabulary fall back to the filename tags;
    // disc numbers out of range are dropped.
    let response_json = r#"{
      "type": "call",
      "success": true,
      "function_calls": [
        {
          "name": "classify_rom",
          "arguments": {
            "platform": "segacd",
            "is_multidisc": true,
            "disc_number": "9",
            "region": "PAL"
          }
        }
      ],
      "suppressed_calls": [],
      "confidence": null,
      "reasoning": ""
    }"#;

    let classification =
        parse_needle_response(response_json, "Sonic CD (Europe) (Disc 2 of 2).cue").unwrap();
    assert_eq!(classification.platform, Platform::SegaCd);
    // "PAL" is not in the vocabulary; falls back to the (Europe) tag
    assert_eq!(classification.region, "Europe");
    assert!(classification.is_multidisc);
    // "9" is out of range: the model answer is dropped, filename Disc 2 wins
    assert_eq!(classification.disc_number, Some(2));
    // confidence None (local LoRA build): neutral default
    assert!((classification.confidence - 0.75).abs() < 1e-4);
}

#[tokio::test]
async fn test_needle_client_reset_then_complete_and_payload() {
    use rom_ingest_core::classifier::needle::NeedleClient;
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://127.0.0.1:{}", addr.port());

    let seen = Arc::new(tokio::sync::Mutex::new(Vec::<String>::new()));
    let seen_clone = Arc::clone(&seen);

    tokio::spawn(async move {
        let body = r#"{
          "type": "call",
          "success": true,
          "function_calls": [
            {"name": "classify_rom", "arguments": {"platform": "dreamcast", "is_multidisc": true, "disc_number": "1", "region": "USA"}}
          ],
          "suppressed_calls": [],
          "confidence": 0.9,
          "reasoning": ""
        }"#;
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        // Request 1: POST /reset -> empty 200
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0u8; 4096];
            let n = socket.read(&mut buf).await.unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            seen_clone.lock().await.push(req);
            let empty = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            let _ = socket.write_all(empty.as_bytes()).await;
        }
        // Request 2: POST /complete -> classification JSON
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0u8; 4096];
            let n = socket.read(&mut buf).await.unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            seen_clone.lock().await.push(req);
            let _ = socket.write_all(resp.as_bytes()).await;
        }
    });

    let client = NeedleClient::with_base_url(base_url)
        .with_backoff(Duration::from_millis(10))
        .with_max_retries(2);

    let result = client
        .evaluate("Shenmue (USA) (Disc 1).gdi", "Dreamcast")
        .await
        .expect("Evaluation should succeed");

    assert_eq!(result.platform, Platform::Dreamcast);
    assert_eq!(result.disc_number, Some(1));

    let requests = seen.lock().await;
    // The engine keeps ONE process-global conversation: every file must start
    // with POST /reset before POST /complete, or answers leak across files.
    assert!(requests.len() >= 2, "expected reset + complete requests");
    assert!(requests[0].starts_with("POST /reset"));
    assert!(requests[1].starts_with("POST /complete"));
    // Folder context is folded into the input text (it is a strong platform
    // signal), and the input is the bare filename plus folder: no verbs that
    // can leak into grounded fields.
    assert!(requests[1].contains(r#""input":"Dreamcast/Shenmue (USA) (Disc 1).gdi""#));
}

#[tokio::test]
async fn test_needle_client_connection_failure_is_error() {
    use rom_ingest_core::classifier::needle::NeedleClient;

    // Port 1 on localhost: reserved, nothing listens there.
    let client = NeedleClient::with_base_url("http://127.0.0.1:1".to_string())
        .with_backoff(Duration::from_millis(5))
        .with_max_retries(2);

    let result = client.evaluate("Game (USA).cue", "").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), NeedleError::NetworkError(_)));
}
