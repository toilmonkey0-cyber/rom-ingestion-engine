//! Live sidecar integration test — runs only when NEEDLE_LIVE_URL is set,
//! e.g. `NEEDLE_LIVE_URL=http://127.0.0.1:8080 cargo test --test needle_live_test`.
//! Skips silently otherwise (CI, plain `cargo test`).

use rom_ingest_core::commands::scan_and_plan;
use rom_ingest_core::models::{ClassificationSource, FrontendPreset};
use std::fs;
use tempfile::TempDir;

fn live_url() -> Option<String> {
    std::env::var("NEEDLE_LIVE_URL").ok().filter(|u| !u.trim().is_empty())
}

fn fixture(dir: &TempDir, cue_name: &str) {
    let stem = cue_name.trim_end_matches(".cue");
    let bin = dir.path().join(format!("{stem}.bin"));
    fs::write(&bin, b"disc-image-bytes").unwrap();
    let cue = dir.path().join(cue_name);
    fs::write(
        &cue,
        format!("FILE \"{stem}.bin\" BINARY\n  TRACK 01 MODE1/2352\n"),
    )
    .unwrap();
}

#[tokio::test]
async fn live_sidecar_classifies_as_needle_source() {
    let Some(url) = live_url() else {
        eprintln!("skipping: NEEDLE_LIVE_URL not set");
        return;
    };
    let dir = TempDir::new().unwrap();
    fixture(&dir, "Final Fantasy VII (USA) (Disc 2).cue");

    let plan = scan_and_plan(
        dir.path().to_string_lossy().to_string(),
        dir.path().join("out").to_string_lossy().to_string(),
        FrontendPreset::EsDe,
        None,
        None,
        None,
        None,
        None,
        Some(url),
        None,
        None,
    )
    .await
    .expect("scan_and_plan with live sidecar should succeed");

    assert_eq!(plan.games.len(), 1);
    assert_eq!(
        plan.games[0].source,
        ClassificationSource::NeedleAI,
        "a live engine that answers must become the classification source"
    );
}

#[tokio::test]
async fn dead_sidecar_falls_back_with_visible_note() {
    let dir = TempDir::new().unwrap();
    fixture(&dir, "Mystery (USA).cue");

    let plan = scan_and_plan(
        dir.path().to_string_lossy().to_string(),
        dir.path().join("out").to_string_lossy().to_string(),
        FrontendPreset::EsDe,
        None,
        None,
        None,
        None,
        None,
        // reserved port: connect fails fast, transport error -> fallback tier
        Some("http://127.0.0.1:1".to_string()),
        None,
        None,
    )
    .await
    .expect("scan_and_plan with dead sidecar should succeed");

    assert_eq!(plan.games.len(), 1);
    assert_eq!(plan.games[0].source, ClassificationSource::Fallback);
    assert!(
        plan.games[0]
            .status_note
            .as_deref()
            .unwrap_or_default()
            .contains("Needle classification failed"),
        "dead sidecar must surface a visible note, got: {:?}",
        plan.games[0].status_note
    );
}
