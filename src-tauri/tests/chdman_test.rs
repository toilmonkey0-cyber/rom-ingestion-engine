// src-tauri/tests/chdman_test.rs
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tempfile::tempdir;

use rom_ingest_core::chdman::runner::{
    parse_chdman_progress_line, verify_chd_header, ChdmanError, ChdmanRunner,
};
use rom_ingest_core::paths::chdman_command_for_input;

#[test]
fn test_parse_chdman_progress() {
    assert_eq!(parse_chdman_progress_line("Compressing, 42.8% complete..."), Some(42.8));
    assert_eq!(parse_chdman_progress_line("Compressing, 100.0% complete..."), Some(100.0));
    assert_eq!(parse_chdman_progress_line("Compressing, 0.0% complete..."), Some(0.0));
    assert_eq!(
        parse_chdman_progress_line("chdman - MAME Compressed Hunks of Data (CHD) manager 0.268"),
        None
    );
    assert_eq!(parse_chdman_progress_line("Error opening input file: not found"), None);
}

#[test]
fn test_verify_chd_header() {
    let dir = tempdir().unwrap();
    let valid_chd = dir.path().join("valid.chd");
    let mut f = File::create(&valid_chd).unwrap();
    f.write_all(b"MComprHD\x00\x00\x00\x05dummychddata").unwrap();

    let invalid_chd = dir.path().join("invalid.chd");
    let mut f2 = File::create(&invalid_chd).unwrap();
    f2.write_all(b"NOTACHD\x00dummydata").unwrap();

    assert!(verify_chd_header(&valid_chd).unwrap());
    assert!(!verify_chd_header(&invalid_chd).unwrap());
}

#[test]
fn test_verify_chd_header_edge_cases() {
    let dir = tempdir().unwrap();

    // 0 bytes
    let empty_file = dir.path().join("empty.chd");
    File::create(&empty_file).unwrap();
    assert!(!verify_chd_header(&empty_file).unwrap());

    // Less than 8 bytes
    let short_file = dir.path().join("short.chd");
    let mut f = File::create(&short_file).unwrap();
    f.write_all(b"MCom").unwrap();
    assert!(!verify_chd_header(&short_file).unwrap());

    // Non-existent file
    let missing_file = dir.path().join("does_not_exist.chd");
    assert!(verify_chd_header(&missing_file).is_err());
}

#[test]
fn test_parse_chdman_progress_variations() {
    // Carriage return prefix
    assert_eq!(
        parse_chdman_progress_line("\rCompressing, 33.3% complete..."),
        Some(33.3)
    );
    // Lowercase
    assert_eq!(
        parse_chdman_progress_line("compressing, 99.9% complete..."),
        Some(99.9)
    );
    // Integer percent without decimal point
    assert_eq!(
        parse_chdman_progress_line("Compressing, 50% complete..."),
        Some(50.0)
    );
    // Extra spaces
    assert_eq!(
        parse_chdman_progress_line("Compressing,   75.5%   complete..."),
        Some(75.5)
    );
}

#[test]
fn test_runner_constructors_and_defaults() {
    let default_runner = ChdmanRunner::default();
    assert_eq!(default_runner.binary_path(), Path::new("chdman"));

    let new_default = ChdmanRunner::new(None);
    assert_eq!(new_default.binary_path(), Path::new("chdman"));

    let custom_runner = ChdmanRunner::new(Some(PathBuf::from("/usr/bin/chdman")));
    assert_eq!(custom_runner.binary_path(), Path::new("/usr/bin/chdman"));
}

#[tokio::test]
async fn test_chdman_runner_binary_not_found() {
    let runner = ChdmanRunner::new(Some(PathBuf::from("nonexistent_chdman_binary_xyz123")));
    let input = PathBuf::from("game.cue");
    let output = PathBuf::from("game.chd");

    let result = runner.convert(&input, &output, |_| {}).await;
    match result {
        Err(ChdmanError::BinaryNotFound) => {} // Expected
        other => panic!("Expected BinaryNotFound, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_chdman_runner_convert_success() {
    let mock_bin = PathBuf::from(env!("CARGO_BIN_EXE_mock_chdman"));
    let runner = ChdmanRunner::new(Some(mock_bin));

    let dir = tempdir().unwrap();
    let input = dir.path().join("disc_success.cue");
    File::create(&input).unwrap();

    let output = dir.path().join("disc_success.chd");
    let part_file = dir.path().join("disc_success.chd.part");

    let progress_updates = Arc::new(Mutex::new(Vec::new()));
    let progress_clone = Arc::clone(&progress_updates);

    let result = runner
        .convert(&input, &output, move |pct| {
            progress_clone.lock().unwrap().push(pct);
        })
        .await;

    assert!(result.is_ok(), "Expected conversion to succeed: {:?}", result.err());

    // Verify progress callbacks
    let updates = progress_updates.lock().unwrap().clone();
    assert_eq!(updates, vec![25.0, 50.0, 100.0]);

    // Verify output file exists and has valid magic header
    assert!(output.exists(), "Final .chd output must exist");
    assert!(verify_chd_header(&output).unwrap(), "Output must have valid CHD header");

    // Verify .part file was atomically renamed away
    assert!(!part_file.exists(), ".part file must not exist after successful rename");
}

#[tokio::test]
async fn test_chdman_runner_convert_process_failed() {
    let mock_bin = PathBuf::from(env!("CARGO_BIN_EXE_mock_chdman"));
    let runner = ChdmanRunner::new(Some(mock_bin));

    let dir = tempdir().unwrap();
    let input = dir.path().join("disc_failure.cue");
    File::create(&input).unwrap();

    let output = dir.path().join("disc_failure.chd");
    let part_file = dir.path().join("disc_failure.chd.part");

    let result = runner.convert(&input, &output, |_| {}).await;

    match result {
        Err(ChdmanError::ProcessFailed { code, stderr }) => {
            assert_eq!(code, Some(1));
            assert!(
                stderr.contains("could not be parsed"),
                "stderr should contain error message: {}",
                stderr
            );
        }
        other => panic!("Expected ProcessFailed, got: {:?}", other),
    }

    // Verify .part file was cleaned up on failure
    assert!(!part_file.exists(), ".part file must be removed upon process failure");
    assert!(!output.exists(), "Output .chd must not exist upon failure");
}

#[tokio::test]
async fn test_chdman_runner_convert_header_verification_failure() {
    let mock_bin = PathBuf::from(env!("CARGO_BIN_EXE_mock_chdman"));
    let runner = ChdmanRunner::new(Some(mock_bin));

    let dir = tempdir().unwrap();
    let input = dir.path().join("disc_bad_header.cue");
    File::create(&input).unwrap();

    let output = dir.path().join("disc_bad_header.chd");
    let part_file = dir.path().join("disc_bad_header.chd.part");

    let result = runner.convert(&input, &output, |_| {}).await;

    match result {
        Err(ChdmanError::HeaderVerificationFailed) => {} // Expected
        other => panic!("Expected HeaderVerificationFailed, got: {:?}", other),
    }

    // Verify .part file was cleaned up on header failure
    assert!(!part_file.exists(), ".part file must be removed upon header verification failure");
    assert!(!output.exists(), "Output .chd must not exist upon header verification failure");
}

#[test]
fn test_gdi_uses_createdvd_and_cue_uses_createcd() {
    assert_eq!(chdman_command_for_input(Path::new("game.gdi")).unwrap(), "createdvd");
    assert_eq!(chdman_command_for_input(Path::new("game.cue")).unwrap(), "createcd");
    assert_eq!(chdman_command_for_input(Path::new("game.iso")).unwrap(), "createcd");
}

#[tokio::test]
async fn test_existing_valid_chd_is_left_untouched() {
    let runner = ChdmanRunner::new(Some(PathBuf::from("nonexistent_chdman_binary_xyz123")));
    let dir = tempdir().unwrap();
    let input = dir.path().join("already.cue");
    File::create(&input).unwrap();
    let output = dir.path().join("already.chd");
    let original = b"MComprHDkeep-this-file";
    File::create(&output).unwrap().write_all(original).unwrap();

    let progress = Arc::new(Mutex::new(Vec::new()));
    let progress_clone = Arc::clone(&progress);
    let result = runner
        .convert(&input, &output, move |pct| progress_clone.lock().unwrap().push(pct))
        .await;

    assert!(result.is_ok(), "{:?}", result.err());
    assert_eq!(std::fs::read(&output).unwrap(), original);
    assert_eq!(progress.lock().unwrap().as_slice(), &[100.0]);
}

#[tokio::test]
async fn test_stderr_progress_is_reported() {
    let runner = ChdmanRunner::new(Some(PathBuf::from(env!("CARGO_BIN_EXE_mock_chdman"))));
    let dir = tempdir().unwrap();
    let input = dir.path().join("disc_stderr_progress.cue");
    File::create(&input).unwrap();
    let output = dir.path().join("disc_stderr_progress.chd");
    let progress = Arc::new(Mutex::new(Vec::new()));
    let progress_clone = Arc::clone(&progress);

    let result = runner
        .convert(&input, &output, move |pct| progress_clone.lock().unwrap().push(pct))
        .await;

    assert!(result.is_ok(), "{:?}", result.err());
    assert_eq!(progress.lock().unwrap().as_slice(), &[40.0, 100.0]);
}
