use std::fs::File;
use std::io::Write;
use tempfile::tempdir;
use rom_ingest_core::chdman::downloader::*;

#[test]
fn test_platform_manifest_resolution() {
    let win_manifest = get_platform_manifest("windows", "x86_64");
    assert!(win_manifest.is_some());
    let m = win_manifest.unwrap();
    assert_eq!(m.binary_name, "chdman.exe");
    assert!(!m.download_url.is_empty());
    assert_eq!(m.expected_sha256.len(), 64);

    let linux_manifest = get_platform_manifest("linux", "x86_64");
    assert!(linux_manifest.is_some());
    let l = linux_manifest.unwrap();
    assert_eq!(l.binary_name, "chdman");
    assert_eq!(l.archive_format, ArchiveFormat::TarGz);

    let mac_arm = get_platform_manifest("darwin", "arm64");
    assert!(mac_arm.is_some());
    assert_eq!(mac_arm.unwrap().binary_name, "chdman");

    let mac_x64 = get_platform_manifest("macos", "x86_64");
    assert!(mac_x64.is_some());
    assert_eq!(mac_x64.unwrap().binary_name, "chdman");

    let unsupported = get_platform_manifest("freebsd", "sparc64");
    assert!(unsupported.is_none());
}

#[test]
fn test_compute_sha256() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.bin");
    let mut f = File::create(&file_path).unwrap();
    f.write_all(b"test data for sha256 calculation").unwrap();

    let hash = compute_file_sha256(&file_path).unwrap();
    assert_eq!(hash.len(), 64);

    // Known SHA-256 for empty file: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
    let empty_path = dir.path().join("empty.bin");
    File::create(&empty_path).unwrap();
    let empty_hash = compute_file_sha256(&empty_path).unwrap();
    assert_eq!(
        empty_hash,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn test_managed_tools_dir() {
    let dir_res = get_managed_tools_dir();
    assert!(dir_res.is_ok());
    let dir = dir_res.unwrap();
    assert!(dir.to_string_lossy().ends_with("bin"));
}

#[test]
fn test_checksum_mismatch_purges_temporary_file() {
    let dir = tempdir().unwrap();
    let dest_dir = dir.path().to_path_buf();
    let corrupt_file = dest_dir.join("corrupt.tmp");
    let mut f = File::create(&corrupt_file).unwrap();
    f.write_all(b"corrupt bytes").unwrap();

    let res = verify_and_install_download(
        &corrupt_file,
        "expected_real_sha256_00000000000000000000000000000000000000000000000",
        ArchiveFormat::RawBinary,
        &dest_dir,
        "chdman.exe",
    );
    assert!(res.is_err());
    assert!(!corrupt_file.exists()); // Must be deleted on mismatch!
}

#[test]
fn test_extract_archive_raw_binary() {
    let dir = tempdir().unwrap();
    let source_binary = dir.path().join("source.bin");
    let mut f = File::create(&source_binary).unwrap();
    f.write_all(b"dummy binary contents").unwrap();

    let dest_dir = dir.path().join("installed");
    let installed = extract_archive(
        &source_binary,
        ArchiveFormat::RawBinary,
        &dest_dir,
        "chdman.exe",
    )
    .unwrap();

    assert_eq!(installed, dest_dir.join("chdman.exe"));
    assert!(installed.exists());
    let contents = std::fs::read(&installed).unwrap();
    assert_eq!(contents, b"dummy binary contents");
}

#[test]
fn test_extract_archive_zip() {
    let dir = tempdir().unwrap();
    let zip_path = dir.path().join("test.zip");

    // Build a test zip containing subfolder/chdman.exe
    {
        let file = File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("chdman-0.268/chdman.exe", options).unwrap();
        zip.write_all(b"zip binary contents").unwrap();
        zip.finish().unwrap();
    }

    let dest_dir = dir.path().join("installed_zip");
    let installed = extract_archive(
        &zip_path,
        ArchiveFormat::Zip,
        &dest_dir,
        "chdman.exe",
    )
    .unwrap();

    assert_eq!(installed, dest_dir.join("chdman.exe"));
    assert!(installed.exists());
    let contents = std::fs::read(&installed).unwrap();
    assert_eq!(contents, b"zip binary contents");
}

#[test]
fn test_extract_archive_tar_gz() {
    let dir = tempdir().unwrap();
    let tar_gz_path = dir.path().join("test.tar.gz");

    // Build a test tar.gz containing chdman
    {
        let file = File::create(&tar_gz_path).unwrap();
        let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut tar = tar::Builder::new(enc);

        let data = b"targz binary contents";
        let mut header = tar::Header::new_gnu();
        header.set_path("chdman-0.268/chdman").unwrap();
        header.set_size(data.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();

        tar.append(&header, &data[..]).unwrap();
        tar.finish().unwrap();
    }

    let dest_dir = dir.path().join("installed_targz");
    let installed = extract_archive(
        &tar_gz_path,
        ArchiveFormat::TarGz,
        &dest_dir,
        "chdman",
    )
    .unwrap();

    assert_eq!(installed, dest_dir.join("chdman"));
    assert!(installed.exists());
    let contents = std::fs::read(&installed).unwrap();
    assert_eq!(contents, b"targz binary contents");
}

#[test]
fn test_verify_and_install_download_success() {
    let dir = tempdir().unwrap();
    let dest_dir = dir.path().join("tools_bin");
    let tmp_file = dest_dir.join("chdman_download.tmp");

    std::fs::create_dir_all(&dest_dir).unwrap();
    let mut f = File::create(&tmp_file).unwrap();
    f.write_all(b"valid binary bytes").unwrap();
    drop(f);

    let sha256 = compute_file_sha256(&tmp_file).unwrap();

    let installed = verify_and_install_download(
        &tmp_file,
        &sha256,
        ArchiveFormat::RawBinary,
        &dest_dir,
        "chdman.exe",
    )
    .unwrap();

    assert_eq!(installed, dest_dir.join("chdman.exe"));
    assert!(installed.exists());
    assert!(!tmp_file.exists()); // tmp file must be cleaned up on success!
}
