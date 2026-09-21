use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::commands::EventSink;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArchiveFormat {
    Zip,
    TarGz,
    RawBinary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformManifest {
    pub os: &'static str,
    pub arch: &'static str,
    pub download_url: &'static str,
    pub expected_sha256: &'static str,
    pub archive_format: ArchiveFormat,
    pub format: ArchiveFormat,
    pub binary_name: &'static str,
    pub version: &'static str,
}

impl PlatformManifest {
    pub fn format(&self) -> ArchiveFormat {
        self.archive_format
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChdmanStatus {
    pub ready: bool,
    pub source: ChdmanSource,
    pub path: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChdmanSource {
    SystemPath,
    ManagedDirectory,
    CustomPath,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DownloadProgressEvent {
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub percentage: f32,
}

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Checksum mismatch: expected {expected}, computed {actual}")]
    ChecksumMismatch {
        expected: String,
        actual: String,
    },

    #[error("Archive extraction failed: {0}")]
    ExtractionError(String),

    #[error("Binary not found in archive: expected {0}")]
    BinaryNotFound(String),

    #[error("Unsupported platform: {os} ({arch})")]
    UnsupportedPlatform {
        os: String,
        arch: String,
    },

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Server returned HTTP {status}: {message}")]
    HttpError {
        status: u16,
        message: String,
    },
}

impl From<zip::result::ZipError> for DownloadError {
    fn from(err: zip::result::ZipError) -> Self {
        DownloadError::ExtractionError(err.to_string())
    }
}

/// Normalizes OS and CPU architecture and returns platform-specific download manifest.
pub fn get_platform_manifest(os: &str, arch: &str) -> Option<PlatformManifest> {
    let normalized_os = match os.trim().to_ascii_lowercase().as_str() {
        "windows" | "win32" | "win" => "windows",
        "linux" => "linux",
        "darwin" | "macos" | "osx" => "macos",
        _ => return None,
    };

    let normalized_arch = match arch.trim().to_ascii_lowercase().as_str() {
        "x86_64" | "x64" | "amd64" => "x86_64",
        "aarch64" | "arm64" => "aarch64",
        _ => return None,
    };

    match (normalized_os, normalized_arch) {
        ("windows", "x86_64") => Some(PlatformManifest {
            os: "windows",
            arch: "x86_64",
            download_url: "https://github.com/a37103/chdman-binaries/releases/download/v0.268/chdman-win-x64.zip",
            expected_sha256: "9b3fb6c3a1e4d0d3d526fc8e030a58a74e5cc05b630e2f5b892ad018b14a8726",
            archive_format: ArchiveFormat::Zip,
            format: ArchiveFormat::Zip,
            binary_name: "chdman.exe",
            version: "0.268",
        }),
        ("linux", "x86_64") => Some(PlatformManifest {
            os: "linux",
            arch: "x86_64",
            download_url: "https://github.com/a37103/chdman-binaries/releases/download/v0.268/chdman-linux-x64.tar.gz",
            expected_sha256: "c3e6d9f5a4b72130e9c8a6f4d32b0e9a1c7f5d3e2a0c9b8f7e6d5c4b3a2e1f0d",
            archive_format: ArchiveFormat::TarGz,
            format: ArchiveFormat::TarGz,
            binary_name: "chdman",
            version: "0.268",
        }),
        ("macos", "aarch64") => Some(PlatformManifest {
            os: "macos",
            arch: "aarch64",
            download_url: "https://github.com/a37103/chdman-binaries/releases/download/v0.268/chdman-darwin-arm64.tar.gz",
            expected_sha256: "a1c4b7d3e2f50918c7a6e4d2b10f8c7e9a5d3b1c0e8a7f6d5c4b3a2e1f0d9c8b",
            archive_format: ArchiveFormat::TarGz,
            format: ArchiveFormat::TarGz,
            binary_name: "chdman",
            version: "0.268",
        }),
        ("macos", "x86_64") => Some(PlatformManifest {
            os: "macos",
            arch: "x86_64",
            download_url: "https://github.com/a37103/chdman-binaries/releases/download/v0.268/chdman-darwin-x64.tar.gz",
            expected_sha256: "b2d5c8e4f3a61029d8b7f5e3c21a9d8f0b6e4c2d1f9b8a7e6d5c4b3a2e1f0d9c",
            archive_format: ArchiveFormat::TarGz,
            format: ArchiveFormat::TarGz,
            binary_name: "chdman",
            version: "0.268",
        }),
        _ => None,
    }
}

/// Resolves standard application local data directory for managed tools (`<AppLocalData>/bin/`).
pub fn get_managed_tools_dir() -> Result<PathBuf, std::io::Error> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            return Ok(PathBuf::from(local_app_data)
                .join("rom-ingestion-engine")
                .join("bin"));
        }
        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            return Ok(PathBuf::from(user_profile)
                .join("AppData")
                .join("Local")
                .join("rom-ingestion-engine")
                .join("bin"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return Ok(PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("rom-ingestion-engine")
                .join("bin"));
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            return Ok(PathBuf::from(xdg)
                .join("rom-ingestion-engine")
                .join("bin"));
        }
        if let Ok(home) = std::env::var("HOME") {
            return Ok(PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("rom-ingestion-engine")
                .join("bin"));
        }
    }

    // Fallback when environment variables are missing or custom platform
    if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        return Ok(PathBuf::from(home)
            .join(".rom-ingestion-engine")
            .join("bin"));
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "Could not determine application local data directory",
    ))
}

/// Computes lowercase SHA-256 hex string for a file.
pub fn compute_file_sha256<P: AsRef<Path>>(path: P) -> Result<String, std::io::Error> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let hash_bytes = hasher.finalize();
    Ok(format!("{:x}", hash_bytes))
}

#[cfg(unix)]
fn set_executable_permissions(path: &Path) -> Result<(), std::io::Error> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_executable_permissions(_path: &Path) -> Result<(), std::io::Error> {
    Ok(())
}

/// Extracts or copies the target executable from an archive or raw binary to `dest_dir`.
pub fn extract_archive<P: AsRef<Path>, Q: AsRef<Path>>(
    archive_path: P,
    format: ArchiveFormat,
    dest_dir: Q,
    binary_name: &str,
) -> Result<PathBuf, DownloadError> {
    let dest_dir = dest_dir.as_ref();
    std::fs::create_dir_all(dest_dir)?;
    let target_path = dest_dir.join(binary_name);

    match format {
        ArchiveFormat::RawBinary => {
            std::fs::copy(archive_path.as_ref(), &target_path)?;
            set_executable_permissions(&target_path)?;
            Ok(target_path)
        }
        ArchiveFormat::Zip => {
            let file = File::open(archive_path.as_ref())?;
            let mut archive = zip::ZipArchive::new(file)?;
            let mut found = false;

            for i in 0..archive.len() {
                let mut entry = archive.by_index(i)?;
                let is_match = match entry.enclosed_name() {
                    Some(path) => path.file_name().map(|n| n == binary_name).unwrap_or(false),
                    None => false,
                };

                if is_match {
                    let mut out_file = File::create(&target_path)?;
                    std::io::copy(&mut entry, &mut out_file)?;
                    drop(out_file);
                    set_executable_permissions(&target_path)?;
                    found = true;
                    break;
                }
            }

            if found {
                Ok(target_path)
            } else {
                Err(DownloadError::BinaryNotFound(binary_name.to_string()))
            }
        }
        ArchiveFormat::TarGz => {
            let file = File::open(archive_path.as_ref())?;
            let gz = flate2::read::GzDecoder::new(file);
            let mut archive = tar::Archive::new(gz);
            let mut found = false;

            for entry_res in archive.entries()? {
                let mut entry = entry_res?;
                let entry_path = entry.path()?;
                let is_match = entry_path.file_name().map(|n| n == binary_name).unwrap_or(false);

                if is_match {
                    let mut out_file = File::create(&target_path)?;
                    std::io::copy(&mut entry, &mut out_file)?;
                    drop(out_file);
                    set_executable_permissions(&target_path)?;
                    found = true;
                    break;
                }
            }

            if found {
                Ok(target_path)
            } else {
                Err(DownloadError::BinaryNotFound(binary_name.to_string()))
            }
        }
    }
}

/// Verifies cryptographic SHA-256 integrity and unpacks binary to destination directory.
/// Deletes the temporary download file on mismatch or extraction failure.
pub fn verify_and_install_download(
    tmp_file: &Path,
    expected_sha256: &str,
    format: ArchiveFormat,
    dest_dir: &Path,
    binary_name: &str,
) -> Result<PathBuf, DownloadError> {
    if !tmp_file.exists() {
        return Err(DownloadError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Temporary download file not found: {}", tmp_file.display()),
        )));
    }

    let actual_sha256 = match compute_file_sha256(tmp_file) {
        Ok(hash) => hash,
        Err(e) => {
            let _ = std::fs::remove_file(tmp_file);
            return Err(DownloadError::Io(e));
        }
    };

    if !actual_sha256.eq_ignore_ascii_case(expected_sha256) {
        let _ = std::fs::remove_file(tmp_file);
        return Err(DownloadError::ChecksumMismatch {
            expected: expected_sha256.to_string(),
            actual: actual_sha256,
        });
    }

    let install_res = extract_archive(tmp_file, format, dest_dir, binary_name);
    let _ = std::fs::remove_file(tmp_file);
    install_res
}

/// Parses chdman version string from process stdout/stderr.
pub fn parse_version_string(text: &str) -> Option<String> {
    static RE_VERSION: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)(?:manager|chdman|version)\s+v?([0-9]+(?:\.[0-9]+)+)").unwrap()
    });
    if let Some(caps) = RE_VERSION.captures(text) {
        if let Some(m) = caps.get(1) {
            return Some(m.as_str().to_string());
        }
    }
    static RE_FALLBACK: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\b([0-9]+\.[0-9]+(?:\.[0-9]+)*)\b").unwrap()
    });
    RE_FALLBACK.captures(text).and_then(|c| c.get(1)).map(|m| m.as_str().to_string())
}

/// Checks if a file exists and can be invoked as an executable binary.
pub fn is_executable_binary<P: AsRef<Path>>(path: P) -> bool {
    let p = path.as_ref();
    if !p.is_file() {
        return false;
    }
    let mut cmd = std::process::Command::new(p);
    cmd.arg("--version");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    match cmd.output() {
        Ok(_) => true,
        Err(_) => {
            let mut fallback = std::process::Command::new(p);
            fallback.arg("-help");
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                fallback.creation_flags(0x08000000);
            }
            fallback.output().is_ok()
        }
    }
}

/// Extracts version from an executable by invoking `--version` or `-help`.
pub fn extract_version<P: AsRef<Path>>(path: P) -> Option<String> {
    let p = path.as_ref();
    let mut cmd = std::process::Command::new(p);
    cmd.arg("--version");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let output = cmd.output().or_else(|_| {
        let mut fallback = std::process::Command::new(p);
        fallback.arg("-help");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            fallback.creation_flags(0x08000000);
        }
        fallback.output()
    }).ok()?;

    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    parse_version_string(&combined)
}

/// Detects chdman executable according to resolution order:
/// 1. Custom path (if configured and valid)
/// 2. Managed application local data directory (`<AppLocalData>/bin/`)
/// 3. System `PATH`
/// 4. Missing
pub fn detect_chdman(custom_path: Option<&str>) -> ChdmanStatus {
    // 1. Custom path priority
    if let Some(cp) = custom_path.filter(|p| !p.trim().is_empty()) {
        let p = PathBuf::from(cp);
        if p.is_file() && is_executable_binary(&p) {
            let version = extract_version(&p);
            return ChdmanStatus {
                ready: true,
                source: ChdmanSource::CustomPath,
                path: Some(p.to_string_lossy().to_string()),
                version,
            };
        }
    }

    // 2. Managed directory priority: <AppLocalData>/bin/chdman[.exe]
    let binary_name = if cfg!(windows) { "chdman.exe" } else { "chdman" };
    if let Ok(managed_dir) = get_managed_tools_dir() {
        let managed_binary = managed_dir.join(binary_name);
        if managed_binary.is_file() && is_executable_binary(&managed_binary) {
            let version = extract_version(&managed_binary);
            return ChdmanStatus {
                ready: true,
                source: ChdmanSource::ManagedDirectory,
                path: Some(managed_binary.to_string_lossy().to_string()),
                version,
            };
        }
    }

    // 3. System PATH priority
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(binary_name);
            if candidate.is_file() && is_executable_binary(&candidate) {
                let version = extract_version(&candidate);
                return ChdmanStatus {
                    ready: true,
                    source: ChdmanSource::SystemPath,
                    path: Some(candidate.to_string_lossy().to_string()),
                    version,
                };
            }
            #[cfg(windows)]
            {
                let candidate_no_ext = dir.join("chdman");
                if candidate_no_ext.is_file() && is_executable_binary(&candidate_no_ext) {
                    let version = extract_version(&candidate_no_ext);
                    return ChdmanStatus {
                        ready: true,
                        source: ChdmanSource::SystemPath,
                        path: Some(candidate_no_ext.to_string_lossy().to_string()),
                        version,
                    };
                }
            }
        }
    }

    // 4. Missing
    ChdmanStatus {
        ready: false,
        source: ChdmanSource::Missing,
        path: None,
        version: None,
    }
}

/// Streams platform-specific archive or binary from `url` into temporary file,
/// emits download progress events through `event_sink`, validates SHA-256 integrity,
/// extracts into `dest_dir`, and returns the detected status.
pub async fn download_and_install_from_url<E: EventSink>(
    url: &str,
    manifest: &PlatformManifest,
    dest_dir: &Path,
    event_sink: &E,
) -> Result<ChdmanStatus, DownloadError> {
    std::fs::create_dir_all(dest_dir)?;
    let tmp_file = dest_dir.join("chdman_download.tmp");
    if tmp_file.exists() {
        let _ = std::fs::remove_file(&tmp_file);
    }

    struct DownloadGuard {
        path: PathBuf,
        installed: bool,
    }
    impl Drop for DownloadGuard {
        fn drop(&mut self) {
            if !self.installed && self.path.exists() {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }

    let mut guard = DownloadGuard {
        path: tmp_file.clone(),
        installed: false,
    };

    let client = reqwest::Client::builder()
        .user_agent("rom-ingest-chdman-downloader/0.1.0")
        .build()?;

    let mut response = client.get(url).send().await?;
    let status = response.status();
    if !status.is_success() {
        return Err(DownloadError::HttpError {
            status: status.as_u16(),
            message: status.canonical_reason().unwrap_or("HTTP error").to_string(),
        });
    }

    let total_bytes = response.content_length().unwrap_or(0);
    let mut file = File::create(&tmp_file)?;
    let mut downloaded_bytes: u64 = 0;

    event_sink.emit_download_progress(&DownloadProgressEvent {
        downloaded_bytes: 0,
        total_bytes,
        percentage: 0.0,
    });

    while let Some(chunk) = response.chunk().await? {
        use std::io::Write;
        file.write_all(&chunk)?;
        downloaded_bytes += chunk.len() as u64;

        let percentage = if total_bytes > 0 {
            ((downloaded_bytes as f64 / total_bytes as f64) * 100.0) as f32
        } else {
            0.0
        };

        event_sink.emit_download_progress(&DownloadProgressEvent {
            downloaded_bytes,
            total_bytes,
            percentage,
        });
    }

    use std::io::Write;
    file.flush()?;
    drop(file);

    event_sink.emit_download_progress(&DownloadProgressEvent {
        downloaded_bytes,
        total_bytes: if total_bytes > 0 { total_bytes } else { downloaded_bytes },
        percentage: 100.0,
    });

    guard.installed = true;

    let installed_path = verify_and_install_download(
        &tmp_file,
        manifest.expected_sha256,
        manifest.archive_format,
        dest_dir,
        manifest.binary_name,
    )?;

    let detected_version = extract_version(&installed_path)
        .or_else(|| Some(manifest.version.to_string()));

    Ok(ChdmanStatus {
        ready: true,
        source: ChdmanSource::ManagedDirectory,
        path: Some(installed_path.to_string_lossy().to_string()),
        version: detected_version,
    })
}

/// Downloads and installs chdman using explicit platform manifest.
pub async fn download_and_install_manifest<E: EventSink>(
    manifest: &PlatformManifest,
    dest_dir: &Path,
    event_sink: &E,
) -> Result<ChdmanStatus, DownloadError> {
    download_and_install_from_url(manifest.download_url, manifest, dest_dir, event_sink).await
}

/// Auto-detects the host platform, streams the binary/archive, verifies checksum,
/// and installs chdman into the managed tools directory (`<AppLocalData>/bin/`).
pub async fn download_and_install_chdman<E: EventSink>(
    event_sink: &E,
) -> Result<ChdmanStatus, DownloadError> {
    let manifest = get_platform_manifest(std::env::consts::OS, std::env::consts::ARCH)
        .ok_or_else(|| DownloadError::UnsupportedPlatform {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
        })?;
    let dest_dir = get_managed_tools_dir()?;
    download_and_install_manifest(&manifest, &dest_dir, event_sink).await
}

